//! gh CLI subprocess wrapper for GitHub API access (ADR 0001 + ADR 0002).
//!
//! Production code injects [`RealGhClient`], which spawns `gh <args>` and
//! returns the captured stdout/stderr/exit-code transparently. Tests inject
//! [`MockGhClient`], which serves canned responses keyed by exact argv match.
//! Mapping of stderr substrings + exit code to [`GitlessError`] variants
//! happens in the callers (e.g. `shared::github::fetch_*`) per
//! `docs/specs/spec-error-contracts.md` § gh 종료 코드 매핑.

use std::process::Command;

use crate::shared::error::GitlessError;

const GH_NOT_FOUND_MESSAGE: &str = "gh CLI not found in PATH; install from https://cli.github.com/";

/// Result of a single `gh` subprocess invocation.
///
/// Returned transparently: callers classify based on `exit_code` + `stderr`
/// substrings per `spec-error-contracts.md`. No interpretation happens here.
#[derive(Debug, Clone)]
pub struct GhResponse {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub exit_code: i32,
}

/// Single-method trait shared by production [`RealGhClient`] and test
/// [`MockGhClient`]. `args` is forwarded verbatim to `gh` (e.g.
/// `["api", "repos/o/r/git/trees/main?recursive=1"]`).
///
/// Integration tests under `tests/` define their own stub implementations
/// of this trait; the in-crate `MockGhClient` is `#[cfg(test)]`-gated for
/// unit tests only.
pub trait GhClient {
    /// Invoke `gh` with the given argv and return the captured response.
    ///
    /// # Errors
    /// Implementations return [`GitlessError::Config`] when the `gh` binary
    /// cannot be found in `PATH`. They never interpret the response — exit
    /// codes and stderr substrings are mapped by the caller per
    /// `spec-error-contracts.md`.
    fn api(&self, args: &[String]) -> Result<GhResponse, GitlessError>;
}

/// Production wrapper around `std::process::Command::new("gh")`.
#[derive(Debug, Default)]
pub struct RealGhClient;

