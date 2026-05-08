use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::error_map::map_gh_error;
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;

/// Fetch the timestamp of the most recent commit that touched `path` on `branch`.
///
/// Calls `gh api -X GET repos/{repo}/commits -F sha={branch} -F path={path} -F per_page=1`
/// and returns the `commit.committer.date` of the first item. The explicit
/// `-X GET` is mandatory: with only `-F` flags `gh` flips the request method
/// to POST, and GitHub's commits endpoint then 404s (G-017).
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
        "-X".to_string(),
        "GET".to_string(),
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

    fn commits_args(repo: &str, branch: &str, path: &str) -> Vec<String> {
        vec![
            "api".to_string(),
            "-X".to_string(),
            "GET".to_string(),
            format!("repos/{repo}/commits"),
            "-F".to_string(),
            format!("sha={branch}"),
            "-F".to_string(),
            format!("path={path}"),
            "-F".to_string(),
            "per_page=1".to_string(),
        ]
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

    /// Intercepts argv from a single `fetch_*` call so a test can assert on
    /// the exact sequence production sent. Returns canned stdout + exit 0 so
    /// the call completes and the captured args are observable.
    struct ArgsCapture {
        seen: std::cell::RefCell<Vec<String>>,
        stdout: Vec<u8>,
    }

    impl ArgsCapture {
        fn with_stdout(stdout: &[u8]) -> Self {
            Self {
                seen: std::cell::RefCell::new(Vec::new()),
                stdout: stdout.to_vec(),
            }
        }
    }

    impl GhClient for ArgsCapture {
        fn api(&self, args: &[String]) -> Result<GhResponse, GitlessError> {
            *self.seen.borrow_mut() = args.to_vec();
            Ok(GhResponse {
                stdout: self.stdout.clone(),
                stderr: String::new(),
                exit_code: 0,
            })
        }
    }

    #[test]
    fn fetch_last_commit_at_prepends_x_get_before_path_arg() {
        // G-017 regression: `gh -F` flips the request method to POST, so the
        // commits API (GET-only) returns 404 unless `-X GET` precedes the path
        // arg. Capture production argv directly and pin the order.
        let cap = ArgsCapture::with_stdout(ok_commits_body().as_bytes());
        let _ = fetch_last_commit_at(&cap, "owner/repo", "main", "README.md").unwrap();

        let args = cap.seen.borrow().clone();
        let api_pos = args.iter().position(|s| s == "api").expect("'api' present");
        let x_pos = args
            .iter()
            .position(|s| s == "-X")
            .expect("'-X' must be present (G-017)");
        let get_pos = args
            .iter()
            .position(|s| s == "GET")
            .expect("'GET' must be present (G-017)");
        let path_pos = args
            .iter()
            .position(|s| s == "repos/owner/repo/commits")
            .expect("path arg present");

        assert_eq!(x_pos, api_pos + 1, "-X must immediately follow 'api'");
        assert_eq!(get_pos, x_pos + 1, "GET must immediately follow '-X'");
        assert!(
            get_pos < path_pos,
            "'-X GET' must precede the path arg, got args = {args:?}"
        );
    }

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
}
