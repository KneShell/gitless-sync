use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
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

/// One element of GraphQL response `errors[]` array (per `spec-error-contracts.md`
/// § GraphQL error mapping).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GraphqlError {
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub extensions: GraphqlErrorExtensions,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GraphqlErrorExtensions {
    #[serde(default)]
    pub code: String,
}

/// Map a non-empty GraphQL `errors[]` list to a [`GitlessError`].
///
/// Per `spec-error-contracts.md` § GraphQL error mapping, classification keys
/// off `errors[0].extensions.code` (exact match, not substring). Unknown
/// codes — `NOT_FOUND`, `INTERNAL_SERVER_ERROR`, etc. — fall through to
/// `Http` with the original messages preserved so the operator still sees
/// the GitHub-side reason.
///
/// # Panics
/// Never. The caller is expected to guard against an empty slice; if one
/// slips through, the function still returns an `Http` variant with a
/// diagnostic message rather than panicking.
#[must_use]
pub fn map_graphql_error(errors: &[GraphqlError]) -> GitlessError {
    let Some(first) = errors.first() else {
        return GitlessError::Http("graphql: empty errors list".to_string());
    };
    match first.extensions.code.as_str() {
        "RATE_LIMITED" => GitlessError::RateLimitExceeded {
            reset_at: String::new(),
        },
        "UNAUTHENTICATED" => GitlessError::AuthFailed,
        _ => GitlessError::Http(format_graphql_errors(errors)),
    }
}

fn format_graphql_errors(errors: &[GraphqlError]) -> String {
    let mut out = String::new();
    for (i, err) in errors.iter().enumerate() {
        if i > 0 {
            out.push_str("; ");
        }
        if err.extensions.code.is_empty() {
            out.push_str(&err.message);
        } else {
            let _ = write!(out, "[{}] {}", err.extensions.code, err.message);
        }
    }
    out
}
