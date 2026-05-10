use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::Deserialize;

use super::error_map::map_gh_error;
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;

/// 100 MB — GitHub Blobs API hard limit (`spec-hash-and-normalize.md`
/// § Phase 7 — 큰 파일 처리, 2026-05-10 fact check). Mirrors
/// `commands/scan/hash_local::FILE_TOO_LARGE_BYTES` since `shared/` cannot
/// import from `commands/`; a future task may extract to `shared/limits.rs`.
const FILE_TOO_LARGE_BYTES: u64 = 100 * 1024 * 1024;

/// 50 MB — tool memory safety threshold (raw bytes + base64 + SHA-1 buffer
/// worst case). Mirrors `commands/scan/hash_local::MEMORY_EXCEEDED_BYTES`.
const MEMORY_EXCEEDED_BYTES: u64 = 50 * 1024 * 1024;

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

/// Fetch a blob by SHA, but short-circuit on the Trees-response size field
/// before any network call. Pre-flight skip avoids both GitHub API budget
/// and tool memory pressure on oversize blobs.
///
/// Boundaries are strict (`>`): a 50 MB-exact blob passes, 50 MB+1 fails;
/// a 100 MB-exact blob fires the memory-threshold arm (over 50, not over
/// 100) — `FileTooLarge` only above 100 MB. Caller maps the `Http` error
/// back to `Status::Failed` + `failed_reason: "file_too_large"` /
/// `"memory_exceeded"` per `spec-hash-and-normalize.md` § 검출 알고리즘.
///
/// # Errors
/// - [`GitlessError::Http`] with prefix `blob {sha} too large:` when
///   `expected_size > 100 MB`.
/// - [`GitlessError::Http`] with prefix `blob {sha} exceeds memory threshold:`
///   when `expected_size > 50 MB` (and within the 100 MB ceiling).
/// - All errors from [`fetch_blob`] when within the threshold.
// Removed in Phase 7.2 task N when `pipeline/hash_pass.rs` (or a sibling
// `hash_remote` shim) plumbs `RemoteFile.size` into the gate.
#[allow(dead_code)]
pub(crate) fn fetch_blob_with_size_gate(
    client: &impl GhClient,
    repo: &str,
    sha: &str,
    expected_size: u64,
) -> Result<Vec<u8>, GitlessError> {
    if expected_size > FILE_TOO_LARGE_BYTES {
        return Err(GitlessError::Http(format!(
            "blob {sha} too large: {expected_size} bytes"
        )));
    }
    if expected_size > MEMORY_EXCEEDED_BYTES {
        return Err(GitlessError::Http(format!(
            "blob {sha} exceeds memory threshold: {expected_size} bytes"
        )));
    }
    fetch_blob(client, repo, sha)
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

    #[test]
    fn fetch_blob_with_size_gate_passes_at_exact_50mb_boundary() {
        // Strict `>` — 50 MB exact does not trip MemoryExceeded.
        let body = r#"{"sha":"abc","content":"aGVsbG8K","encoding":"base64","size":6,"url":"u"}"#;
        let mut mock = MockGhClient::new();
        mock.stub(blob_args("o/r", "abc"), ok_resp(body.as_bytes()));
        let bytes = fetch_blob_with_size_gate(&mock, "o/r", "abc", MEMORY_EXCEEDED_BYTES).unwrap();
        assert_eq!(bytes, b"hello\n");
    }

    #[test]
    fn fetch_blob_with_size_gate_emits_memory_exceeded_just_over_50mb() {
        // Mock unstubbed — a `gh api` invocation would yield a `MockGhClient`
        // error. The size-gate prefix proves zero invocations.
        let mock = MockGhClient::new();
        let n = MEMORY_EXCEEDED_BYTES + 1;
        let err = fetch_blob_with_size_gate(&mock, "o/r", "abc", n).unwrap_err();
        match err {
            GitlessError::Http(msg) => {
                assert!(
                    msg.contains("blob abc exceeds memory threshold"),
                    "got: {msg}"
                );
                assert!(msg.contains(&n.to_string()), "got: {msg}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_blob_with_size_gate_emits_file_too_large_just_over_100mb() {
        let mock = MockGhClient::new();
        let n = FILE_TOO_LARGE_BYTES + 1;
        let err = fetch_blob_with_size_gate(&mock, "o/r", "abc", n).unwrap_err();
        match err {
            GitlessError::Http(msg) => {
                assert!(msg.contains("blob abc too large"), "got: {msg}");
                assert!(msg.contains(&n.to_string()), "got: {msg}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn fetch_blob_with_size_gate_prefers_file_too_large_over_memory_exceeded() {
        // 200 MB is over both thresholds — `FileTooLarge` arm wins per priority.
        let mock = MockGhClient::new();
        let err = fetch_blob_with_size_gate(&mock, "o/r", "abc", 200 * 1024 * 1024).unwrap_err();
        match err {
            GitlessError::Http(msg) => assert!(msg.contains("too large"), "got: {msg}"),
            other => panic!("expected Http, got {other:?}"),
        }
    }
}
