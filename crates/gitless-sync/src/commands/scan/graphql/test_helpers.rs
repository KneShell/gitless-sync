//! Shared test fixtures for `graphql/{batch,parse}.rs` unit tests.
//!
//! `MockGhClient` argv builders + canned `gh api graphql` response shapes.

use std::fmt::Write as _;

use crate::shared::gh::GhResponse;

pub(super) fn ok_resp(body: &[u8]) -> GhResponse {
    GhResponse {
        stdout: body.to_vec(),
        stderr: String::new(),
        exit_code: 0,
    }
}

pub(super) fn err_resp(stderr: &str) -> GhResponse {
    GhResponse {
        stdout: Vec::new(),
        stderr: stderr.to_string(),
        exit_code: 1,
    }
}

pub(super) fn graphql_args(query: &str) -> Vec<String> {
    vec![
        "api".to_string(),
        "graphql".to_string(),
        "-f".to_string(),
        format!("query={query}"),
    ]
}

pub(super) fn ok_response_for(paths: &[(&str, &str)]) -> String {
    let mut alias_entries = String::new();
    for (i, (_, date)) in paths.iter().enumerate() {
        if i > 0 {
            alias_entries.push(',');
        }
        let _ = write!(
            alias_entries,
            r#""a{i}":{{"nodes":[{{"committedDate":"{date}"}}]}}"#
        );
    }
    format!(r#"{{"data":{{"repository":{{"ref":{{"target":{{{alias_entries}}}}}}}}},"errors":[]}}"#)
}
