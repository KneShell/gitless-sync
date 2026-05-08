use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::Deserialize;

use super::error_map::map_gh_error;
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;

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

#[derive(Debug, Deserialize)]
struct BlobResponse {
    content: String,
    encoding: String,
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

    fn blob_args(repo: &str, sha: &str) -> Vec<String> {
        vec!["api".to_string(), format!("repos/{repo}/git/blobs/{sha}")]
    }

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
}
