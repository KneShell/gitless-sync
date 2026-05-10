//! GitHub Trees API client (recursive=1) + Phase 7 sub-tree fallback.
//!
//! Phase 5.13 task CC split — three wire-level sub-modules per
//! `spec-architecture.md` § Module 폴더 단위 정책:
//! - [`parse`] — wire-level deserialization + truncation guard.
//! - [`classify`] — tree-entry → [`RemoteFile`] mode-bit dispatch (pure).
//! - [`fetch`] — `recursive=1` orchestrator (`GhClient` → error map →
//!   parse → classify).
//!
//! Phase 7 task E added [`fetch_tree_with_fallback`] in this file as the
//! caller-facing entry point: it routes the non-truncated path through
//! [`fetch_tree`] unchanged (v0.2.x byte-identical) and only invokes the
//! [`fallback`] sub-module when the initial recursive response sets
//! `truncated: true`.
//!
//! Caller imports stay `crate::shared::github::{RemoteFile,
//! fetch_tree_with_fallback}` via the parent's re-exports in
//! `shared/github/mod.rs`.

mod classify;
mod fallback;
mod fetch;
mod parse;

pub use classify::RemoteFile;
pub(crate) use fetch::fetch_tree;

use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use fallback::recursive::{Descent, fetch_subtree_recursive};
use fallback::{Budget, resolve_root_tree_sha};

