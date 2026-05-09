use serde::Deserialize;

use super::error_map::map_gh_error;
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::path::to_nfc;

#[derive(Debug, Clone)]
pub struct RemoteFile {
    pub path: String,
    pub sha: String,
}

/// Fetch the recursive tree of `branch` via `gh api` subprocess.
///
/// Calls `gh api repos/{repo}/git/trees/{branch}?recursive=1` and converts the
/// response per `spec-github-api.md`. Authentication, rate limiting, and
/// truncation are observed through `GhResponse.exit_code` + `stderr`
/// substring matching per `spec-error-contracts.md`.
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

    let body: TreeResponse = serde_json::from_slice(&resp.stdout)
        .map_err(|e| GitlessError::Http(format!("decode trees response: {e}")))?;

    if body.truncated {
        return Err(GitlessError::TreesTruncated);
    }

    let mut files = Vec::with_capacity(body.tree.len());
    for entry in body.tree {
        if entry.entry_type != "blob" {
            if entry.entry_type == "commit" {
                eprintln!(
                    "warning: skipping {} (submodule, v0.1 unsupported)",
                    entry.path
                );
            }
            continue;
        }
        if entry.mode == "100644" {
            files.push(RemoteFile {
                path: to_nfc(&entry.path),
                sha: entry.sha,
            });
        } else {
            eprintln!(
                "warning: skipping {} (mode {} unsupported in v0.1)",
                entry.path, entry.mode
            );
        }
    }

    Ok(files)
}

#[derive(Debug, Deserialize)]
struct TreeResponse {
    tree: Vec<TreeEntry>,
    truncated: bool,
}

#[derive(Debug, Deserialize)]
struct TreeEntry {
    path: String,
    mode: String,
    #[serde(rename = "type")]
    entry_type: String,
    sha: String,
}

#[cfg(test)]
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

    fn ok_tree_body() -> &'static str {
        r#"{
            "sha": "root",
            "url": "ignored",
            "tree": [
                {"path": "README.md", "mode": "100644", "type": "blob", "sha": "sha1", "size": 100, "url": "u1"},
                {"path": "src", "mode": "040000", "type": "tree", "sha": "tsha", "url": "u2"},
                {"path": "src/main.rs", "mode": "100644", "type": "blob", "sha": "sha2", "size": 200, "url": "u3"}
            ],
            "truncated": false
        }"#
    }

    #[test]
    fn fetch_tree_returns_blob_entries_only() {
        let mut mock = MockGhClient::new();
        mock.stub(
            tree_args("owner/repo", "main"),
            ok_resp(ok_tree_body().as_bytes()),
        );

        let files = fetch_tree(&mock, "owner/repo", "main").unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "README.md");
        assert_eq!(files[0].sha, "sha1");
        assert_eq!(files[1].path, "src/main.rs");
        assert_eq!(files[1].sha, "sha2");
    }

    #[test]
    fn fetch_tree_skips_unsupported_modes() {
        let body = r#"{
            "sha": "root",
            "tree": [
                {"path": "ok.md",  "mode": "100644", "type": "blob",   "sha": "s1"},
                {"path": "exec.sh","mode": "100755", "type": "blob",   "sha": "s2"},
                {"path": "link",   "mode": "120000", "type": "blob",   "sha": "s3"},
                {"path": "submod", "mode": "160000", "type": "commit", "sha": "s4"}
            ],
            "truncated": false
        }"#;
        let mut mock = MockGhClient::new();
        mock.stub(tree_args("o/r", "main"), ok_resp(body.as_bytes()));

        let files = fetch_tree(&mock, "o/r", "main").unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "ok.md");
    }

    #[test]
    fn fetch_tree_truncated_returns_error() {
        let body = r#"{"sha":"x","tree":[],"truncated":true}"#;
        let mut mock = MockGhClient::new();
        mock.stub(tree_args("o/r", "main"), ok_resp(body.as_bytes()));

        let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
        assert!(matches!(err, GitlessError::TreesTruncated));
        assert_eq!(err.exit_code(), 5);
    }

    #[test]
    fn fetch_tree_auth_failed_when_bad_credentials_stderr() {
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
    fn fetch_tree_rate_limit_when_primary_stderr() {
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
    fn fetch_tree_rate_limit_when_secondary_stderr() {
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
    fn fetch_tree_http_5xx_falls_through_to_http() {
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
    fn fetch_tree_invalid_json_returns_http() {
        let mut mock = MockGhClient::new();
        mock.stub(tree_args("o/r", "main"), ok_resp(b"not json at all"));

        let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("decode trees"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_tree_propagates_client_config_error() {
        let mock = MockGhClient::new();
        let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
        assert!(matches!(err, GitlessError::Http(_)));
    }

    #[test]
    fn fetch_tree_nfc_normalizes_remote_path() {
        // GitHub returns paths exactly as committed. If a file was committed
        // from a macOS shell that emitted NFD bytes, the response carries
        // NFD. We canonicalize to NFC so the comparison key aligns with the
        // walker's NFC output. The decomposed Korean syllable
        // U+1100 + U+1161 is a distinct sequence from the composed
        // form U+AC00 until NFC normalization runs.
        let nfd_path = "\u{1100}\u{1161}.txt";
        let body = format!(
            "{{\"sha\":\"root\",\"tree\":[{{\"path\":\"{nfd_path}\",\"mode\":\"100644\",\"type\":\"blob\",\"sha\":\"s1\"}}],\"truncated\":false}}"
        );
        let mut mock = MockGhClient::new();
        mock.stub(tree_args("o/r", "main"), ok_resp(body.as_bytes()));

        let files = fetch_tree(&mock, "o/r", "main").unwrap();
        assert_eq!(files.len(), 1);
        assert_ne!(files[0].path, nfd_path);
        assert_eq!(files[0].path, "\u{AC00}.txt");
    }
}
