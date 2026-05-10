//! Pass 3 — `pre_entry_to_file` + `finalize_entries`.
//!
//! Each `PreEntry` resolves to a `FileEntry` via `classify` (4-state status
//! decision) + `compare` (presence + `diff_meaningful`, Phase 8 task H/I).
//! `presence` flows straight from `PreEntry` (covers `Failed` entries where
//! the SHA-derived presence in `compare()` would lose information). The
//! `Hashed` arm consults the upstream `normalize_eq_map` to feed `compare`'s
//! third arg per `spec-output-schema.md` § v1.3.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::super::hash_pass::{PreEntry, PreState};
use crate::commands::scan::compare::{FileEntry, Status, classify, compare};
use crate::commands::scan::lfs;
use crate::commands::scan::output::Summary;

struct CompareCtx<'a> {
    commit_map: &'a HashMap<String, DateTime<Utc>>,
    normalize_eq_map: &'a HashMap<String, bool>,
}

pub(in super::super) fn finalize_entries(
    pending: Vec<PreEntry>,
    commit_map: &HashMap<String, DateTime<Utc>>,
    normalize_eq_map: &HashMap<String, bool>,
) -> (Vec<FileEntry>, Summary, usize) {
    let mut entries: Vec<FileEntry> = Vec::with_capacity(pending.len());
    let mut summary = Summary::default();
    let mut failed_count = 0usize;
    let ctx = CompareCtx {
        commit_map,
        normalize_eq_map,
    };

    for pre in pending {
        let entry = pre_entry_to_file(pre, &ctx, &mut summary, &mut failed_count);
        entries.push(entry);
    }

    (entries, summary, failed_count)
}

fn pre_entry_to_file(
    pre: PreEntry,
    ctx: &CompareCtx<'_>,
    summary: &mut Summary,
    failed_count: &mut usize,
) -> FileEntry {
    match &pre.state {
        PreState::Failed { .. } => {
            summary.failed += 1;
            *failed_count += 1;
            failed_to_file_entry(pre)
        }
        PreState::Hashed { .. } => hashed_to_file_entry(pre, ctx, summary),
    }
}

fn failed_to_file_entry(pre: PreEntry) -> FileEntry {
    let PreEntry {
        path,
        mode,
        presence,
        state,
    } = pre;
    let PreState::Failed {
        remote_sha,
        local_mtime,
        failed_reason,
        is_binary,
        size_bytes,
    } = state
    else {
        unreachable!("failed_to_file_entry called with non-Failed state");
    };
    FileEntry {
        path,
        status: Status::Failed,
        presence,
        local_sha: None,
        remote_sha,
        local_mtime,
        remote_last_commit_at: None,
        is_binary,
        mode,
        diff_meaningful: None,
        lfs_pointer: lfs::placeholder_pointer_for(failed_reason),
        failed_reason,
        size_bytes,
    }
}

