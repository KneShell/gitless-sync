//! Sub-tree fallback budget caps + root tree sha resolution + recursive descent (Phase 7).
//!
//! Module split per `spec-architecture.md` § Module 폴더 정책 — task D
//! pushed total LOC past 300, so the recursive descent
//! ([`recursive::fetch_subtree_recursive`]) sits in the [`recursive`]
//! sibling. Cap constants ([`MAX_TREE_CALL_BUDGET`] + [`MAX_TREE_ENTRIES`])
//! and the [`Budget`] counter live here because the recursion reads /
//! writes them through `super::`. The two-step
//! [`resolve_root_tree_sha`] (`ref` → commit → root tree) also lives
//! here — task E wires it together with the recursive descent into
//! `super::fetch_tree_with_fallback`.
//!
//! `resolve_root_tree_sha` keeps `allow(dead_code)` until task E plugs
//! it into the parent entry point.

mod recursive;

use serde::Deserialize;

use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::github::map_gh_error;

/// Hard cap on `gh api ...trees/{sub_tree_sha}` calls during a single
/// sub-tree fallback recovery. Exceeding aborts with
/// [`crate::shared::error::GitlessError::TreesTruncated`] (G-002
/// no-partial-result policy).
pub(super) const MAX_TREE_CALL_BUDGET: u32 = 1000;

/// Hard cap on cumulative entries during sub-tree fallback. Compared
/// against `Vec::<RemoteFile>::len()`. Exceeding aborts with
/// [`crate::shared::error::GitlessError::TreesTruncated`] (memory
/// safety).
pub(super) const MAX_TREE_ENTRIES: usize = 500_000;

/// Mutable counter advanced by the recursive descent. Read by the call
/// budget check; incremented after each `gh api` call. Initial value
/// is `0`.
#[derive(Debug, Default)]
pub(super) struct Budget {
    pub(super) calls_used: u32,
}

impl Budget {
    #[allow(dead_code)]
    pub(super) const fn new() -> Self {
        Self { calls_used: 0 }
    }
}

#[derive(Debug, Deserialize)]
struct RefResponse {
    object: RefObject,
}

#[derive(Debug, Deserialize)]
struct RefObject {
    sha: String,
}

#[derive(Debug, Deserialize)]
struct CommitResponse {
    tree: CommitTree,
}

#[derive(Debug, Deserialize)]
struct CommitTree {
    sha: String,
}