impl RealGhClient {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl GhClient for RealGhClient {
    fn api(&self, args: &[String]) -> Result<GhResponse, GitlessError> {
        let output = Command::new("gh")
            .args(args)
            .output()
            .map_err(|_| GitlessError::Config(GH_NOT_FOUND_MESSAGE.to_string()))?;
        Ok(GhResponse {
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }
}

/// Test-only stub keyed by exact argv match.
///
/// Construct with [`Self::new`], register canned responses with [`Self::stub`].
/// Unmatched calls return [`GitlessError::Http`] so missing stubs surface as
/// test failures instead of silent passes.
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct MockGhClient {
    responses: std::collections::HashMap<Vec<String>, GhResponse>,
}

#[cfg(test)]
impl MockGhClient {
    pub(crate) fn new() -> Self {
        Self {
            responses: std::collections::HashMap::new(),
        }
    }

    pub(crate) fn stub(&mut self, args: Vec<String>, response: GhResponse) {
        self.responses.insert(args, response);
    }
}

#[cfg(test)]
impl GhClient for MockGhClient {
    fn api(&self, args: &[String]) -> Result<GhResponse, GitlessError> {
        match self.responses.get(args) {
            Some(r) => Ok(r.clone()),
            None => Err(GitlessError::Http(format!(
                "MockGhClient: no stub registered for args {args:?}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_response(stdout: &[u8]) -> GhResponse {
        GhResponse {
            stdout: stdout.to_vec(),
            stderr: String::new(),
            exit_code: 0,
        }
    }

    #[test]
    fn gh_response_holds_stdout_stderr_and_exit_code() {
        let r = GhResponse {
            stdout: b"hello".to_vec(),
            stderr: "warn".to_string(),
            exit_code: 1,
        };
        assert_eq!(r.stdout, b"hello");
        assert_eq!(r.stderr, "warn");
        assert_eq!(r.exit_code, 1);
    }

    #[test]
    fn gh_response_clone_is_deep() {
        let original = ok_response(b"data");
        let cloned = original.clone();
        assert_eq!(cloned.stdout, original.stdout);
        assert_eq!(cloned.stderr, original.stderr);
        assert_eq!(cloned.exit_code, original.exit_code);
    }

    #[test]
    fn real_gh_client_new_returns_unit_value() {
        let _client = RealGhClient::new();
    }

    #[test]
    fn mock_gh_client_returns_stubbed_response_for_matching_argv() {
        let mut mock = MockGhClient::new();
        let args = vec!["api".to_string(), "rate_limit".to_string()];
        mock.stub(args.clone(), ok_response(b"{\"limit\":5000}"));

        let resp = mock.api(&args).expect("stub registered");
        assert_eq!(resp.stdout, b"{\"limit\":5000}");
        assert_eq!(resp.stderr, "");
        assert_eq!(resp.exit_code, 0);
    }

    #[test]
    fn mock_gh_client_returns_http_error_for_unstubbed_argv() {
        let mock = MockGhClient::new();
        let err = mock
            .api(&["api".to_string(), "missing".to_string()])
            .unwrap_err();
        match err {
            GitlessError::Http(msg) => {
                assert!(msg.contains("MockGhClient"), "got: {msg}");
                assert!(msg.contains("missing"), "got: {msg}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn mock_gh_client_distinguishes_responses_by_exact_argv() {
        let mut mock = MockGhClient::new();
        mock.stub(vec!["api".to_string(), "a".to_string()], ok_response(b"A"));
        mock.stub(vec!["api".to_string(), "b".to_string()], ok_response(b"B"));

        let a = mock.api(&["api".to_string(), "a".to_string()]).unwrap();
        let b = mock.api(&["api".to_string(), "b".to_string()]).unwrap();
        assert_eq!(a.stdout, b"A");
        assert_eq!(b.stdout, b"B");
    }

    #[test]
    fn mock_gh_client_propagates_stub_stderr_and_exit_code() {
        let mut mock = MockGhClient::new();
        let args = vec!["api".to_string(), "boom".to_string()];
        mock.stub(
            args.clone(),
            GhResponse {
                stdout: Vec::new(),
                stderr: "gh: Bad credentials (HTTP 401)".to_string(),
                exit_code: 1,
            },
        );

        let resp = mock.api(&args).unwrap();
        assert_eq!(resp.exit_code, 1);
        assert!(resp.stderr.contains("Bad credentials"));
    }

    #[test]
    fn gh_client_trait_dispatches_through_dyn_reference() {
        let mut mock = MockGhClient::new();
        let args = vec!["api".to_string(), "x".to_string()];
        mock.stub(args.clone(), ok_response(b"y"));

        let client: &dyn GhClient = &mock;
        let resp = client.api(&args).expect("stub registered");
        assert_eq!(resp.stdout, b"y");
    }

    #[test]
    fn gh_not_found_message_contains_install_hint() {
        let err = GitlessError::Config(GH_NOT_FOUND_MESSAGE.to_string());
        match err {
            GitlessError::Config(msg) => {
                assert!(msg.contains("gh CLI not found"), "got: {msg}");
                assert!(msg.contains("https://cli.github.com/"), "got: {msg}");
            }
            other => panic!("expected Config, got {other:?}"),
        }
    }

    #[test]
    fn real_gh_client_api_smoke_invokes_gh_version() {
        // Relies on the test environment having `gh` installed — required by
        // the project (ADR 0001 + M2a env-check). `gh --version` is hermetic:
        // no GitHub round-trip, no auth, exit 0 with "gh version" prefix.
        let client = RealGhClient::new();
        let resp = client
            .api(&["--version".to_string()])
            .expect("gh --version must succeed in dev/CI env (M2a env-check)");
        assert_eq!(resp.exit_code, 0, "stderr: {}", resp.stderr);
        let stdout = String::from_utf8_lossy(&resp.stdout);
        assert!(
            stdout.contains("gh version"),
            "stdout did not look like `gh --version`: {stdout}"
        );
    }
}
