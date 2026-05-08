//! Shared test fixtures for the `diff` slice.
//!
//! Declared in `mod.rs` as `#[cfg(test)] mod test_helpers;` — only compiled
//! for `cargo test`. Sub-modules import via `use super::test_helpers::*;`.

use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;

use crate::shared::gh::{GhResponse, MockGhClient};

use super::args::DiffArgs;

pub(super) fn args_for(dir: &Path, path: &str) -> DiffArgs {
    DiffArgs {
        repo: Some("o/r".to_string()),
        branch: "main".to_string(),
        local: dir.to_str().unwrap().to_string(),
        keep_bom: false,
        path: path.to_string(),
    }
}

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

pub(super) fn tree_args(repo: &str, branch: &str) -> Vec<String> {
    vec![
        "api".to_string(),
        format!("repos/{repo}/git/trees/{branch}?recursive=1"),
    ]
}

pub(super) fn blob_args(repo: &str, sha: &str) -> Vec<String> {
    vec!["api".to_string(), format!("repos/{repo}/git/blobs/{sha}")]
}

pub(super) fn tree_body_with_blob(path: &str, sha: &str) -> String {
    format!(
        r#"{{"sha":"x","tree":[{{"path":"{path}","mode":"100644","type":"blob","sha":"{sha}","size":1}}],"truncated":false}}"#
    )
}

pub(super) fn blob_body_for(content: &[u8]) -> String {
    let b64 = BASE64_STANDARD.encode(content);
    format!(r#"{{"sha":"abc","content":"{b64}","encoding":"base64","size":1,"url":"u"}}"#)
}

pub(super) fn stub_tree(mock: &mut MockGhClient, repo: &str, branch: &str, body: &str) {
    mock.stub(tree_args(repo, branch), ok_resp(body.as_bytes()));
}

pub(super) fn stub_blob(mock: &mut MockGhClient, repo: &str, sha: &str, content: &[u8]) {
    mock.stub(
        blob_args(repo, sha),
        ok_resp(blob_body_for(content).as_bytes()),
    );
}
