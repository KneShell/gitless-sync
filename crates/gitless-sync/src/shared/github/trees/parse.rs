//! Wire-level deserialization for the GitHub Trees API response.
//!
//! Holds the private struct shape (`TreeResponse` / `TreeEntry`) that
//! mirrors `gh api repos/{owner}/{repo}/git/trees/{branch}?recursive=1`
//! stdout, plus the truncation guard. No `GhClient`, no NFC, no mode-bit
//! dispatch — those live in `super::fetch` and `super::classify`.

use serde::Deserialize;

use crate::shared::error::GitlessError;

#[derive(Debug, Deserialize)]
pub(super) struct TreeResponse {
    pub(super) tree: Vec<TreeEntry>,
    pub(super) truncated: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct TreeEntry {
    pub(super) path: String,
    pub(super) mode: String,
    #[serde(rename = "type")]
    pub(super) entry_type: String,
    pub(super) sha: String,
}

/// Decode `gh api ...trees/{branch}?recursive=1` stdout.
///
/// # Errors
/// - [`GitlessError::TreesTruncated`] when `truncated == true` (G-002).
/// - [`GitlessError::Http`] with prefix `decode trees response:` when
///   serde rejects the JSON body.
pub(super) fn parse_tree_body(stdout: &[u8]) -> Result<TreeResponse, GitlessError> {
    let body: TreeResponse = serde_json::from_slice(stdout)
        .map_err(|e| GitlessError::Http(format!("decode trees response: {e}")))?;

    if body.truncated {
        return Err(GitlessError::TreesTruncated);
    }

    Ok(body)
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
mod tests {
    use super::*;

    #[test]
    fn parse_tree_body_decodes_simple_response_into_struct() {
        let body = br#"{
            "sha":"root",
            "url":"ignored",
            "tree":[
                {"path":"README.md","mode":"100644","type":"blob","sha":"s1","size":100,"url":"u1"}
            ],
            "truncated":false
        }"#;
        let parsed = parse_tree_body(body).unwrap();
        assert!(!parsed.truncated);
        assert_eq!(parsed.tree.len(), 1);
        assert_eq!(parsed.tree[0].path, "README.md");
        assert_eq!(parsed.tree[0].mode, "100644");
        assert_eq!(parsed.tree[0].entry_type, "blob");
        assert_eq!(parsed.tree[0].sha, "s1");
    }

    #[test]
    fn parse_tree_body_surfaces_truncated_as_trees_truncated() {
        let body = br#"{"sha":"x","tree":[],"truncated":true}"#;
        let err = parse_tree_body(body).unwrap_err();
        assert!(matches!(err, GitlessError::TreesTruncated));
    }

    #[test]
    fn parse_tree_body_invalid_json_returns_http_with_decode_prefix() {
        let err = parse_tree_body(b"not json at all").unwrap_err();
        match err {
            GitlessError::Http(msg) => {
                assert!(msg.contains("decode trees response"), "got: {msg}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }
}
