use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;

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

/// Fetch the raw bytes of a single blob by its SHA via `gh api` subprocess.
///
/// Calls `gh api repos/{repo}/git/blobs/{sha}` and decodes the base64 payload.
/// Whitespace inside the base64 content (the API line-wraps every 60 chars) is
/// stripped before decoding.
///
/// # Errors
/// - [`GitlessError::AuthFailed`] / [`GitlessError::RateLimitExceeded`] /
///   [`GitlessError::Http`] per `spec-error-contracts.md` § gh 종료 코드 매핑.
/// - [`GitlessError::Http`] for unexpected `encoding` values, base64 decode
///   failures, or JSON decode failures.
pub(crate) fn fetch_blob(
    client: &impl GhClient,
    repo: &str,
    sha: &str,
) -> Result<Vec<u8>, GitlessError> {
    let args = vec!["api".to_string(), format!("repos/{repo}/git/blobs/{sha}")];
    let resp = client.api(&args)?;
    if resp.exit_code != 0 {
        return Err(map_gh_error(&resp.stderr));
    }

    let body: BlobResponse = serde_json::from_slice(&resp.stdout)
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
/// Calls `gh api repos/{repo}/commits -F sha={branch} -F path={path} -F per_page=1`
/// and returns the `commit.committer.date` of the first item.
///
/// Callers MUST gate this on a known SHA difference — the Commits API is rate
/// limited (G-003 obsoleted by gh's own rate handling, but the call-frequency
/// guideline survives).
///
/// # Errors
/// - [`GitlessError::AuthFailed`] / [`GitlessError::RateLimitExceeded`] /
///   [`GitlessError::Http`] per `spec-error-contracts.md`.
/// - [`GitlessError::Http`] for an empty commits array, unparseable date, or
///   JSON decode failures.
pub(crate) fn fetch_last_commit_at(
    client: &impl GhClient,
    repo: &str,
    branch: &str,
    path: &str,
) -> Result<DateTime<Utc>, GitlessError> {
    let args = vec![
        "api".to_string(),
        format!("repos/{repo}/commits"),
        "-F".to_string(),
        format!("sha={branch}"),
        "-F".to_string(),
        format!("path={path}"),
        "-F".to_string(),
        "per_page=1".to_string(),
    ];
    let resp = client.api(&args)?;
    if resp.exit_code != 0 {
        return Err(map_gh_error(&resp.stderr));
    }

    let body: Vec<CommitItem> = serde_json::from_slice(&resp.stdout)
        .map_err(|e| GitlessError::Http(format!("decode commits response: {e}")))?;

    let first = body
        .into_iter()
        .next()
        .ok_or_else(|| GitlessError::Http(format!("no commits found for path: {path}")))?;

    DateTime::parse_from_rfc3339(&first.commit.committer.date)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| GitlessError::Http(format!("parse commit date: {e}")))
}

