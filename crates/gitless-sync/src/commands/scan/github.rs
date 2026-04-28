use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use ureq::Error as UreqError;

use crate::shared::error::GitlessError;

pub(crate) const GITHUB_API_BASE: &str = "https://api.github.com";
const USER_AGENT: &str = "gitless-sync/0.1";

#[derive(Debug, Clone)]
pub struct RemoteFile {
    pub path: String,
    pub sha: String,
}

/// Fetch the recursive tree of `branch` from a GitHub repository.
///
/// Returns one [`RemoteFile`] per regular blob entry (mode `100644`). Other
/// entry types are skipped: directories (`type == "tree"`) silently, and
/// unsupported modes (`100755`, `120000`, `160000`, …) with a stderr warning
/// (G-010).
///
/// `base` is injectable so integration tests can point at a mockito server.
/// Production callers pass [`GITHUB_API_BASE`].
///
/// # Errors
/// - [`GitlessError::TreesTruncated`] when the API response sets `truncated: true` (G-002).
/// - [`GitlessError::AuthFailed`] on HTTP 401.
/// - [`GitlessError::RateLimitExceeded`] on HTTP 403 with `X-RateLimit-Remaining: 0`.
/// - [`GitlessError::Http`] for other non-2xx responses, transport failures,
///   and JSON decode errors.
pub(crate) fn fetch_tree_with_base(
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

/// Fetch the raw bytes of a single blob by its SHA.
///
/// GitHub returns the blob payload as base64 in the `content` field with
/// `encoding: "base64"`. The response is decoded into raw bytes; whitespace
/// inside the base64 payload (the API line-wraps every 60 chars) is stripped
/// before decoding.
///
/// `base` is injectable so integration tests can point at a mockito server.
/// Production callers pass [`GITHUB_API_BASE`].
///
/// # Errors
/// - [`GitlessError::AuthFailed`] on HTTP 401.
/// - [`GitlessError::RateLimitExceeded`] on HTTP 403 with `X-RateLimit-Remaining: 0`.
/// - [`GitlessError::Http`] for other non-2xx responses, transport failures,
///   unexpected `encoding` values, and base64 decode errors.
pub(crate) fn fetch_blob_with_base(
    base: &str,
    repo: &str,
    sha: &str,
    token: &str,
) -> Result<Vec<u8>, GitlessError> {
    let url = format!("{base}/repos/{repo}/git/blobs/{sha}");
    let response = ureq::get(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(map_ureq_error)?;

    let body: BlobResponse = response
        .into_json()
        .map_err(|e| GitlessError::Http(format!("decode blob response: {e}")))?;

    if body.encoding != "base64" {
        return Err(GitlessError::Http(format!(
            "unexpected blob encoding: {}",
            body.encoding
        )));
    }

    let stripped: String = body
        .content
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    BASE64_STANDARD
        .decode(stripped.as_bytes())
        .map_err(|e| GitlessError::Http(format!("decode blob base64: {e}")))
}

/// Fetch the timestamp of the most recent commit that touched `path` on `branch`.
///
/// Calls `GET /repos/{repo}/commits?sha={branch}&path={path}&per_page=1` and
/// returns the `commit.committer.date` of the first item.
///
/// Callers MUST gate this on a known SHA difference — the Commits API counts
/// against the 5,000 req/h rate limit, so identical files must not trigger a
/// call (G-003). Concurrent invocation across threads is safe; ureq's HTTP
/// path is stateless and the function holds no shared mutable state.
///
/// `base` is injectable so integration tests can point at a mockito server.
/// Production callers pass [`GITHUB_API_BASE`].
///
/// # Errors
/// - [`GitlessError::AuthFailed`] on HTTP 401.
/// - [`GitlessError::RateLimitExceeded`] on HTTP 403 with `X-RateLimit-Remaining: 0`.
/// - [`GitlessError::Http`] for other non-2xx responses, transport failures,
///   JSON decode errors, an empty commits array, or an unparseable date.
pub(crate) fn fetch_last_commit_at_with_base(
    base: &str,
    repo: &str,
    branch: &str,
    path: &str,
    token: &str,
) -> Result<DateTime<Utc>, GitlessError> {
    let url = format!("{base}/repos/{repo}/commits");
    let response = ureq::get(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .query("sha", branch)
        .query("path", path)
        .query("per_page", "1")
        .call()
        .map_err(map_ureq_error)?;

    let body: Vec<CommitItem> = response
        .into_json()
        .map_err(|e| GitlessError::Http(format!("decode commits response: {e}")))?;

    let first = body
        .into_iter()
        .next()
        .ok_or_else(|| GitlessError::Http(format!("no commits found for path: {path}")))?;

    DateTime::parse_from_rfc3339(&first.commit.committer.date)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| GitlessError::Http(format!("parse commit date: {e}")))
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
struct BlobResponse {
    content: String,
    encoding: String,
}

#[derive(Debug, Deserialize)]
struct TreeEntry {
    path: String,
    mode: String,
    #[serde(rename = "type")]
    entry_type: String,
    sha: String,
}

#[derive(Debug, Deserialize)]
struct CommitItem {
    commit: CommitInner,
}

#[derive(Debug, Deserialize)]
struct CommitInner {
    committer: CommitActor,
}

#[derive(Debug, Deserialize)]
struct CommitActor {
    date: String,
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

    #[test]
    fn fetch_blob_decodes_base64_payload() {
        let mut server = Server::new();
        // base64("hello\n") = "aGVsbG8K"
        let body = r#"{
            "sha": "abc",
            "content": "aGVsbG8K",
            "encoding": "base64",
            "size": 6,
            "url": "u"
        }"#;
        let mock = server
            .mock("GET", "/repos/o/r/git/blobs/abc")
            .match_header("authorization", "Bearer t")
            .match_header("user-agent", "gitless-sync/0.1")
            .match_header("accept", "application/vnd.github+json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create();

        let bytes = fetch_blob_with_base(&server.url(), "o/r", "abc", "t").unwrap();
        assert_eq!(bytes, b"hello\n");
        mock.assert();
    }

    #[test]
    fn fetch_blob_strips_whitespace_in_base64() {
        let mut server = Server::new();
        // GitHub line-wraps the base64 payload every 60 chars with `\n`.
        // Split "aGVsbG8K" across newlines to mimic that format.
        let body = "{\
            \"sha\": \"abc\",\
            \"content\": \"aGVs\\nbG8K\\n\",\
            \"encoding\": \"base64\",\
            \"size\": 6,\
            \"url\": \"u\"\
        }";
        let _m = server
            .mock("GET", "/repos/o/r/git/blobs/abc")
            .with_status(200)
            .with_body(body)
            .create();

        let bytes = fetch_blob_with_base(&server.url(), "o/r", "abc", "t").unwrap();
        assert_eq!(bytes, b"hello\n");
    }

    #[test]
    fn fetch_blob_invalid_base64_returns_http_error() {
        let mut server = Server::new();
        let body =
            r#"{"sha":"abc","content":"!!!not-base64!!!","encoding":"base64","size":1,"url":"u"}"#;
        let _m = server
            .mock("GET", "/repos/o/r/git/blobs/abc")
            .with_status(200)
            .with_body(body)
            .create();

        let err = fetch_blob_with_base(&server.url(), "o/r", "abc", "t").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("base64"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_blob_unexpected_encoding_returns_http_error() {
        let mut server = Server::new();
        let body = r#"{"sha":"abc","content":"hello","encoding":"utf-8","size":5,"url":"u"}"#;
        let _m = server
            .mock("GET", "/repos/o/r/git/blobs/abc")
            .with_status(200)
            .with_body(body)
            .create();

        let err = fetch_blob_with_base(&server.url(), "o/r", "abc", "t").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("encoding"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_blob_invalid_json_returns_http_error() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/repos/o/r/git/blobs/abc")
            .with_status(200)
            .with_body("not json at all")
            .create();

        let err = fetch_blob_with_base(&server.url(), "o/r", "abc", "t").unwrap_err();
        assert!(matches!(err, GitlessError::Http(_)));
    }

    #[test]
    fn fetch_blob_401_returns_auth_failed() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/repos/o/r/git/blobs/abc")
            .with_status(401)
            .with_body(r#"{"message":"Bad credentials"}"#)
            .create();

        let err = fetch_blob_with_base(&server.url(), "o/r", "abc", "t").unwrap_err();
        assert!(matches!(err, GitlessError::AuthFailed));
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn fetch_blob_403_with_zero_remaining_returns_rate_limit() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/repos/o/r/git/blobs/abc")
            .with_status(403)
            .with_header("x-ratelimit-remaining", "0")
            .with_header("x-ratelimit-reset", "1700000000")
            .with_body(r#"{"message":"rate limit"}"#)
            .create();

        let err = fetch_blob_with_base(&server.url(), "o/r", "abc", "t").unwrap_err();
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
    fn fetch_blob_500_returns_http_error() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/repos/o/r/git/blobs/abc")
            .with_status(500)
            .with_body("internal error")
            .create();

        let err = fetch_blob_with_base(&server.url(), "o/r", "abc", "t").unwrap_err();
        assert!(matches!(err, GitlessError::Http(_)));
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn fetch_blob_sends_required_headers() {
        let mut server = Server::new();
        let mock = server
            .mock("GET", "/repos/o/r/git/blobs/abc")
            .match_header("authorization", "Bearer my_secret")
            .match_header("user-agent", "gitless-sync/0.1")
            .match_header("accept", "application/vnd.github+json")
            .with_status(200)
            .with_body(r#"{"sha":"abc","content":"","encoding":"base64","size":0,"url":"u"}"#)
            .create();

        let bytes = fetch_blob_with_base(&server.url(), "o/r", "abc", "my_secret").unwrap();
        assert!(bytes.is_empty());
        mock.assert();
    }

    fn ok_commits_body() -> &'static str {
        r#"[
            {
                "sha": "c1",
                "commit": {
                    "author":    {"name": "a", "email": "a@e", "date": "2024-01-15T09:00:00Z"},
                    "committer": {"name": "c", "email": "c@e", "date": "2024-01-15T10:30:00Z"},
                    "message": "msg"
                },
                "url": "u"
            }
        ]"#
    }

    #[test]
    fn fetch_last_commit_at_returns_committer_date() {
        let mut server = Server::new();
        let mock = server
            .mock("GET", "/repos/owner/repo/commits")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("sha".into(), "main".into()),
                Matcher::UrlEncoded("path".into(), "README.md".into()),
                Matcher::UrlEncoded("per_page".into(), "1".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ok_commits_body())
            .create();

        let dt =
            fetch_last_commit_at_with_base(&server.url(), "owner/repo", "main", "README.md", "tok")
                .unwrap();
        assert_eq!(dt.to_rfc3339(), "2024-01-15T10:30:00+00:00");

        mock.assert();
    }

    #[test]
    fn fetch_last_commit_at_empty_array_returns_http_error() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/repos/o/r/commits")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("sha".into(), "main".into()),
                Matcher::UrlEncoded("path".into(), "missing.md".into()),
                Matcher::UrlEncoded("per_page".into(), "1".into()),
            ]))
            .with_status(200)
            .with_body("[]")
            .create();

        let err = fetch_last_commit_at_with_base(&server.url(), "o/r", "main", "missing.md", "t")
            .unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("no commits"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_last_commit_at_invalid_date_returns_http_error() {
        let mut server = Server::new();
        let body = r#"[{"sha":"c1","commit":{"committer":{"name":"c","email":"e","date":"not-a-date"},"author":{"name":"a","email":"e","date":"not-a-date"},"message":"m"},"url":"u"}]"#;
        let _m = server
            .mock("GET", "/repos/o/r/commits")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_body(body)
            .create();

        let err =
            fetch_last_commit_at_with_base(&server.url(), "o/r", "main", "f.md", "t").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("parse commit date"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_last_commit_at_invalid_json_returns_http_error() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/repos/o/r/commits")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_body("not json at all")
            .create();

        let err =
            fetch_last_commit_at_with_base(&server.url(), "o/r", "main", "f.md", "t").unwrap_err();
        assert!(matches!(err, GitlessError::Http(_)));
    }

    #[test]
    fn fetch_last_commit_at_401_returns_auth_failed() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/repos/o/r/commits")
            .match_query(Matcher::Any)
            .with_status(401)
            .with_body(r#"{"message":"Bad credentials"}"#)
            .create();

        let err =
            fetch_last_commit_at_with_base(&server.url(), "o/r", "main", "f.md", "t").unwrap_err();
        assert!(matches!(err, GitlessError::AuthFailed));
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn fetch_last_commit_at_403_with_zero_remaining_returns_rate_limit() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/repos/o/r/commits")
            .match_query(Matcher::Any)
            .with_status(403)
            .with_header("x-ratelimit-remaining", "0")
            .with_header("x-ratelimit-reset", "1700000000")
            .with_body(r#"{"message":"rate limit"}"#)
            .create();

        let err =
            fetch_last_commit_at_with_base(&server.url(), "o/r", "main", "f.md", "t").unwrap_err();
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
    fn fetch_last_commit_at_500_returns_http_error() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/repos/o/r/commits")
            .match_query(Matcher::Any)
            .with_status(500)
            .with_body("internal error")
            .create();

        let err =
            fetch_last_commit_at_with_base(&server.url(), "o/r", "main", "f.md", "t").unwrap_err();
        assert!(matches!(err, GitlessError::Http(_)));
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn fetch_last_commit_at_sends_required_headers_and_query() {
        let mut server = Server::new();
        let mock = server
            .mock("GET", "/repos/o/r/commits")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("sha".into(), "dev".into()),
                Matcher::UrlEncoded("path".into(), "src/lib.rs".into()),
                Matcher::UrlEncoded("per_page".into(), "1".into()),
            ]))
            .match_header("authorization", "Bearer my_secret")
            .match_header("user-agent", "gitless-sync/0.1")
            .match_header("accept", "application/vnd.github+json")
            .with_status(200)
            .with_body(ok_commits_body())
            .create();

        let dt =
            fetch_last_commit_at_with_base(&server.url(), "o/r", "dev", "src/lib.rs", "my_secret")
                .unwrap();
        assert_eq!(dt.to_rfc3339(), "2024-01-15T10:30:00+00:00");
        mock.assert();
    }

    #[test]
    fn fetch_last_commit_at_is_callable_concurrently() {
        let mut server = Server::new();
        let mock = server
            .mock("GET", "/repos/o/r/commits")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("sha".into(), "main".into()),
                Matcher::UrlEncoded("per_page".into(), "1".into()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ok_commits_body())
            .expect_at_least(8)
            .create();

        let base = server.url();
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let base = base.clone();
                std::thread::spawn(move || {
                    let path = format!("file{i}.md");
                    fetch_last_commit_at_with_base(&base, "o/r", "main", &path, "t")
                })
            })
            .collect();

        for handle in handles {
            let dt = handle
                .join()
                .expect("thread panicked")
                .expect("call failed");
            assert_eq!(dt.to_rfc3339(), "2024-01-15T10:30:00+00:00");
        }

        mock.assert();
    }
}