/// Trees fetch with sub-tree fallback — Phase 7 caller-facing entry point.
///
/// Calls [`fetch_tree`] for the initial `recursive=1` request. When that
/// returns [`GitlessError::TreesTruncated`], resolves the branch's root
/// tree sha (`refs/heads/{branch}` → commit sha → root tree sha) and
/// walks the tree one layer at a time via
/// [`fetch_subtree_recursive`] per `spec-github-api.md`
/// § Trees truncation handling. The non-truncated path stays
/// byte-identical to v0.2.x — only the truncated branch enters
/// [`fallback`].
///
/// Cap checks ([`fallback::MAX_TREE_CALL_BUDGET`] +
/// [`fallback::MAX_TREE_ENTRIES`]) gate every sub-tree call; either
/// trip aborts with `TreesTruncated` and discards partial state
/// (G-002 no-partial-result policy applies inside the fallback as well).
///
/// # Errors
/// - [`GitlessError::TreesTruncated`] when either cap is reached during
///   fallback, or a sub-tree response itself sets `truncated: true`.
/// - [`GitlessError::AuthFailed`] / [`GitlessError::RateLimitExceeded`]
///   / [`GitlessError::Http`] propagated from [`fetch_tree`],
///   [`resolve_root_tree_sha`], or [`fetch_subtree_recursive`] per
///   `spec-error-contracts.md`.
pub(crate) fn fetch_tree_with_fallback(
    client: &impl GhClient,
    repo: &str,
    branch: &str,
) -> Result<Vec<RemoteFile>, GitlessError> {
    match fetch_tree(client, repo, branch) {
        Ok(entries) => Ok(entries),
        Err(GitlessError::TreesTruncated) => {
            let root_sha = resolve_root_tree_sha(client, repo, branch)?;
            let mut entries = Vec::new();
            let mut budget = Budget::new();
            {
                let mut descent = Descent {
                    client,
                    repo,
                    entries: &mut entries,
                    budget: &mut budget,
                };
                fetch_subtree_recursive(&mut descent, &root_sha, "")?;
            }
            Ok(entries)
        }
        Err(other) => Err(other),
    }
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
mod tests {
    use super::*;
    use crate::shared::gh::{GhResponse, MockGhClient};

    fn ok_resp(stdout: &[u8]) -> GhResponse {
        GhResponse {
            stdout: stdout.to_vec(),
            stderr: String::new(),
            exit_code: 0,
        }
    }

    fn err_resp(stderr: &str) -> GhResponse {
        GhResponse {
            stdout: Vec::new(),
            stderr: stderr.to_string(),
            exit_code: 1,
        }
    }

    fn recursive_args(repo: &str, branch: &str) -> Vec<String> {
        vec![
            "api".to_string(),
            format!("repos/{repo}/git/trees/{branch}?recursive=1"),
        ]
    }

    fn ref_args(repo: &str, branch: &str) -> Vec<String> {
        vec![
            "api".to_string(),
            format!("repos/{repo}/git/refs/heads/{branch}"),
        ]
    }

    fn commit_args(repo: &str, sha: &str) -> Vec<String> {
        vec!["api".to_string(), format!("repos/{repo}/git/commits/{sha}")]
    }

    fn sub_tree_args(repo: &str, sha: &str) -> Vec<String> {
        vec!["api".to_string(), format!("repos/{repo}/git/trees/{sha}")]
    }

    #[test]
    fn non_truncated_response_returns_fetch_tree_entries_unchanged() {
        // v0.2.x parity check: when truncated=false, the result must be
        // identical to fetch_tree (no extra ref/commit calls performed).
        let body = br#"{
            "sha":"root",
            "tree":[
                {"path":"a.md","mode":"100644","type":"blob","sha":"sha1","size":3}
            ],
            "truncated":false
        }"#;
        let mut mock = MockGhClient::new();
        mock.stub(recursive_args("o/r", "main"), ok_resp(body));

        let entries = fetch_tree_with_fallback(&mock, "o/r", "main").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "a.md");
        assert_eq!(entries[0].sha, "sha1");
        assert_eq!(entries[0].mode, "100644");
    }

    #[test]
    fn truncated_response_descends_via_fallback_and_returns_subtree_entries() {
        let mut mock = MockGhClient::new();
        // recursive=1 -> truncated, triggers fallback.
        mock.stub(
            recursive_args("o/r", "main"),
            ok_resp(br#"{"sha":"x","tree":[],"truncated":true}"#),
        );
        // fallback resolve: branch ref -> commit sha -> root tree sha.
        mock.stub(
            ref_args("o/r", "main"),
            ok_resp(br#"{"ref":"refs/heads/main","object":{"sha":"commit_abc","type":"commit"}}"#),
        );
        mock.stub(
            commit_args("o/r", "commit_abc"),
            ok_resp(br#"{"sha":"commit_abc","tree":{"sha":"root_tree"},"message":"m"}"#),
        );
        // fallback descent on root_tree returns one blob.
        mock.stub(
            sub_tree_args("o/r", "root_tree"),
            ok_resp(
                br#"{
                "sha":"root_tree",
                "tree":[{"path":"a.md","mode":"100644","type":"blob","sha":"blob_a","size":3}],
                "truncated":false
            }"#,
            ),
        );

        let entries = fetch_tree_with_fallback(&mock, "o/r", "main").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "a.md");
        assert_eq!(entries[0].sha, "blob_a");
        assert_eq!(entries[0].mode, "100644");
    }

    #[test]
    fn non_truncated_error_propagates_without_entering_fallback() {
        // No ref/commit stubs: if the fallback ran, the test would
        // surface an unstubbed-args Http error instead of AuthFailed.
        let mut mock = MockGhClient::new();
        mock.stub(
            recursive_args("o/r", "main"),
            err_resp("gh: Bad credentials (HTTP 401)"),
        );

        let err = fetch_tree_with_fallback(&mock, "o/r", "main").unwrap_err();
        assert!(matches!(err, GitlessError::AuthFailed));
    }

    #[test]
    fn fallback_resolve_failure_propagates_with_original_variant() {
        let mut mock = MockGhClient::new();
        mock.stub(
            recursive_args("o/r", "main"),
            ok_resp(br#"{"sha":"x","tree":[],"truncated":true}"#),
        );
        mock.stub(
            ref_args("o/r", "main"),
            err_resp("gh: Bad credentials (HTTP 401)"),
        );

        let err = fetch_tree_with_fallback(&mock, "o/r", "main").unwrap_err();
        assert!(matches!(err, GitlessError::AuthFailed));
    }

    #[test]
    fn fallback_inner_truncated_subtree_returns_trees_truncated() {
        // PRD scenario 12 semantics under Phase 7: fallback descent
        // hitting a truncated sub-tree response surfaces TreesTruncated
        // (G-002 no-partial-result policy preserved end-to-end).
        let mut mock = MockGhClient::new();
        mock.stub(
            recursive_args("o/r", "main"),
            ok_resp(br#"{"sha":"x","tree":[],"truncated":true}"#),
        );
        mock.stub(
            ref_args("o/r", "main"),
            ok_resp(br#"{"object":{"sha":"c0"}}"#),
        );
        mock.stub(
            commit_args("o/r", "c0"),
            ok_resp(br#"{"tree":{"sha":"root_tree"}}"#),
        );
        mock.stub(
            sub_tree_args("o/r", "root_tree"),
            ok_resp(br#"{"sha":"root_tree","tree":[],"truncated":true}"#),
        );

        let err = fetch_tree_with_fallback(&mock, "o/r", "main").unwrap_err();
        assert!(matches!(err, GitlessError::TreesTruncated));
        assert_eq!(err.exit_code(), 5);
    }
}
