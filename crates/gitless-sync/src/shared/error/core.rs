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
    Io(#[from] std::io::Error),

    #[error("Partial failure: {failed_count} files could not be hashed")]
    PartialFailure { failed_count: usize },
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
