//! Orchestrator: `GhClient` → error map → parse → classify.
//!
//! The wire-level deserialization lives in [`super::parse`]; mode-arm
//! dispatch lives in [`super::classify`]. This file owns only the
//! `gh api ...trees/{branch}?recursive=1` invocation, error mapping
//! (delegated to [`map_gh_error`]), and the per-entry `filter_map`.

use super::classify::{RemoteFile, classify_tree_entry};
use super::parse::parse_tree_body;
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::github::map_gh_error;

/// Fetch the recursive tree of `branch` via `gh api` subprocess.
///
/// Calls `gh api repos/{repo}/git/trees/{branch}?recursive=1` and
/// converts the response per `spec-github-api.md`. Authentication, rate
/// limiting, and truncation flow through `GhResponse.exit_code` +
/// `stderr` substring matching per `spec-error-contracts.md`.
///
/// # Errors
/// - [`GitlessError::TreesTruncated`] when `truncated == true` (G-002).
/// - [`GitlessError::AuthFailed`] when stderr contains `"Bad credentials"`.
/// - [`GitlessError::RateLimitExceeded`] when stderr contains
///   `"secondary rate limit"` or `"API rate limit exceeded"`.
/// - [`GitlessError::Http`] for parse failures or any other gh failure mode.
pub(crate) fn fetch_tree(
    client: &impl GhClient,
    repo: &str,
    branch: &str,
) -> Result<Vec<RemoteFile>, GitlessError> {
    let args = vec![
        "api".to_string(),
        format!("repos/{repo}/git/trees/{branch}?recursive=1"),
    ];
    let resp = client.api(&args)?;
    if resp.exit_code != 0 {
        return Err(map_gh_error(&resp.stderr));
    }

    let body = parse_tree_body(&resp.stdout)?;
    Ok(body
        .tree
        .into_iter()
        .filter_map(classify_tree_entry)
        .collect())
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

    fn tree_args(repo: &str, branch: &str) -> Vec<String> {
        vec![
            "api".to_string(),
            format!("repos/{repo}/git/trees/{branch}?recursive=1"),
        ]
    }

    #[test]
    fn fetch_tree_returns_supported_blob_entries_in_input_order() {
        let body = br#"{
            "sha":"root",
            "url":"ignored",
            "tree":[
                {"path":"README.md","mode":"100644","type":"blob","sha":"sha1"},
                {"path":"src","mode":"040000","type":"tree","sha":"tsha"},
                {"path":"src/main.rs","mode":"100644","type":"blob","sha":"sha2"}
            ],
            "truncated":false
        }"#;
        let mut mock = MockGhClient::new();
        mock.stub(tree_args("owner/repo", "main"), ok_resp(body));

        let files = fetch_tree(&mock, "owner/repo", "main").unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "README.md");
        assert_eq!(files[0].sha, "sha1");
        assert_eq!(files[0].mode, "100644");
        assert_eq!(files[1].path, "src/main.rs");
        assert_eq!(files[1].sha, "sha2");
        assert_eq!(files[1].mode, "100644");
    }

    #[test]
    fn fetch_tree_propagates_truncated_from_parse() {
        let body = br#"{"sha":"x","tree":[],"truncated":true}"#;
        let mut mock = MockGhClient::new();
        mock.stub(tree_args("o/r", "main"), ok_resp(body));

        let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
        assert!(matches!(err, GitlessError::TreesTruncated));
        assert_eq!(err.exit_code(), 5);
    }

    #[test]
    fn fetch_tree_maps_bad_credentials_stderr_to_auth_failed() {
        let mut mock = MockGhClient::new();
        mock.stub(
            tree_args("o/r", "main"),
            err_resp("gh: Bad credentials (HTTP 401)"),
        );

        let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
        assert!(matches!(err, GitlessError::AuthFailed));
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn fetch_tree_maps_primary_rate_limit_stderr() {
        let mut mock = MockGhClient::new();
        mock.stub(
            tree_args("o/r", "main"),
            err_resp("gh: API rate limit exceeded for user XXX. (HTTP 403)"),
        );

        let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
        match err {
            GitlessError::RateLimitExceeded { reset_at } => {
                assert_eq!(reset_at, "");
            }
            other => panic!("expected RateLimitExceeded, got {other:?}"),
        }
    }

    #[test]
    fn fetch_tree_maps_secondary_rate_limit_stderr() {
        let mut mock = MockGhClient::new();
        mock.stub(
            tree_args("o/r", "main"),
            err_resp("gh: You have exceeded a secondary rate limit ... (HTTP 403)"),
        );

        let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
        assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn fetch_tree_5xx_stderr_falls_through_to_http() {
        let mut mock = MockGhClient::new();
        mock.stub(tree_args("o/r", "main"), err_resp("gh: HTTP 503"));

        let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("HTTP 503"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_tree_unknown_stderr_falls_through_to_http() {
        let mut mock = MockGhClient::new();
        mock.stub(
            tree_args("o/r", "main"),
            err_resp("gh: Not Found (HTTP 404)"),
        );

        let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("HTTP 404"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_tree_propagates_client_config_error_when_unstubbed() {
        let mock = MockGhClient::new();
        let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
        assert!(matches!(err, GitlessError::Http(_)));
    }
}
