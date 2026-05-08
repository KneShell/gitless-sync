//! GraphQL response decoding for `fetch_last_commit_at_batch`.
//!
//! Splits envelope traversal + alias-keyed `committedDate` extraction out of
//! the chunk loop in `batch.rs` so that error-mapping tests can hit the parse
//! logic on raw bytes without stubbing a `MockGhClient`. Per ADR 0005/0006 +
//! `spec-error-contracts.md` § GraphQL error mapping: `errors[]` non-empty
//! routes through [`map_graphql_error`]; transport / decode / envelope-shape
//! failures map to [`GitlessError::Http`].

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::shared::error::{GitlessError, GraphqlError, map_graphql_error};

/// Decode one chunk's GraphQL response and merge the alias → `committedDate`
/// pairs into `out`.
///
/// `chunk` is the slice of paths that produced this `body` — alias `a{i}`
/// maps to `chunk[i]`. `branch` is used only to format the
/// "ref refs/heads/{branch} target missing" error message.
///
/// # Errors
/// - [`GitlessError::AuthFailed`] / [`GitlessError::RateLimitExceeded`] /
///   [`GitlessError::Http`] when `errors[]` is non-empty (mapped via
///   [`map_graphql_error`]).
/// - [`GitlessError::Http`] for JSON decode failures, missing
///   `data.repository.ref.target` envelope, missing alias entries, empty
///   `nodes` lists (no commits found for a path), or unparseable
///   `committedDate` values.
pub(super) fn parse_chunk(
    body: &[u8],
    chunk: &[String],
    branch: &str,
    out: &mut HashMap<String, DateTime<Utc>>,
) -> Result<(), GitlessError> {
    let resp: GraphqlResponse = serde_json::from_slice(body)
        .map_err(|e| GitlessError::Http(format!("decode graphql response: {e}")))?;

    if let Some(errors) = resp.errors.as_ref()
        && !errors.is_empty()
    {
        return Err(map_graphql_error(errors));
    }

    let target = resp
        .data
        .and_then(|d| d.repository)
        .and_then(|r| r.git_ref)
        .and_then(|r| r.target)
        .ok_or_else(|| {
            GitlessError::Http(format!(
                "graphql: ref refs/heads/{branch} target missing in response"
            ))
        })?;

    for (i, path) in chunk.iter().enumerate() {
        let alias = format!("a{i}");
        let history = target.aliases.get(&alias).ok_or_else(|| {
            GitlessError::Http(format!("graphql: missing alias {alias} for path {path}"))
        })?;
        let node = history
            .nodes
            .first()
            .ok_or_else(|| GitlessError::Http(format!("no commits found for path: {path}")))?;
        let dt = DateTime::parse_from_rfc3339(&node.committed_date)
            .map_err(|e| GitlessError::Http(format!("parse graphql commit date: {e}")))?
            .with_timezone(&Utc);
        out.insert(path.clone(), dt);
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct GraphqlResponse {
    data: Option<GraphqlData>,
    #[serde(default)]
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphqlData {
    repository: Option<GraphqlRepository>,
}

#[derive(Debug, Deserialize)]
struct GraphqlRepository {
    #[serde(rename = "ref")]
    git_ref: Option<GraphqlRef>,
}

#[derive(Debug, Deserialize)]
struct GraphqlRef {
    target: Option<GraphqlTarget>,
}

#[derive(Debug, Deserialize)]
struct GraphqlTarget {
    #[serde(flatten)]
    aliases: HashMap<String, GraphqlHistory>,
}

#[derive(Debug, Deserialize)]
struct GraphqlHistory {
    nodes: Vec<GraphqlCommitNode>,
}

#[derive(Debug, Deserialize)]
struct GraphqlCommitNode {
    #[serde(rename = "committedDate")]
    committed_date: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_one(
        body: &[u8],
        paths: &[&str],
    ) -> Result<HashMap<String, DateTime<Utc>>, GitlessError> {
        let chunk: Vec<String> = paths.iter().map(|s| (*s).to_string()).collect();
        let mut out = HashMap::new();
        parse_chunk(body, &chunk, "main", &mut out)?;
        Ok(out)
    }

    // --- errors[] mapping --------------------------------------------------

    #[test]
    fn rate_limited_extension_code_maps_to_rate_limit_exceeded() {
        let body = br#"{"data":null,"errors":[{"message":"throttled","extensions":{"code":"RATE_LIMITED"}}]}"#;
        let err = parse_one(body, &["a.md"]).unwrap_err();
        assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
    }

    #[test]
    fn unauthenticated_extension_code_maps_to_auth_failed() {
        let body = br#"{"data":null,"errors":[{"message":"login required","extensions":{"code":"UNAUTHENTICATED"}}]}"#;
        let err = parse_one(body, &["a.md"]).unwrap_err();
        assert!(matches!(err, GitlessError::AuthFailed));
    }

    #[test]
    fn not_found_extension_code_falls_through_to_http() {
        let body = br#"{"data":null,"errors":[{"message":"object not found","extensions":{"code":"NOT_FOUND"}}]}"#;
        let err = parse_one(body, &["a.md"]).unwrap_err();
        match err {
            GitlessError::Http(msg) => {
                assert!(msg.contains("NOT_FOUND"), "got: {msg}");
                assert!(msg.contains("object not found"), "got: {msg}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn internal_server_error_falls_through_to_http() {
        let body = br#"{"data":null,"errors":[{"message":"oops","extensions":{"code":"INTERNAL_SERVER_ERROR"}}]}"#;
        let err = parse_one(body, &["a.md"]).unwrap_err();
        match err {
            GitlessError::Http(msg) => {
                assert!(msg.contains("INTERNAL_SERVER_ERROR"), "got: {msg}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn partial_errors_with_data_present_returns_fail() {
        // Some aliases resolved (`data` non-null) but `errors[]` is non-empty.
        // Per spec § Partial errors policy, errors[0] is mapped and the call
        // fails — the partial data is never returned.
        let body = br#"{"data":{"repository":{"ref":{"target":{"a0":{"nodes":[{"committedDate":"2026-05-07T10:00:00Z"}]}}}}},"errors":[{"message":"throttled","extensions":{"code":"RATE_LIMITED"}}]}"#;
        let err = parse_one(body, &["good.md", "bad.md"]).unwrap_err();
        assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
    }

    // --- envelope-shape failures -------------------------------------------

    #[test]
    fn missing_target_returns_http_with_branch_name() {
        let body = br#"{"data":{"repository":{"ref":null}},"errors":[]}"#;
        let err = parse_one(body, &["a.md"]).unwrap_err();
        match err {
            GitlessError::Http(msg) => {
                assert!(msg.contains("refs/heads/main"), "got: {msg}");
                assert!(msg.contains("target missing"), "got: {msg}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn missing_alias_returns_http() {
        // chunk has 2 paths but body returns alias only for a0.
        let body = br#"{"data":{"repository":{"ref":{"target":{"a0":{"nodes":[{"committedDate":"2026-05-07T10:00:00Z"}]}}}}},"errors":[]}"#;
        let err = parse_one(body, &["a.md", "b.md"]).unwrap_err();
        match err {
            GitlessError::Http(msg) => {
                assert!(msg.contains("missing alias a1"), "got: {msg}");
                assert!(msg.contains("b.md"), "got: {msg}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn empty_nodes_returns_http() {
        let body = br#"{"data":{"repository":{"ref":{"target":{"a0":{"nodes":[]}}}}},"errors":[]}"#;
        let err = parse_one(body, &["a.md"]).unwrap_err();
        match err {
            GitlessError::Http(msg) => {
                assert!(msg.contains("no commits found"), "got: {msg}");
                assert!(msg.contains("a.md"), "got: {msg}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn invalid_committed_date_returns_http() {
        let body = br#"{"data":{"repository":{"ref":{"target":{"a0":{"nodes":[{"committedDate":"not-a-date"}]}}}}},"errors":[]}"#;
        let err = parse_one(body, &["a.md"]).unwrap_err();
        match err {
            GitlessError::Http(msg) => {
                assert!(msg.contains("parse graphql commit date"), "got: {msg}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn invalid_json_returns_http_decode() {
        let body = b"not json at all";
        let err = parse_one(body, &["a.md"]).unwrap_err();
        match err {
            GitlessError::Http(msg) => {
                assert!(msg.contains("decode graphql response"), "got: {msg}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    // --- happy path direct ------------------------------------------------

    #[test]
    fn parse_chunk_merges_into_out_map() {
        let body = br#"{"data":{"repository":{"ref":{"target":{"a0":{"nodes":[{"committedDate":"2026-05-07T10:00:00Z"}]},"a1":{"nodes":[{"committedDate":"2026-05-07T11:00:00Z"}]}}}}},"errors":[]}"#;
        let chunk: Vec<String> = vec!["a.md".to_string(), "b.md".to_string()];
        let mut out = HashMap::new();
        parse_chunk(body, &chunk, "main", &mut out).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(
            out.get("a.md").unwrap().to_rfc3339(),
            "2026-05-07T10:00:00+00:00"
        );
        assert_eq!(
            out.get("b.md").unwrap().to_rfc3339(),
            "2026-05-07T11:00:00+00:00"
        );
    }
}
