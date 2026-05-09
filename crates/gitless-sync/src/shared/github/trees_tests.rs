//! Test sibling for `trees.rs`. Loaded via
//! `#[cfg(test)] #[path = "trees_tests.rs"] mod tests;` so the test LOC
//! stays out of the 300-LOC implementation-file gate (Phase 6).

use super::*;
use crate::shared::gh::{GhResponse, MockGhClient};

fn ok_resp(stdout: &[u8]) -> GhResponse {
    GhResponse {
        stdout: stdout.to_vec(),
        stderr: String::new(),
        exit_code: 0,
    }
}

fn err_resp(stderr: &str) -> GhResponse {
    GhResponse {
        stdout: Vec::new(),
        stderr: stderr.to_string(),
        exit_code: 1,
    }
}

fn tree_args(repo: &str, branch: &str) -> Vec<String> {
    vec![
        "api".to_string(),
        format!("repos/{repo}/git/trees/{branch}?recursive=1"),
    ]
}

fn ok_tree_body() -> &'static str {
    r#"{
        "sha": "root",
        "url": "ignored",
        "tree": [
            {"path": "README.md", "mode": "100644", "type": "blob", "sha": "sha1", "size": 100, "url": "u1"},
            {"path": "src", "mode": "040000", "type": "tree", "sha": "tsha", "url": "u2"},
            {"path": "src/main.rs", "mode": "100644", "type": "blob", "sha": "sha2", "size": 200, "url": "u3"}
        ],
        "truncated": false
    }"#
}

#[test]
fn fetch_tree_returns_blob_entries_only() {
    let mut mock = MockGhClient::new();
    mock.stub(
        tree_args("owner/repo", "main"),
        ok_resp(ok_tree_body().as_bytes()),
    );

    let files = fetch_tree(&mock, "owner/repo", "main").unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path, "README.md");
    assert_eq!(files[0].sha, "sha1");
    assert_eq!(files[0].mode, "100644");
    assert_eq!(files[1].path, "src/main.rs");
    assert_eq!(files[1].sha, "sha2");
    assert_eq!(files[1].mode, "100644");
}

#[test]
fn fetch_tree_skips_unsupported_modes() {
    // Phase 5 task G admits submodule (`160000`, type=commit). Task H now
    // admits symlink (`120000`, type=blob). J will extend executable
    // (`100755`); for now it remains skipped with the existing
    // unsupported-mode warning.
    let body = r#"{
        "sha": "root",
        "tree": [
            {"path": "ok.md",  "mode": "100644", "type": "blob",   "sha": "s1"},
            {"path": "exec.sh","mode": "100755", "type": "blob",   "sha": "s2"},
            {"path": "link",   "mode": "120000", "type": "blob",   "sha": "s3"},
            {"path": "submod", "mode": "160000", "type": "commit", "sha": "s4"}
        ],
        "truncated": false
    }"#;
    let mut mock = MockGhClient::new();
    mock.stub(tree_args("o/r", "main"), ok_resp(body.as_bytes()));

    let files = fetch_tree(&mock, "o/r", "main").unwrap();
    assert_eq!(files.len(), 3);
    assert_eq!(files[0].path, "ok.md");
    assert_eq!(files[0].mode, "100644");
    assert_eq!(files[1].path, "link");
    assert_eq!(files[1].mode, "120000");
    assert_eq!(files[1].sha, "s3");
    assert_eq!(files[2].path, "submod");
    assert_eq!(files[2].mode, "160000");
    assert_eq!(files[2].sha, "s4");
}

#[test]
fn fetch_tree_carries_symlink_mode_and_target_blob_sha() {
    // Symlinks report `type: "blob"` + `mode: "120000"` and the `sha`
    // points to a blob whose contents are the link target path. Phase 5
    // task H surfaces both through `RemoteFile` so `compare.rs` can
    // promote the path to `Status::Failed` + `failed_reason: "symlink"`
    // while preserving the mode bit (spec-domain-pitfalls.md § Symlink).
    let body = r#"{
        "sha":"root",
        "tree":[
            {"path":"link/to/elsewhere","mode":"120000","type":"blob","sha":"feedface"}
        ],
        "truncated":false
    }"#;
    let mut mock = MockGhClient::new();
    mock.stub(tree_args("o/r", "main"), ok_resp(body.as_bytes()));

    let files = fetch_tree(&mock, "o/r", "main").unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "link/to/elsewhere");
    assert_eq!(files[0].mode, "120000");
    assert_eq!(files[0].sha, "feedface");
}

#[test]
fn fetch_tree_carries_submodule_mode_and_pointer_sha() {
    // Submodules report `type: "commit"` + `mode: "160000"` and the `sha`
    // is the pointer commit SHA (not a blob). Phase 5 task G surfaces
    // both through `RemoteFile` so `compare.rs` can promote the path to
    // `Status::Failed` + `failed_reason: "submodule"` while preserving
    // the pointer SHA for the caller (spec-domain-pitfalls.md § Submodule).
    let body = r#"{
        "sha":"root",
        "tree":[
            {"path":"vendor/lib","mode":"160000","type":"commit","sha":"deadbeefcafe"}
        ],
        "truncated":false
    }"#;
    let mut mock = MockGhClient::new();
    mock.stub(tree_args("o/r", "main"), ok_resp(body.as_bytes()));

    let files = fetch_tree(&mock, "o/r", "main").unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "vendor/lib");
    assert_eq!(files[0].mode, "160000");
    assert_eq!(files[0].sha, "deadbeefcafe");
}

