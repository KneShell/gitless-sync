//! Pass 1.5 — `normalize_equal` verification for sha-differ Hashed entries.
//!
//! Phase 8 task I (`spec-output-schema.md` § v1.3 — `diff_meaningful` field):
//! `local_sha` is already `blob_hash(prepare_for_hash(local_raw))`. When it
//! differs from the Trees-API `remote_sha`, fetch the remote blob, run
//! `prepare_for_hash` on its raw bytes, and compare the resulting
//! self-defined hash with `local_sha`. Match → `normalize_equal=true`
//! (F1 BOM/encoding-only drift); mismatch → `false` (real semantic diff).
//!
//! Sequential — `fetch_blob` is a `gh api` subprocess; rayon-parallelizing
//! at the rate-limit-friendly 8-concurrency cap from G-011 / ADR 0003 is a
//! follow-up optimization (yagni until vault dogfood surfaces wall-clock
//! pressure, mirrors the Phase 5 `hash_pass` sequential pattern in
//! `commands/scan/pipeline/hash_pass`).

use std::collections::HashMap;
use std::sync::Arc;

use super::hash_pass::{PreEntry, PreState};
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::gitattributes::GitAttributes;
use crate::shared::github::fetch_blob;
use crate::shared::hash::blob_hash;
use crate::shared::normalize::prepare_for_hash;

