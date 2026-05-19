//! Cross-path rename / move detection — `spec-output-schema.md` § v1.7.
//!
//! Two arms, both feeding a single `renames` envelope array:
//! - **Case A** (`raw_equal: true`) — `local_only_changed.local_sha` ==
//!   `remote_only_changed.remote_sha` hash-join. Zero extra fetches.
//! - **Case B** (`raw_equal: false`) — Case A 로 unmatched 인
//!   `remote_only_changed` 한정 remote blob bytes fetch + `prepare_for_hash`
//!   normalize → `local_sha` 와 hash 비교. Issue #1 cosmetic-identical 검사를
//!   cross-path 시나리오로 mirror.
//!
//! Read-only — facts only; no fs/remote mutation. Caller decides direction +
//! action on the hint.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use crate::commands::scan::compare::{FileEntry, RenamePair, Status};
use crate::shared::gh::GhClient;
use crate::shared::gitattributes::GitAttributes;
use crate::shared::github::fetch_blob;
use crate::shared::hash::blob_hash;
use crate::shared::normalize::prepare_for_hash;

/// Cap on the unmatched `remote_only_changed` pool feeding Case B. Beyond
/// this count the arm is skipped (with a stderr diagnostic line) to keep
/// regular-vault scans from silently regressing into N blob fetches.
const CASE_B_UNMATCHED_CAP: usize = 256;

/// Detect rename / move pairs across `local_only_changed` ↔
/// `remote_only_changed` entries. Always returns a `Vec` (possibly empty);
/// the caller wraps in `Some(...)` to emit the envelope `renames` key.
pub(super) fn detect_renames<C: GhClient + Sync>(
    entries: &[FileEntry],
    gitattr: &Arc<GitAttributes>,
    keep_bom: bool,
    client: &C,
    repo: &str,
) -> Vec<RenamePair> {
    // Group local_only_changed / remote_only_changed by sha. BTreeMap keeps
    // emit order deterministic across runs (caller diffability).
    let local_by_sha = group_paths_by_sha(entries, Status::LocalOnlyChanged, |e| {
        e.local_sha.as_deref()
    });
    let remote_by_sha = group_paths_by_sha(entries, Status::RemoteOnlyChanged, |e| {
        e.remote_sha.as_deref()
    });

    // Short-circuit 1 — either side empty → nothing to match.
    if local_by_sha.is_empty() || remote_by_sha.is_empty() {
        return Vec::new();
    }

    let mut pairs = Vec::new();
    let mut matched_remote_shas: HashSet<String> = HashSet::new();

    // Case A — same-sha hash-join.
    for (sha, remote_paths) in &remote_by_sha {
        if let Some(local_paths) = local_by_sha.get(sha) {
            emit_pairs(&mut pairs, local_paths, remote_paths, sha, true);
            matched_remote_shas.insert(sha.clone());
        }
    }

    // Case B — normalize-equal cross-path on Case A unmatched remotes.
    let unmatched: Vec<(&String, &Vec<String>)> = remote_by_sha
        .iter()
        .filter(|(sha, _)| !matched_remote_shas.contains(sha.as_str()))
        .collect();

    // Short-circuit 2 — cap exceeded → skip Case B, keep Case A results.
    if unmatched.len() > CASE_B_UNMATCHED_CAP {
        eprintln!(
            "renames: Case B skipped, unmatched remote_only_changed={} exceeds cap={CASE_B_UNMATCHED_CAP}",
            unmatched.len()
        );
        return pairs;
    }

    for (remote_sha, remote_paths) in unmatched {
        let Some(first_path) = remote_paths.first() else {
            continue;
        };
        // Best-effort fetch — any error (auth / rate / network / decode)
        // drops Case B for this entry. Case A pairs are preserved.
        let Ok(bytes) = fetch_blob(client, repo, remote_sha) else {
            continue;
        };
        let (prepared, _is_binary) = prepare_for_hash(&bytes, keep_bom, gitattr, first_path);
        let normalized_hash = blob_hash(&prepared);
        if let Some(local_paths) = local_by_sha.get(&normalized_hash) {
            emit_pairs(
                &mut pairs,
                local_paths,
                remote_paths,
                &normalized_hash,
                false,
            );
        }
    }

    pairs
}

fn group_paths_by_sha<F>(
    entries: &[FileEntry],
    status: Status,
    sha_of: F,
) -> BTreeMap<String, Vec<String>>
where
    F: Fn(&FileEntry) -> Option<&str>,
{
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in entries {
        if entry.status != status {
            continue;
        }
        let Some(sha) = sha_of(entry) else {
            continue;
        };
        out.entry(sha.to_string())
            .or_default()
            .push(entry.path.clone());
    }
    // Stable path order within each sha bucket (caller diffability).
    for paths in out.values_mut() {
        paths.sort();
    }
    out
}