fn hashed_to_file_entry(pre: PreEntry, ctx: &CompareCtx<'_>, summary: &mut Summary) -> FileEntry {
    let PreEntry {
        path,
        mode,
        presence,
        state,
    } = pre;
    let PreState::Hashed {
        local_sha,
        remote_sha,
        local_mtime,
        is_binary,
    } = state
    else {
        unreachable!("hashed_to_file_entry called with non-Hashed state");
    };
    let remote_last_commit_at = ctx.commit_map.get(path.as_str()).copied();
    let status = classify(
        local_sha.as_deref(),
        remote_sha.as_deref(),
        local_mtime,
        remote_last_commit_at,
    );
    let normalize_equal = ctx.normalize_eq_map.get(path.as_str()).copied();
    let (_, diff_meaningful) =
        compare(local_sha.as_deref(), remote_sha.as_deref(), normalize_equal);
    summary.tally(status);
    FileEntry {
        path,
        status,
        presence,
        local_sha,
        remote_sha,
        local_mtime,
        remote_last_commit_at,
        is_binary,
        mode,
        diff_meaningful,
        failed_reason: None,
        lfs_pointer: None,
        size_bytes: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::scan::compare::{FailedReason, Presence};
    use crate::commands::scan::test_helpers::mtime;

    fn hashed(path: &str, local: Option<&str>, remote: Option<&str>) -> PreEntry {
        let presence = match (local.is_some(), remote.is_some()) {
            (true, true) => Presence::Both,
            (true, false) => Presence::LocalOnly,
            (false, true) => Presence::RemoteOnly,
            (false, false) => unreachable!(),
        };
        PreEntry {
            path: path.to_string(),
            mode: "100644".to_string(),
            presence,
            state: PreState::Hashed {
                local_sha: local.map(str::to_string),
                remote_sha: remote.map(str::to_string),
                local_mtime: Some(mtime(1_700_000_000)),
                is_binary: false,
            },
        }
    }

    fn failed_pre(reason: Option<FailedReason>, is_binary: bool) -> PreEntry {
        PreEntry {
            path: "x".to_string(),
            mode: "100644".to_string(),
            presence: Presence::Both,
            state: PreState::Failed {
                remote_sha: Some("rs".to_string()),
                local_mtime: Some(mtime(1_700_000_000)),
                failed_reason: reason,
                is_binary,
                size_bytes: None,
            },
        }
    }

    fn run(pre: PreEntry) -> (FileEntry, Summary, usize) {
        let mut summary = Summary::default();
        let mut failed = 0usize;
        let ctx = CompareCtx {
            commit_map: &HashMap::new(),
            normalize_eq_map: &HashMap::new(),
        };
        let entry = pre_entry_to_file(pre, &ctx, &mut summary, &mut failed);
        (entry, summary, failed)
    }

    #[test]
    fn failed_lfs_pointer_carries_placeholder_other_reasons_omit_pointer_and_dm() {
        // spec-output-schema.md § v1.1 + v1.3 — `lfs_pointer` placeholder
        // {oid:"?", size:0} only on `LfsPointer` reason; all Failed entries
        // have `diff_meaningful: None` (no comparable bytes).
        let (entry, summary, failed) = run(failed_pre(Some(FailedReason::LfsPointer), false));
        let pointer = entry.lfs_pointer.expect("lfs_pointer placeholder");
        assert_eq!(pointer.oid, "?");
        assert_eq!(pointer.size, 0);
        assert_eq!(entry.diff_meaningful, None);
        assert_eq!(failed, 1);
        assert_eq!(summary.failed, 1);
        for reason in [
            FailedReason::CaseCollision,
            FailedReason::Submodule,
            FailedReason::Symlink,
            FailedReason::LongPath,
            FailedReason::NfdCollision,
            FailedReason::GitattributesUnsupported,
        ] {
            let (entry, _, _) = run(failed_pre(Some(reason), false));
            assert!(entry.lfs_pointer.is_none(), "{reason:?} pointer leak");
            assert!(entry.diff_meaningful.is_none(), "{reason:?} dm leak");
        }
    }

    #[test]
    fn failed_encoding_preserves_is_binary_from_pre_entry() {
        // EE: encoding-failure preserves `try_hash_local`'s NUL heuristic
        // (UTF-16 BOM has embedded NULs → `is_binary: true` survives).
        let (entry, _, _) = run(failed_pre(Some(FailedReason::Encoding), true));
        assert_eq!(entry.failed_reason, Some(FailedReason::Encoding));
        assert!(entry.is_binary);
    }

    #[test]
    fn hashed_identical_emits_diff_meaningful_false_without_normalize_lookup() {
        // Phase 8 task I — sha-equal Hashed → diff_meaningful=Some(false)
        // (no fetch, no normalize_eq_map lookup).
        let (entry, summary, _) = run(hashed("a.md", Some("x"), Some("x")));
        assert_eq!(entry.status, Status::Identical);
        assert_eq!(entry.diff_meaningful, Some(false));
        assert_eq!(entry.presence, Presence::Both);
        assert_eq!(summary.identical, 1);
    }

    #[test]
    fn hashed_sha_differ_uses_normalize_eq_map_for_diff_meaningful() {
        // sha differ + map=Some(true) → dm=Some(false) (F1 normalize-only
        // drift); map=Some(false) → Some(true); absent → None.
        let mut s = Summary::default();
        let mut f = 0usize;
        let cm = HashMap::new();
        let mut nm = HashMap::new();
        nm.insert("a.md".to_string(), true);
        nm.insert("b.md".to_string(), false);
        let ctx = CompareCtx {
            commit_map: &cm,
            normalize_eq_map: &nm,
        };
        let e = pre_entry_to_file(hashed("a.md", Some("l"), Some("r")), &ctx, &mut s, &mut f);
        assert_eq!(e.diff_meaningful, Some(false));
        let e = pre_entry_to_file(hashed("b.md", Some("l"), Some("r")), &ctx, &mut s, &mut f);
        assert_eq!(e.diff_meaningful, Some(true));
        let (e, _, _) = run(hashed("c.md", Some("l"), Some("r")));
        assert_eq!(e.diff_meaningful, None);
    }

    #[test]
    fn finalize_entries_aggregates_summary_and_carries_presence_through() {
        // Multi-entry rollup — each PreEntry resolves through its own match
        // arm; presence flows through unchanged from PreEntry.
        let pending = vec![
            hashed("same.md", Some("abc"), Some("abc")),
            failed_pre(Some(FailedReason::Symlink), false),
            hashed("local-only.md", Some("l"), None),
        ];
        let (entries, summary, failed) =
            finalize_entries(pending, &HashMap::new(), &HashMap::new());
        assert_eq!(entries.len(), 3);
        assert_eq!(summary.identical, 1);
        assert_eq!(summary.local_only_changed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(failed, 1);
        assert_eq!(entries[2].presence, Presence::LocalOnly);
    }
}
