use chrono::{DateTime, Utc};
use serde::Deserialize;
use ureq::Error as UreqError;

use crate::shared::error::GitlessError;

const GITHUB_API_BASE: &str = "https://api.github.com";
const USER_AGENT: &str = "gitless-sync/0.1";

#[derive(Debug, Clone)]
pub struct RemoteFile {
    pub path: String,
    pub sha: String,
    pub mode: String,
    pub size: Option<u64>,
}

/// Fetch the recursive tree of `branch` from a GitHub repository.
///
/// Returns one [`RemoteFile`] per regular blob entry (mode `100644`). Other
/// entry types are skipped: directories (`type == "tree"`) silently, and
/// unsupported modes (`100755`, `120000`, `160000`, …) with a stderr warning
/// (G-010).
///
/// # Errors
/// - [`GitlessError::TreesTruncated`] when the API response sets `truncated: true` (G-002).
/// - [`GitlessError::AuthFailed`] on HTTP 401.
/// - [`GitlessError::RateLimitExceeded`] on HTTP 403 with `X-RateLimit-Remaining: 0`.
/// - [`GitlessError::Http`] for other non-2xx responses, transport failures,
///   and JSON decode errors.
pub fn fetch_tree(repo: &str, branch: &str, token: &str) -> Result<Vec<RemoteFile>, GitlessError> {
    fetch_tree_with_base(GITHUB_API_BASE, repo, branch, token)
}