fn emit_pairs(
    pairs: &mut Vec<RenamePair>,
    local_paths: &[String],
    remote_paths: &[String],
    sha: &str,
    raw_equal: bool,
) {
    for from in local_paths {
        for to in remote_paths {
            pairs.push(RenamePair {
                from: from.clone(),
                to: to.clone(),
                sha: sha.to_string(),
                raw_equal,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::scan::compare::{FileEntry, Presence, Status};
    use crate::shared::gh::{GhResponse, MockGhClient};

    fn entry(
        path: &str,
        status: Status,
        local_sha: Option<&str>,
        remote_sha: Option<&str>,
    ) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            status,
            presence: match status {
                Status::LocalOnlyChanged => Presence::LocalOnly,
                Status::RemoteOnlyChanged => Presence::RemoteOnly,
                _ => Presence::Both,
            },
            local_sha: local_sha.map(str::to_string),
            remote_sha: remote_sha.map(str::to_string),
            local_mtime: None,
            remote_last_commit_at: None,
            is_binary: false,
            mode: "100644".to_string(),
            diff_meaningful: None,
            failed_reason: None,
            lfs_pointer: None,
            size_bytes: None,
        }
    }

    fn empty_gitattr() -> Arc<GitAttributes> {
        Arc::new(GitAttributes::default())
    }

    fn detect(entries: &[FileEntry], client: &MockGhClient) -> Vec<RenamePair> {
        detect_renames(entries, &empty_gitattr(), false, client, "o/r")
    }

    fn local(p: &str, sha: &str) -> FileEntry {
        entry(p, Status::LocalOnlyChanged, Some(sha), None)
    }

    fn remote(p: &str, sha: &str) -> FileEntry {
        entry(p, Status::RemoteOnlyChanged, None, Some(sha))
    }

    #[test]
    fn case_a_emits_single_pair_for_matching_sha() {
        let pairs = detect(
            &[local("old/file.md", "abc"), remote("new/file.md", "abc")],
            &MockGhClient::new(),
        );
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].from, "old/file.md");
        assert_eq!(pairs[0].to, "new/file.md");
        assert_eq!(pairs[0].sha, "abc");
        assert!(pairs[0].raw_equal);
    }

    #[test]
    fn empty_when_local_only_count_is_zero() {
        let pairs = detect(&[remote("new/file.md", "abc")], &MockGhClient::new());
        assert!(pairs.is_empty());
    }

    #[test]
    fn empty_when_remote_only_count_is_zero() {
        let pairs = detect(&[local("old/file.md", "abc")], &MockGhClient::new());
        assert!(pairs.is_empty());
    }

    #[test]
    fn disjoint_shas_emit_no_pairs() {
        // Unstubbed fetch_blob would error → Case B silently skipped.
        let pairs = detect(
            &[local("a.md", "abc"), remote("b.md", "def")],
            &MockGhClient::new(),
        );
        assert!(pairs.is_empty());
    }

    #[test]
    fn collision_one_local_two_remote_emits_two_pairs() {
        let pairs = detect(
            &[
                local("old.md", "abc"),
                remote("new1.md", "abc"),
                remote("new2.md", "abc"),
            ],
            &MockGhClient::new(),
        );
        assert_eq!(pairs.len(), 2);
        assert!(pairs.iter().all(|p| p.from == "old.md" && p.raw_equal));
        let tos: HashSet<&str> = pairs.iter().map(|p| p.to.as_str()).collect();
        assert!(tos.contains("new1.md") && tos.contains("new2.md"));
    }

    #[test]
    fn case_b_emits_pair_when_normalize_equal_cross_path() {
        // local prepared bytes hash = blob_hash(b"hello\n"); remote raw blob
        // is `hello\r\n` (CRLF). raw shas differ but normalize-equal matches.
        let local_norm = blob_hash(b"hello\n");
        // base64("hello\r\n") = "aGVsbG8NCg=="
        let body =
            r#"{"sha":"deadbeef","content":"aGVsbG8NCg==","encoding":"base64","size":7,"url":"u"}"#;
        let mut mock = MockGhClient::new();
        mock.stub(
            vec!["api".into(), "repos/o/r/git/blobs/deadbeef".into()],
            GhResponse {
                stdout: body.as_bytes().to_vec(),
                stderr: String::new(),
                exit_code: 0,
            },
        );
        let pairs = detect(
            &[local("old.md", &local_norm), remote("new.md", "deadbeef")],
            &mock,
        );
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].from, "old.md");
        assert_eq!(pairs[0].to, "new.md");
        assert!(!pairs[0].raw_equal);
        assert_eq!(pairs[0].sha, local_norm);
    }

    #[test]
    fn case_b_fetch_failure_is_silently_skipped() {
        // Unstubbed MockGhClient → fetch errors → Case B drops candidate.
        let pairs = detect(
            &[local("old.md", "abc"), remote("new.md", "def")],
            &MockGhClient::new(),
        );
        assert!(pairs.is_empty());
    }

    #[test]
    fn case_b_cap_skips_fetch_when_unmatched_exceeds_threshold() {
        // 257 unmatched remote_only entries (cap=256) → Case B short-circuits
        // before fetch_blob. MockGhClient unstubbed; cap skip is the only
        // path avoiding a fetch error.
        let mut entries = vec![local("old.md", "abc")];
        for i in 0..=CASE_B_UNMATCHED_CAP {
            entries.push(remote(&format!("new{i}.md"), &format!("sha-{i}")));
        }
        let pairs = detect(&entries, &MockGhClient::new());
        assert!(pairs.is_empty());
    }
}
