//! `GitlessError` enum + exit code / error code / stderr payload mapping.
//!
//! All variants are defined here so `exit_code()` / `error_code()` /
//! `to_stderr_payload()` decisions sit next to the data they describe.
//! Domain-specific helpers (GraphQL response mapping, gh subprocess stderr
//! matching) live next door — see `network.rs` and `shared/github/error_map.rs`.

use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GitlessError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Authentication failed")]
    AuthFailed,

    #[error("GitHub API rate limit exceeded (resets at {reset_at})")]
    RateLimitExceeded { reset_at: String },

    #[error("Tree response truncated; repo too large for v0.1")]
    TreesTruncated,

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("I/O error: {0}")]
    Io(std::io::Error),

    /// Downstream consumer closed the output pipe early (e.g. `| head`).
    /// Not a failure — a Unix-style CLI treats this as "seen enough" and
    /// exits cleanly. See issue #25.
    #[error("downstream pipe closed")]
    BrokenPipe,

    #[error("Partial failure: {failed_count} files could not be hashed")]
    PartialFailure { failed_count: usize },
}

// issue #25: a closed output pipe surfaces as `ErrorKind::BrokenPipe` (EPIPE on
// Unix, ERROR_BROKEN_PIPE / os error 109 on Windows). Route it to the dedicated
// `BrokenPipe` variant so the top-level handler can exit 0 instead of treating
// it as a real I/O failure. Every `?` on a write inherits this mapping.
impl From<std::io::Error> for GitlessError {
    fn from(err: std::io::Error) -> Self {
        if err.kind() == std::io::ErrorKind::BrokenPipe {
            Self::BrokenPipe
        } else {
            Self::Io(err)
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StderrPayload<'a> {
    pub error_code: &'a str,
    pub message: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub context: serde_json::Value,
}

impl GitlessError {
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            // BrokenPipe is a normal termination, not an error (issue #25).
            Self::BrokenPipe => 0,
            Self::Config(_) | Self::Io(_) => 1,
            Self::AuthFailed => 2,
            Self::RateLimitExceeded { .. } | Self::Http(_) => 3,
            Self::PartialFailure { .. } => 4,
            Self::TreesTruncated => 5,
        }
    }

    #[must_use]
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Config(_) => "CONFIG_ERROR",
            Self::AuthFailed => "AUTH_FAILED",
            Self::RateLimitExceeded { .. } => "RATE_LIMIT_EXCEEDED",
            Self::TreesTruncated => "TREES_TRUNCATED",
            Self::Http(_) => "HTTP_ERROR",
            Self::Io(_) => "IO_ERROR",
            Self::BrokenPipe => "BROKEN_PIPE",
            Self::PartialFailure { .. } => "PARTIAL_FAILURE",
        }
    }

    #[must_use]
    pub fn to_stderr_payload(&self) -> StderrPayload<'_> {
        let context = match self {
            Self::RateLimitExceeded { reset_at } => {
                serde_json::json!({ "reset_at": reset_at })
            }
            Self::PartialFailure { failed_count } => {
                serde_json::json!({ "failed_count": failed_count })
            }
            _ => serde_json::Value::Null,
        };
        StderrPayload {
            error_code: self.error_code(),
            message: self.to_string(),
            context,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // issue #25: a BrokenPipe io::Error must map to the dedicated variant that
    // exits 0, NOT to a generic Io failure.
    #[test]
    fn broken_pipe_io_error_maps_to_clean_exit() {
        let io_err = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
        let err: GitlessError = io_err.into();
        assert!(
            matches!(err, GitlessError::BrokenPipe),
            "BrokenPipe io::Error must convert to GitlessError::BrokenPipe, got {err:?}"
        );
        assert_eq!(err.exit_code(), 0);
        assert_eq!(err.error_code(), "BROKEN_PIPE");
    }

    // Regression: non-pipe io::Errors must keep the generic Io mapping (exit 1).
    #[test]
    fn other_io_error_keeps_generic_io_mapping() {
        let io_err = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let err: GitlessError = io_err.into();
        assert!(
            matches!(err, GitlessError::Io(_)),
            "non-pipe io::Error must stay GitlessError::Io, got {err:?}"
        );
        assert_eq!(err.exit_code(), 1);
        assert_eq!(err.error_code(), "IO_ERROR");
    }
}
