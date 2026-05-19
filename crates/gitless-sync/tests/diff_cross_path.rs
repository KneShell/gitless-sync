#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Issue #14 — `diff --remote-path` cross-path comparison.
//!
//! Pinned scenarios:
//! - distinct local/remote paths → unified diff headers use each side's key
//!   (`--- a/<remote>` / `+++ b/<local>`)
//! - `--remote-path` value equal to local path → behaves like the option was
//!   omitted (no warning, no error)
//! - `--remote-path` backslash input → normalized to forward slash before
//!   Trees lookup, matching the existing local-side behavior
//!
//! Unit-level header construction is covered by
//! `crates/gitless-sync/src/commands/diff/render.rs` tests; this file pins
//! the full `compute_diff` → `render` pipeline through `run_with_client`.

mod common;

use std::fs;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use tempfile::TempDir;

use gitless_sync::commands::diff::{DiffArgs, run_with_client};

use common::{TestGhClient, ok_resp, tree_args};

fn tree_body_with_blob(path: &str, sha: &str) -> String {
    format!(
        r#"{{"sha":"x","tree":[{{"path":"{path}","mode":"100644","type":"blob","sha":"{sha}","size":1}}],"truncated":false}}"#
    )
}

fn blob_body_for(content: &[u8]) -> String {
    let b64 = BASE64_STANDARD.encode(content);
    format!(r#"{{"sha":"abc","content":"{b64}","encoding":"base64","size":1,"url":"u"}}"#)
}

fn stub_blob(mock: &mut TestGhClient, repo: &str, sha: &str, content: &[u8]) {
    let args = vec!["api".to_string(), format!("repos/{repo}/git/blobs/{sha}")];
    mock.stub(args, ok_resp(blob_body_for(content).as_bytes()));
}

fn args_for(dir: &std::path::Path, path: &str, remote_path: Option<&str>) -> DiffArgs {
    DiffArgs {
        repo: Some("o/r".to_string()),
        branch: "main".to_string(),
        local: dir.to_str().unwrap().to_string(),
        keep_bom: false,
        path: path.to_string(),
        remote_path: remote_path.map(str::to_string),
        json: false,
    }
}

#[test]
fn remote_path_distinct_emits_cross_path_headers() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("local.md"), "alpha\nbeta\n").unwrap();

    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        ok_resp(tree_body_with_blob("remote.md", "shaR").as_bytes()),
    );
    stub_blob(&mut mock, "o/r", "shaR", b"alpha\ngamma\n");

    let args = args_for(dir.path(), "local.md", Some("remote.md"));
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_with_client(&args, &mock, &mut stdout, &mut stderr).expect("run_with_client");

    let s = String::from_utf8(stdout).expect("utf-8 stdout");
    assert!(
        s.contains("--- a/remote.md"),
        "remote-side header missing: {s}"
    );
    assert!(
        s.contains("+++ b/local.md"),
        "local-side header missing: {s}"
    );
}

#[test]
fn remote_path_equal_to_local_matches_default_behavior() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "hello\n").unwrap();

    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        ok_resp(tree_body_with_blob("a.md", "shaSame").as_bytes()),
    );
    stub_blob(&mut mock, "o/r", "shaSame", b"hello\n");

    let args = args_for(dir.path(), "a.md", Some("a.md"));
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_with_client(&args, &mock, &mut stdout, &mut stderr).expect("run_with_client");

    assert!(stderr.is_empty(), "stderr must be silent for identical");
    let s = String::from_utf8(stdout).expect("utf-8 stdout");
    assert!(!s.contains("@@"), "expected no diff hunk, got: {s}");
}

#[test]
fn remote_path_backslash_is_normalized_to_forward_slash() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("local.md"), "x\n").unwrap();

    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        ok_resp(tree_body_with_blob("sub/remote.md", "shaB").as_bytes()),
    );
    stub_blob(&mut mock, "o/r", "shaB", b"x\n");

    let args = args_for(dir.path(), "local.md", Some(r"sub\remote.md"));
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_with_client(&args, &mock, &mut stdout, &mut stderr).expect("run_with_client");

    assert!(
        stderr.is_empty(),
        "backslash input should resolve to remote entry, stderr: {:?}",
        String::from_utf8_lossy(&stderr)
    );
}