#[test]
fn fetch_tree_skips_commit_with_non_submodule_mode() {
    // Belt-and-suspenders: a `type: "commit"` entry that somehow lacks the
    // `160000` mode bit is ignored — only the canonical submodule shape
    // promotes through. Defends against malformed responses.
    let body = r#"{
        "sha":"root",
        "tree":[
            {"path":"ok.md","mode":"100644","type":"blob","sha":"s1"},
            {"path":"weird","mode":"100644","type":"commit","sha":"s2"}
        ],
        "truncated":false
    }"#;
    let mut mock = MockGhClient::new();
    mock.stub(tree_args("o/r", "main"), ok_resp(body.as_bytes()));

    let files = fetch_tree(&mock, "o/r", "main").unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "ok.md");
}

#[test]
fn fetch_tree_truncated_returns_error() {
    let body = r#"{"sha":"x","tree":[],"truncated":true}"#;
    let mut mock = MockGhClient::new();
    mock.stub(tree_args("o/r", "main"), ok_resp(body.as_bytes()));

    let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
    assert!(matches!(err, GitlessError::TreesTruncated));
    assert_eq!(err.exit_code(), 5);
}

#[test]
fn fetch_tree_auth_failed_when_bad_credentials_stderr() {
    let mut mock = MockGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        err_resp("gh: Bad credentials (HTTP 401)"),
    );

    let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
    assert!(matches!(err, GitlessError::AuthFailed));
    assert_eq!(err.exit_code(), 2);
}

#[test]
fn fetch_tree_rate_limit_when_primary_stderr() {
    let mut mock = MockGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        err_resp("gh: API rate limit exceeded for user XXX. (HTTP 403)"),
    );

    let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
    match err {
        GitlessError::RateLimitExceeded { reset_at } => {
            assert_eq!(reset_at, "");
        }
        other => panic!("expected RateLimitExceeded, got {other:?}"),
    }
}

#[test]
fn fetch_tree_rate_limit_when_secondary_stderr() {
    let mut mock = MockGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        err_resp("gh: You have exceeded a secondary rate limit ... (HTTP 403)"),
    );

    let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
    assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
    assert_eq!(err.exit_code(), 3);
}

#[test]
fn fetch_tree_http_5xx_falls_through_to_http() {
    let mut mock = MockGhClient::new();
    mock.stub(tree_args("o/r", "main"), err_resp("gh: HTTP 503"));

    let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
    match err {
        GitlessError::Http(msg) => assert!(msg.contains("HTTP 503"), "got: {msg}"),
        other => panic!("expected Http, got {other:?}"),
    }
}

#[test]
fn fetch_tree_unknown_stderr_falls_through_to_http() {
    let mut mock = MockGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        err_resp("gh: Not Found (HTTP 404)"),
    );

    let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
    match err {
        GitlessError::Http(msg) => assert!(msg.contains("HTTP 404"), "got: {msg}"),
        other => panic!("expected Http, got {other:?}"),
    }
}

#[test]
fn fetch_tree_invalid_json_returns_http() {
    let mut mock = MockGhClient::new();
    mock.stub(tree_args("o/r", "main"), ok_resp(b"not json at all"));

    let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
    match err {
        GitlessError::Http(msg) => assert!(msg.contains("decode trees"), "got: {msg}"),
        other => panic!("expected Http, got {other:?}"),
    }
}

#[test]
fn fetch_tree_propagates_client_config_error() {
    let mock = MockGhClient::new();
    let err = fetch_tree(&mock, "o/r", "main").unwrap_err();
    assert!(matches!(err, GitlessError::Http(_)));
}

#[test]
fn fetch_tree_nfc_normalizes_remote_path() {
    // GitHub returns paths exactly as committed. If a file was committed
    // from a macOS shell that emitted NFD bytes, the response carries
    // NFD. We canonicalize to NFC so the comparison key aligns with the
    // walker's NFC output. The decomposed Korean syllable
    // U+1100 + U+1161 is a distinct sequence from the composed
    // form U+AC00 until NFC normalization runs.
    let nfd_path = "\u{1100}\u{1161}.txt";
    let body = format!(
        "{{\"sha\":\"root\",\"tree\":[{{\"path\":\"{nfd_path}\",\"mode\":\"100644\",\"type\":\"blob\",\"sha\":\"s1\"}}],\"truncated\":false}}"
    );
    let mut mock = MockGhClient::new();
    mock.stub(tree_args("o/r", "main"), ok_resp(body.as_bytes()));

    let files = fetch_tree(&mock, "o/r", "main").unwrap();
    assert_eq!(files.len(), 1);
    assert_ne!(files[0].path, nfd_path);
    assert_eq!(files[0].path, "\u{AC00}.txt");
    assert_eq!(files[0].mode, "100644");
}
