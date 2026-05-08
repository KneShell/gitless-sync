//! Network-layer error helpers — GraphQL response mapping.
//!
//! REST-side `gh` subprocess stderr substring matching lives at the call
//! sites in `shared/github/error_map.rs`. This module owns the GraphQL
//! response error shape (`errors[].extensions.code`) and its mapping to
//! [`GitlessError`].

use std::fmt::Write as _;

use serde::Deserialize;

use super::core::GitlessError;

/// One element of a GraphQL response `errors[]` array (per
/// `spec-error-contracts.md` § GraphQL error mapping).
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
/// Per `spec-error-contracts.md` § GraphQL error mapping, classification
/// keys off `errors[0].extensions.code` (exact match, not substring).
/// Unknown codes — `NOT_FOUND`, `INTERNAL_SERVER_ERROR`, etc. — fall
/// through to `Http` with the original messages preserved so the operator
/// still sees the GitHub-side reason. An empty slice slips through as an
/// `Http` diagnostic rather than panicking.
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