/// Map a non-zero `gh api` invocation's stderr to a [`GitlessError`].
///
/// Substring matches per `spec-error-contracts.md` § gh 종료 코드 매핑. Order
/// follows the spec's explicit priority list (auth → secondary rate → primary
/// rate → fallthrough Http). Unknown stderrs fall through to `Http(stderr)`
/// with the original message preserved.
fn map_gh_error(stderr: &str) -> GitlessError {
    if stderr.contains("Bad credentials") {
        GitlessError::AuthFailed
    } else if stderr.contains("secondary rate limit") || stderr.contains("API rate limit exceeded")
    {
        GitlessError::RateLimitExceeded {
            reset_at: String::new(),
        }
    } else {
        GitlessError::Http(stderr.to_string())
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

    fn blob_args(repo: &str, sha: &str) -> Vec<String> {
        vec!["api".to_string(), format!("repos/{repo}/git/blobs/{sha}")]
    }

    fn commits_args(repo: &str, branch: &str, path: &str) -> Vec<String> {
        vec![
            "api".to_string(),
            format!("repos/{repo}/commits"),
            "-F".to_string(),
            format!("sha={branch}"),
            "-F".to_string(),
            format!("path={path}"),
            "-F".to_string(),
            "per_page=1".to_string(),
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

    // --- fetch_tree --------------------------------------------------------

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

    // --- fetch_blob --------------------------------------------------------

    #[test]
    fn fetch_blob_decodes_base64_payload() {
        // base64("hello\n") = "aGVsbG8K"
        let body = r#"{"sha":"abc","content":"aGVsbG8K","encoding":"base64","size":6,"url":"u"}"#;
        let mut mock = MockGhClient::new();
        mock.stub(blob_args("o/r", "abc"), ok_resp(body.as_bytes()));

        let bytes = fetch_blob(&mock, "o/r", "abc").unwrap();
        assert_eq!(bytes, b"hello\n");
    }

    #[test]
    fn fetch_blob_strips_whitespace_in_base64() {
        // GitHub line-wraps base64 with `\n` every 60 chars. Mimic that here.
        let body = "{\
            \"sha\": \"abc\",\
            \"content\": \"aGVs\\nbG8K\\n\",\
            \"encoding\": \"base64\",\
            \"size\": 6,\
            \"url\": \"u\"\
        }";
        let mut mock = MockGhClient::new();
        mock.stub(blob_args("o/r", "abc"), ok_resp(body.as_bytes()));

        let bytes = fetch_blob(&mock, "o/r", "abc").unwrap();
        assert_eq!(bytes, b"hello\n");
    }

    #[test]
    fn fetch_blob_invalid_base64_returns_http_error() {
        let body =
            r#"{"sha":"abc","content":"!!!not-base64!!!","encoding":"base64","size":1,"url":"u"}"#;
        let mut mock = MockGhClient::new();
        mock.stub(blob_args("o/r", "abc"), ok_resp(body.as_bytes()));

        let err = fetch_blob(&mock, "o/r", "abc").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("base64"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_blob_unexpected_encoding_returns_http_error() {
        let body = r#"{"sha":"abc","content":"hello","encoding":"utf-8","size":5,"url":"u"}"#;
        let mut mock = MockGhClient::new();
        mock.stub(blob_args("o/r", "abc"), ok_resp(body.as_bytes()));

        let err = fetch_blob(&mock, "o/r", "abc").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("encoding"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_blob_invalid_json_returns_http_error() {
        let mut mock = MockGhClient::new();
        mock.stub(blob_args("o/r", "abc"), ok_resp(b"not json at all"));

        let err = fetch_blob(&mock, "o/r", "abc").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("decode blob"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_blob_auth_failed_when_bad_credentials_stderr() {
        let mut mock = MockGhClient::new();
        mock.stub(
            blob_args("o/r", "abc"),
            err_resp("gh: Bad credentials (HTTP 401)"),
        );

        let err = fetch_blob(&mock, "o/r", "abc").unwrap_err();
        assert!(matches!(err, GitlessError::AuthFailed));
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn fetch_blob_rate_limit_when_primary_stderr() {
        let mut mock = MockGhClient::new();
        mock.stub(
            blob_args("o/r", "abc"),
            err_resp("gh: API rate limit exceeded for user XXX. (HTTP 403)"),
        );

        let err = fetch_blob(&mock, "o/r", "abc").unwrap_err();
        assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn fetch_blob_5xx_falls_through_to_http() {
        let mut mock = MockGhClient::new();
        mock.stub(blob_args("o/r", "abc"), err_resp("gh: HTTP 503"));

        let err = fetch_blob(&mock, "o/r", "abc").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("HTTP 503"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_blob_propagates_client_config_error() {
        let mock = MockGhClient::new();
        let err = fetch_blob(&mock, "o/r", "abc").unwrap_err();
        assert!(matches!(err, GitlessError::Http(_)));
    }

    // --- fetch_last_commit_at ---------------------------------------------

    #[test]
    fn fetch_last_commit_at_returns_committer_date() {
        let mut mock = MockGhClient::new();
        mock.stub(
            commits_args("owner/repo", "main", "README.md"),
            ok_resp(ok_commits_body().as_bytes()),
        );

        let dt = fetch_last_commit_at(&mock, "owner/repo", "main", "README.md").unwrap();
        assert_eq!(dt.to_rfc3339(), "2024-01-15T10:30:00+00:00");
    }

    #[test]
    fn fetch_last_commit_at_empty_array_returns_http_error() {
        let mut mock = MockGhClient::new();
        mock.stub(commits_args("o/r", "main", "missing.md"), ok_resp(b"[]"));

        let err = fetch_last_commit_at(&mock, "o/r", "main", "missing.md").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("no commits"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_last_commit_at_invalid_date_returns_http_error() {
        let body = r#"[{"sha":"c1","commit":{"committer":{"name":"c","email":"e","date":"not-a-date"},"author":{"name":"a","email":"e","date":"not-a-date"},"message":"m"},"url":"u"}]"#;
        let mut mock = MockGhClient::new();
        mock.stub(
            commits_args("o/r", "main", "f.md"),
            ok_resp(body.as_bytes()),
        );

        let err = fetch_last_commit_at(&mock, "o/r", "main", "f.md").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("parse commit date"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_last_commit_at_invalid_json_returns_http_error() {
        let mut mock = MockGhClient::new();
        mock.stub(
            commits_args("o/r", "main", "f.md"),
            ok_resp(b"not json at all"),
        );

        let err = fetch_last_commit_at(&mock, "o/r", "main", "f.md").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("decode commits"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_last_commit_at_auth_failed_when_bad_credentials_stderr() {
        let mut mock = MockGhClient::new();
        mock.stub(
            commits_args("o/r", "main", "f.md"),
            err_resp("gh: Bad credentials (HTTP 401)"),
        );

        let err = fetch_last_commit_at(&mock, "o/r", "main", "f.md").unwrap_err();
        assert!(matches!(err, GitlessError::AuthFailed));
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn fetch_last_commit_at_rate_limit_when_primary_stderr() {
        let mut mock = MockGhClient::new();
        mock.stub(
            commits_args("o/r", "main", "f.md"),
            err_resp("gh: API rate limit exceeded for user XXX. (HTTP 403)"),
        );

        let err = fetch_last_commit_at(&mock, "o/r", "main", "f.md").unwrap_err();
        assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn fetch_last_commit_at_5xx_falls_through_to_http() {
        let mut mock = MockGhClient::new();
        mock.stub(
            commits_args("o/r", "main", "f.md"),
            err_resp("gh: HTTP 503"),
        );

        let err = fetch_last_commit_at(&mock, "o/r", "main", "f.md").unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("HTTP 503"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_last_commit_at_propagates_client_config_error() {
        let mock = MockGhClient::new();
        let err = fetch_last_commit_at(&mock, "o/r", "main", "f.md").unwrap_err();
        assert!(matches!(err, GitlessError::Http(_)));
    }

    // --- map_gh_error ------------------------------------------------------

    #[test]
    fn map_gh_error_bad_credentials_returns_auth_failed() {
        let err = map_gh_error("gh: Bad credentials (HTTP 401)");
        assert!(matches!(err, GitlessError::AuthFailed));
    }

    #[test]
    fn map_gh_error_secondary_rate_takes_precedence_over_primary_substring() {
        // Per spec § 매칭 우선순위: secondary checked before primary. Both
        // substrings would match, so the resulting message must still be a
        // RateLimitExceeded — order doesn't change the variant here, but the
        // explicit ordering protects against regressions.
        let err = map_gh_error("gh: secondary rate limit exceeded ...");
        assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
    }

    #[test]
    fn map_gh_error_primary_rate_returns_rate_limit() {
        let err = map_gh_error("gh: API rate limit exceeded for user XXX. (HTTP 403)");
        assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
    }

    #[test]
    fn map_gh_error_unknown_returns_http_with_stderr_preserved() {
        let stderr = "gh: weird thing happened";
        let err = map_gh_error(stderr);
        match err {
            GitlessError::Http(msg) => assert_eq!(msg, stderr),
            other => panic!("expected Http, got {other:?}"),
        }
    }
}