fn fetch_tree_with_base(
    base: &str,
    repo: &str,
    branch: &str,
    token: &str,
) -> Result<Vec<RemoteFile>, GitlessError> {
    let url = format!("{base}/repos/{repo}/git/trees/{branch}?recursive=1");
    let response = ureq::get(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(map_ureq_error)?;

    let body: TreeResponse = response
        .into_json()
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
                path: entry.path,
                sha: entry.sha,
                mode: entry.mode,
                size: entry.size,
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

pub fn fetch_blob(repo: &str, sha: &str, token: &str) -> Result<Vec<u8>, GitlessError> {
    let _ = (repo, sha, token);
    todo!("GET /repos/{repo}/git/blobs/{sha} -- base64 decode")
}

pub fn fetch_last_commit_at(
    repo: &str,
    branch: &str,
    path: &str,
    token: &str,
) -> Result<DateTime<Utc>, GitlessError> {
    let _ = (repo, branch, path, token);
    todo!("GET /repos/{repo}/commits?sha={branch}&path={path}&per_page=1")
}

fn map_ureq_error(err: UreqError) -> GitlessError {
    match err {
        UreqError::Status(code, response) => match code {
            401 => GitlessError::AuthFailed,
            403 if response.header("x-ratelimit-remaining") == Some("0") => {
                let reset_unix: i64 = response
                    .header("x-ratelimit-reset")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let reset_at = DateTime::<Utc>::from_timestamp(reset_unix, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default();
                GitlessError::RateLimitExceeded { reset_at }
            }
            _ => GitlessError::Http(format!("HTTP {code}: {}", response.status_text())),
        },
        UreqError::Transport(t) => GitlessError::Http(format!("transport: {t}")),
    }
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
    #[serde(default)]
    size: Option<u64>,
}

#[cfg(test)]
mod tests {
    use mockito::{Matcher, Server};

    use super::*;

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
        let mut server = Server::new();
        let mock = server
            .mock("GET", "/repos/owner/repo/git/trees/main")
            .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
            .match_header("authorization", "Bearer token")
            .match_header("user-agent", "gitless-sync/0.1")
            .match_header("accept", "application/vnd.github+json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ok_tree_body())
            .create();

        let files = fetch_tree_with_base(&server.url(), "owner/repo", "main", "token").unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "README.md");
        assert_eq!(files[0].sha, "sha1");
        assert_eq!(files[0].mode, "100644");
        assert_eq!(files[0].size, Some(100));
        assert_eq!(files[1].path, "src/main.rs");
        assert_eq!(files[1].sha, "sha2");

        mock.assert();
    }

    #[test]
    fn fetch_tree_skips_unsupported_mode_entries() {
        let mut server = Server::new();
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
        let _m = server
            .mock("GET", "/repos/o/r/git/trees/main")
            .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
            .with_status(200)
            .with_body(body)
            .create();

        let files = fetch_tree_with_base(&server.url(), "o/r", "main", "tok").unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "ok.md");
    }

    #[test]
    fn fetch_tree_truncated_returns_error() {
        let mut server = Server::new();
        let body = r#"{"sha":"x","tree":[],"truncated":true}"#;
        let _m = server
            .mock("GET", "/repos/o/r/git/trees/main")
            .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
            .with_status(200)
            .with_body(body)
            .create();

        let err = fetch_tree_with_base(&server.url(), "o/r", "main", "t").unwrap_err();
        assert!(matches!(err, GitlessError::TreesTruncated));
        assert_eq!(err.exit_code(), 5);
    }

    #[test]
    fn fetch_tree_401_returns_auth_failed() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/repos/o/r/git/trees/main")
            .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
            .with_status(401)
            .with_body(r#"{"message":"Bad credentials"}"#)
            .create();

        let err = fetch_tree_with_base(&server.url(), "o/r", "main", "t").unwrap_err();
        assert!(matches!(err, GitlessError::AuthFailed));
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn fetch_tree_403_with_zero_remaining_returns_rate_limit() {
        let mut server = Server::new();
        // 1700000000 = 2023-11-14T22:13:20Z
        let _m = server
            .mock("GET", "/repos/o/r/git/trees/main")
            .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
            .with_status(403)
            .with_header("x-ratelimit-remaining", "0")
            .with_header("x-ratelimit-reset", "1700000000")
            .with_body(r#"{"message":"rate limit"}"#)
            .create();

        let err = fetch_tree_with_base(&server.url(), "o/r", "main", "t").unwrap_err();
        match err {
            GitlessError::RateLimitExceeded { reset_at } => {
                assert!(
                    reset_at.starts_with("2023-11-14"),
                    "expected ISO timestamp, got {reset_at}"
                );
            }
            other => panic!("expected RateLimitExceeded, got {other:?}"),
        }
    }

    #[test]
    fn fetch_tree_403_without_zero_remaining_returns_http_error() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/repos/o/r/git/trees/main")
            .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
            .with_status(403)
            .with_header("x-ratelimit-remaining", "42")
            .with_body(r#"{"message":"forbidden but not rate limited"}"#)
            .create();

        let err = fetch_tree_with_base(&server.url(), "o/r", "main", "t").unwrap_err();
        assert!(matches!(err, GitlessError::Http(_)));
    }

    #[test]
    fn fetch_tree_500_returns_http_error() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/repos/o/r/git/trees/main")
            .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
            .with_status(500)
            .with_body("internal error")
            .create();

        let err = fetch_tree_with_base(&server.url(), "o/r", "main", "t").unwrap_err();
        assert!(matches!(err, GitlessError::Http(_)));
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn fetch_tree_invalid_json_returns_http_error() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/repos/o/r/git/trees/main")
            .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
            .with_status(200)
            .with_body("not json at all")
            .create();

        let err = fetch_tree_with_base(&server.url(), "o/r", "main", "t").unwrap_err();
        assert!(matches!(err, GitlessError::Http(_)));
    }

    #[test]
    fn fetch_tree_sends_required_headers() {
        let mut server = Server::new();
        // mockito returns 501 if no matcher matches — so a successful 200 response
        // here proves all match_header constraints were satisfied.
        let mock = server
            .mock("GET", "/repos/o/r/git/trees/main")
            .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
            .match_header("authorization", "Bearer my_secret")
            .match_header("user-agent", "gitless-sync/0.1")
            .match_header("accept", "application/vnd.github+json")
            .with_status(200)
            .with_body(r#"{"sha":"x","tree":[],"truncated":false}"#)
            .create();

        let files = fetch_tree_with_base(&server.url(), "o/r", "main", "my_secret").unwrap();
        assert!(files.is_empty());
        mock.assert();
    }
}