/// For each `Hashed` entry whose `local_sha != remote_sha`, fetch the remote
/// blob bytes and compute `blob_hash(prepare_for_hash(remote_raw))`. The
/// returned map keys on `path` and stores `true` when the recomputed hash
/// equals `local_sha` (= F1 normalize-only drift), `false` otherwise.
///
/// Identical / single-side / Failed entries do not appear in the map —
/// callers reading via `.get(path).copied()` get `None`, which feeds
/// `compare()` for the "unknown" `diff_meaningful` arm.
pub(super) fn fetch_normalize_equal_map<C: GhClient>(
    pending: &[PreEntry],
    client: &C,
    repo: &str,
    keep_bom: bool,
    gitattr: &Arc<GitAttributes>,
) -> Result<HashMap<String, bool>, GitlessError> {
    let mut map = HashMap::new();
    for pre in pending {
        let PreState::Hashed {
            local_sha: Some(l),
            remote_sha: Some(r),
            ..
        } = &pre.state
        else {
            continue;
        };
        if l == r {
            continue;
        }
        let raw = fetch_blob(client, repo, r)?;
        let (prepared, _) = prepare_for_hash(&raw, keep_bom, gitattr, &pre.path);
        let normalized_remote_sha = blob_hash(&prepared);
        map.insert(pre.path.clone(), normalized_remote_sha == *l);
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;

    use super::*;
    use crate::commands::scan::compare::Presence;
    use crate::commands::scan::pipeline::hash_pass::{PreEntry, PreState};
    use crate::commands::scan::test_helpers::mtime;
    use crate::shared::gh::{GhResponse, MockGhClient};

    fn blob_args(repo: &str, sha: &str) -> Vec<String> {
        vec!["api".to_string(), format!("repos/{repo}/git/blobs/{sha}")]
    }

    fn blob_body(content: &[u8]) -> String {
        let b64 = BASE64_STANDARD.encode(content);
        format!(r#"{{"sha":"x","content":"{b64}","encoding":"base64","size":1,"url":"u"}}"#)
    }

    fn empty_attrs() -> Arc<GitAttributes> {
        Arc::new(GitAttributes::default())
    }

    fn hashed_pre(path: &str, local: Option<&str>, remote: Option<&str>) -> PreEntry {
        PreEntry {
            path: path.to_string(),
            mode: "100644".to_string(),
            presence: Presence::Both,
            state: PreState::Hashed {
                local_sha: local.map(str::to_string),
                remote_sha: remote.map(str::to_string),
                local_mtime: Some(mtime(1_700_000_000)),
                is_binary: false,
            },
        }
    }

    fn failed_pre(path: &str, presence: Presence) -> PreEntry {
        PreEntry {
            path: path.to_string(),
            mode: "100644".to_string(),
            presence,
            state: PreState::Failed {
                remote_sha: Some("rs".to_string()),
                local_mtime: Some(mtime(1_700_000_000)),
                failed_reason: None,
                is_binary: false,
                size_bytes: None,
            },
        }
    }

    fn ok_response(stdout: &[u8]) -> GhResponse {
        GhResponse {
            stdout: stdout.to_vec(),
            stderr: String::new(),
            exit_code: 0,
        }
    }

    #[test]
    fn fetch_normalize_equal_map_returns_true_when_remote_normalize_matches_local_sha() {
        // F1 case — remote stored `hello\r\n` (Trees SHA = blob of CRLF
        // bytes, differs from local_sha which already prepared-and-hashed
        // `hello\n`). Fetching the remote blob, applying prepare_for_hash,
        // recomputing the self-defined hash → equals local_sha.
        let local_sha = blob_hash(b"hello\n");
        let entries = vec![hashed_pre(
            "a.md",
            Some(&local_sha),
            Some("remote-trees-sha"),
        )];

        let mut mock = MockGhClient::new();
        mock.stub(
            blob_args("o/r", "remote-trees-sha"),
            ok_response(blob_body(b"hello\r\n").as_bytes()),
        );

        let attrs = empty_attrs();
        let map = fetch_normalize_equal_map(&entries, &mock, "o/r", false, &attrs).unwrap();
        assert_eq!(map.get("a.md"), Some(&true));
    }

    #[test]
    fn fetch_normalize_equal_map_returns_false_when_normalize_still_differs() {
        // Real semantic drift — remote `goodbye\n` vs local `hello\n`. After
        // prepare_for_hash, both sides still differ → normalize_equal=false.
        let local_sha = blob_hash(b"hello\n");
        let entries = vec![hashed_pre("b.md", Some(&local_sha), Some("remote-other"))];

        let mut mock = MockGhClient::new();
        mock.stub(
            blob_args("o/r", "remote-other"),
            ok_response(blob_body(b"goodbye\n").as_bytes()),
        );

        let attrs = empty_attrs();
        let map = fetch_normalize_equal_map(&entries, &mock, "o/r", false, &attrs).unwrap();
        assert_eq!(map.get("b.md"), Some(&false));
    }

    #[test]
    fn fetch_normalize_equal_map_skips_identical_sha_entries() {
        // local_sha == remote_sha → no fetch (Identical entry, diff_meaningful
        // already determined as Some(false) at compare time). MockGhClient
        // unstubbed — any fetch attempt would Err. Empty map proves zero
        // invocations.
        let entries = vec![hashed_pre("c.md", Some("same"), Some("same"))];
        let mock = MockGhClient::new();
        let attrs = empty_attrs();
        let map = fetch_normalize_equal_map(&entries, &mock, "o/r", false, &attrs).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn fetch_normalize_equal_map_skips_local_only_entries() {
        let entries = vec![hashed_pre("d.md", Some("l"), None)];
        let mock = MockGhClient::new();
        let attrs = empty_attrs();
        let map = fetch_normalize_equal_map(&entries, &mock, "o/r", false, &attrs).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn fetch_normalize_equal_map_skips_remote_only_entries() {
        let entries = vec![hashed_pre("e.md", None, Some("r"))];
        let mock = MockGhClient::new();
        let attrs = empty_attrs();
        let map = fetch_normalize_equal_map(&entries, &mock, "o/r", false, &attrs).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn fetch_normalize_equal_map_skips_failed_entries() {
        let entries = vec![failed_pre("f.md", Presence::Both)];
        let mock = MockGhClient::new();
        let attrs = empty_attrs();
        let map = fetch_normalize_equal_map(&entries, &mock, "o/r", false, &attrs).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn fetch_normalize_equal_map_propagates_blob_fetch_error() {
        // Unstubbed mock → fetch_blob errors → propagated as Err.
        let local_sha = blob_hash(b"hello\n");
        let entries = vec![hashed_pre("g.md", Some(&local_sha), Some("missing"))];
        let mock = MockGhClient::new();
        let attrs = empty_attrs();
        let err = fetch_normalize_equal_map(&entries, &mock, "o/r", false, &attrs).unwrap_err();
        assert!(matches!(err, GitlessError::Http(_)));
    }
}
