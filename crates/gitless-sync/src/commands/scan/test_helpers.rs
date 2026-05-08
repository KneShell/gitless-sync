//! Shared test fixtures for the `scan` slice.
//!
//! Declared in `mod.rs` as `#[cfg(test)] mod test_helpers;` — only compiled
//! for `cargo test`. Sub-modules import from here via
//! `use super::super::test_helpers::*;` (or `use super::test_helpers::*;`
//! from `mod.rs`).

use std::path::Path;

use chrono::{DateTime, TimeZone, Utc};

use crate::shared::gh::{GhResponse, MockGhClient};

use super::args::{Backend, ScanArgs};

pub(super) const COMMITS_BODY: &str = r#"[{
    "sha": "c1",
    "commit": {
        "author":    {"name": "a", "email": "a@e", "date": "2024-01-15T09:00:00Z"},
        "committer": {"name": "c", "email": "c@e", "date": "2024-01-15T10:30:00Z"},
        "message": "msg"
    },
    "url": "u"
}]"#;

pub(super) fn args_for(dir: &Path, repo: Option<&str>) -> ScanArgs {
    ScanArgs {
        repo: repo.map(String::from),
        branch: "main".to_string(),
        local: dir.to_str().unwrap().to_string(),
        ignore: vec![],
        keep_bom: false,
        pretty: false,
        summary_only: false,
        status: None,
        backend: Backend::Rest,
        verbose: 0,
    }
}

pub(super) fn mtime(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).unwrap()
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

pub(super) fn commits_args(repo: &str, branch: &str, path: &str) -> Vec<String> {
    vec![
        "api".to_string(),
        "-X".to_string(),
        "GET".to_string(),
        format!("repos/{repo}/commits"),
        "-F".to_string(),
        format!("sha={branch}"),
        "-F".to_string(),
        format!("path={path}"),
        "-F".to_string(),
        "per_page=1".to_string(),
    ]
}

pub(super) fn stub_tree(mock: &mut MockGhClient, repo: &str, branch: &str, body: &str) {
    mock.stub(tree_args(repo, branch), ok_resp(body.as_bytes()));
}

pub(super) fn stub_commits(
    mock: &mut MockGhClient,
    repo: &str,
    branch: &str,
    path: &str,
    body: &str,
) {
    mock.stub(commits_args(repo, branch, path), ok_resp(body.as_bytes()));
}