/// Resolve `branch` to a stable root tree sha via two `gh api` calls.
///
/// Called once at sub-tree fallback entry (per `spec-github-api.md`
/// § sha 일관성) so every subsequent sub-tree call addresses an
/// immutable tree object — guards against HEAD drift between the
/// initial Trees-recursive call and the fallback descent.
///
/// 1. `gh api repos/{repo}/git/refs/heads/{branch}` → response
///    `object.sha` (commit sha).
/// 2. `gh api repos/{repo}/git/commits/{commit_sha}` → response
///    `tree.sha` (root tree sha).
///
/// # Errors
/// - [`GitlessError::AuthFailed`] / [`GitlessError::RateLimitExceeded`]
///   / [`GitlessError::Http`] per `spec-error-contracts.md` (mapping
///   delegated to [`map_gh_error`]).
/// - [`GitlessError::Http`] for JSON decode failures, prefixed
///   `"decode refs response:"` or `"decode commits response:"`.
#[allow(dead_code)]
pub(super) fn resolve_root_tree_sha(
    client: &impl GhClient,
    repo: &str,
    branch: &str,
) -> Result<String, GitlessError> {
    let ref_args = vec![
        "api".to_string(),
        format!("repos/{repo}/git/refs/heads/{branch}"),
    ];
    let ref_resp = client.api(&ref_args)?;
    if ref_resp.exit_code != 0 {
        return Err(map_gh_error(&ref_resp.stderr));
    }
    let ref_body: RefResponse = serde_json::from_slice(&ref_resp.stdout)
        .map_err(|e| GitlessError::Http(format!("decode refs response: {e}")))?;
    let commit_sha = ref_body.object.sha;

    let commit_args = vec![
        "api".to_string(),
        format!("repos/{repo}/git/commits/{commit_sha}"),
    ];
    let commit_resp = client.api(&commit_args)?;
    if commit_resp.exit_code != 0 {
        return Err(map_gh_error(&commit_resp.stderr));
    }
    let commit_body: CommitResponse = serde_json::from_slice(&commit_resp.stdout)
        .map_err(|e| GitlessError::Http(format!("decode commits response: {e}")))?;

    Ok(commit_body.tree.sha)
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
mod tests {
    use super::{Budget, MAX_TREE_CALL_BUDGET, MAX_TREE_ENTRIES, resolve_root_tree_sha};
    use crate::shared::error::GitlessError;
    use crate::shared::gh::{GhResponse, MockGhClient};

    #[test]
    fn cap_constants_match_spec_values() {
        assert_eq!(MAX_TREE_CALL_BUDGET, 1000);
        assert_eq!(MAX_TREE_ENTRIES, 500_000);
    }

    #[test]
    fn budget_new_starts_at_zero_calls_used() {
        assert_eq!(Budget::new().calls_used, 0);
    }

    #[test]
    fn budget_default_matches_new() {
        assert_eq!(Budget::default().calls_used, Budget::new().calls_used);
    }

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

    fn ref_args(repo: &str, branch: &str) -> Vec<String> {
        vec![
            "api".to_string(),
            format!("repos/{repo}/git/refs/heads/{branch}"),
        ]
    }

    fn commit_args(repo: &str, sha: &str) -> Vec<String> {
        vec!["api".to_string(), format!("repos/{repo}/git/commits/{sha}")]
    }

    fn ref_body(commit_sha: &str) -> String {
        format!(r#"{{"ref":"refs/heads/main","object":{{"sha":"{commit_sha}","type":"commit"}}}}"#)
    }

    fn commit_body(tree_sha: &str) -> String {
        format!(r#"{{"sha":"c0","tree":{{"sha":"{tree_sha}"}},"message":"m"}}"#)
    }

    #[test]
    fn resolve_root_tree_sha_returns_tree_sha_after_two_calls() {
        let mut mock = MockGhClient::new();
        mock.stub(
            ref_args("o/r", "main"),
            ok_resp(ref_body("commit_abc").as_bytes()),
        );
        mock.stub(
            commit_args("o/r", "commit_abc"),
            ok_resp(commit_body("tree_xyz").as_bytes()),
        );

        let sha = resolve_root_tree_sha(&mock, "o/r", "main").unwrap();
        assert_eq!(sha, "tree_xyz");
    }

    #[test]
    fn resolve_root_tree_sha_chains_commit_sha_from_ref_response() {
        // Ensures the second call uses the sha returned by the first call —
        // not branch name, not a hardcoded value. If chain were broken the
        // commit_args stub keyed on "deadbeef" would not match.
        let mut mock = MockGhClient::new();
        mock.stub(
            ref_args("owner/repo", "feature"),
            ok_resp(ref_body("deadbeef").as_bytes()),
        );
        mock.stub(
            commit_args("owner/repo", "deadbeef"),
            ok_resp(commit_body("rooted").as_bytes()),
        );

        let sha = resolve_root_tree_sha(&mock, "owner/repo", "feature").unwrap();
        assert_eq!(sha, "rooted");
    }

    #[test]
    fn resolve_root_tree_sha_maps_ref_auth_failure() {
        let mut mock = MockGhClient::new();
        mock.stub(
            ref_args("o/r", "main"),
            err_resp("gh: Bad credentials (HTTP 401)"),
        );

        let err = resolve_root_tree_sha(&mock, "o/r", "main").unwrap_err();
        assert!(matches!(err, GitlessError::AuthFailed));
    }

    #[test]
    fn resolve_root_tree_sha_maps_commit_rate_limit() {
        let mut mock = MockGhClient::new();
        mock.stub(ref_args("o/r", "main"), ok_resp(ref_body("c0").as_bytes()));
        mock.stub(
            commit_args("o/r", "c0"),
            err_resp("gh: API rate limit exceeded for user XXX. (HTTP 403)"),
        );

        let err = resolve_root_tree_sha(&mock, "o/r", "main").unwrap_err();
        assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
    }

    #[test]
    fn resolve_root_tree_sha_5xx_falls_through_to_http() {
        let mut mock = MockGhClient::new();
        mock.stub(ref_args("o/r", "main"), err_resp("gh: HTTP 503"));

        let err = resolve_root_tree_sha(&mock, "o/r", "main").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("HTTP 503"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn resolve_root_tree_sha_invalid_ref_json_returns_decode_error() {
        let mut mock = MockGhClient::new();
        mock.stub(ref_args("o/r", "main"), ok_resp(b"not json"));

        let err = resolve_root_tree_sha(&mock, "o/r", "main").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("decode refs response"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn resolve_root_tree_sha_invalid_commit_json_returns_decode_error() {
        let mut mock = MockGhClient::new();
        mock.stub(ref_args("o/r", "main"), ok_resp(ref_body("c0").as_bytes()));
        mock.stub(commit_args("o/r", "c0"), ok_resp(b"not json"));

        let err = resolve_root_tree_sha(&mock, "o/r", "main").unwrap_err();
        match err {
            GitlessError::Http(msg) => {
                assert!(msg.contains("decode commits response"), "got: {msg}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }
}
