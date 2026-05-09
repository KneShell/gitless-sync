#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Error contract robustness fixture for Phase 5 task R2.
//!
//! Per `spec-error-contracts.md` § Per-file Pitfall Reasons + § Custom Error
//! Types: degraded inputs must remain panic-free. Four scenarios cover the
//! axes called out by the task acceptance:
//!
//! 1. Malformed `.gitattributes` lines (negation, comments, empties) →
//!    parser silently skips per `spec-hash-and-normalize.md` §
//!    `.gitattributes` 파서, scan completes, surrounding valid lines
//!    still apply.
//! 2. Mid-byte-truncated UTF-8 → flows through the hash chain unchanged
//!    (raw-bytes b-policy, `spec-domain-pitfalls.md` § Encoding) and
//!    classifies `Status::Identical` against an equally-byte-equivalent
//!    remote blob.
//! 3. Dangling symlink (Unix) → walker's lstat-only stance emits
//!    `is_symlink=true`; pipeline short-circuits to `Status::Failed` +
//!    `failed_reason: "symlink"` even though the target is missing.
//! 4. Circular symlink (Unix) → `WalkDir::follow_links(false)` skips target
//!    resolution; both endpoints land in the report without infinite
//!    descent.
//!
//! Symlink scenarios are Unix-only because Windows `CreateSymbolicLinkW`
//! requires admin / Developer Mode and would silently skip on stock CI
//! runners. `commands/scan/walker.rs::tests` already gates symlink coverage
//! with `#[cfg(unix)]`; this file mirrors that boundary at the integration
//! layer.

mod common;

use std::fs;

use tempfile::TempDir;

use gitless_sync::shared::hash::blob_hash;

use common::{TestGhClient, args_for, ok_resp, run_to_json, tree_args};

#[cfg(unix)]
const EMPTY_TREE_BODY: &[u8] = br#"{"sha":"x","tree":[],"truncated":false}"#;

#[test]
fn malformed_gitattributes_skips_negation_comments_and_empties_without_panic() {
    // `parse_one_line` silently drops:
    //   * negation patterns (`!foo ...`) — gitattributes(5) forbids them,
    //   * `#`-comment lines,
    //   * blank lines,
    //   * pattern-only lines with no attribute tokens.
    // Robustness here is "scan keeps running" — the parser yields control
    // back to `GitAttributes::load` with the surviving valid rules instead
    // of bubbling a parser error and aborting. We probe both halves: the
    // skip itself doesn't fail, and a trailing valid line still applies
    // (verified by the `Status::Identical` classification on `keep.txt`,
    // whose remote blob hash is computed against the LF-normalized text
    // that the `*.txt text=auto` rule selects).
    let dir = TempDir::new().unwrap();
    let attrs = "!negated text=auto\n# inline comment\n\n   \n*.txt text=auto\n";
    fs::write(dir.path().join(".gitattributes"), attrs).unwrap();
    fs::write(dir.path().join("keep.txt"), "hello\r\n").unwrap();
    let lf_sha = blob_hash(b"hello\n");

    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        ok_resp(
            format!(
                r#"{{"sha":"x","tree":[{{"path":"keep.txt","mode":"100644","type":"blob","sha":"{lf_sha}","size":6}}],"truncated":false}}"#
            )
            .as_bytes(),
        ),
    );

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    // `.gitattributes` itself is local-only (no remote stub for it), so we
    // pin only what the malformed-line robustness contract guarantees: the
    // valid trailing rule still applies and `keep.txt` reaches Identical.
    assert_eq!(json["summary"]["failed"], 0);
    let entries = json["files"].as_array().unwrap();
    let keep = entries
        .iter()
        .find(|e| e["path"] == "keep.txt")
        .expect("keep.txt entry must be present after malformed lines silently skip");
    assert_eq!(
        keep["status"], "identical",
        "valid `*.txt text=auto` line must apply after malformed lines silently skip"
    );
    assert_eq!(keep["local_sha"], lf_sha);
}

#[test]
fn mid_byte_truncated_utf8_local_matches_remote_with_identical_raw_bytes() {
    // EUC-KR-style leading bytes ending mid-character (continuation byte
    // missing). Not valid UTF-8, no NUL bytes, no CR/LF — `prepare_for_hash`
    // unspecified branch passes the raw bytes straight to `blob_hash`. A
    // remote blob with an identical SHA-derived sha therefore classifies
    // `Status::Identical` and the b-policy invariant (raw bytes preserved)
    // holds across the full integration chain.
    let raw: &[u8] = &[0xC7, 0xD1, 0xC7];
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("notes.txt"), raw).unwrap();
    let sha = blob_hash(raw);
    let size = raw.len();

    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        ok_resp(
            format!(
                r#"{{"sha":"x","tree":[{{"path":"notes.txt","mode":"100644","type":"blob","sha":"{sha}","size":{size}}}],"truncated":false}}"#
            )
            .as_bytes(),
        ),
    );

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["identical"], 1);
    assert_eq!(json["summary"]["failed"], 0);
    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    let entry = &files[0];
    assert_eq!(entry["path"], "notes.txt");
    assert_eq!(entry["status"], "identical");
    assert_eq!(entry["local_sha"], sha);
    assert_eq!(entry["remote_sha"], sha);
}

#[cfg(unix)]
#[test]
fn dangling_symlink_local_classifies_failed_with_symlink_reason() {
    // Target does not exist — lstat still succeeds, walker emits
    // `is_symlink: true`, and `pipeline.rs::try_short_circuit_failed`
    // promotes the entry to `Status::Failed` + `failed_reason: "symlink"`
    // before any blob fetch / commits API call.
    use std::os::unix::fs::symlink;

    let dir = TempDir::new().unwrap();
    symlink(dir.path().join("nonexistent"), dir.path().join("link.txt")).unwrap();

    let mut mock = TestGhClient::new();
    mock.stub(tree_args("o/r", "main"), ok_resp(EMPTY_TREE_BODY));

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    let entry = &files[0];
    assert_eq!(entry["path"], "link.txt");
    assert_eq!(entry["status"], "failed");
    assert_eq!(entry["failed_reason"], "symlink");
    assert_eq!(entry["mode"], "120000");
    assert_eq!(json["summary"]["failed"], 1);
}

#[cfg(unix)]
#[test]
fn circular_symlink_local_does_not_loop_and_classifies_both_failed() {
    // Two symlinks pointing at each other. `WalkDir::follow_links(false)`
    // keeps lstat semantics — neither target is resolved, so the walker
    // does not descend into a cycle. Both endpoints land in the report as
    // `Status::Failed` + `failed_reason: "symlink"`.
    use std::os::unix::fs::symlink;

    let dir = TempDir::new().unwrap();
    symlink(dir.path().join("loop_b"), dir.path().join("loop_a")).unwrap();
    symlink(dir.path().join("loop_a"), dir.path().join("loop_b")).unwrap();

    let mut mock = TestGhClient::new();
    mock.stub(tree_args("o/r", "main"), ok_resp(EMPTY_TREE_BODY));

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(json["summary"]["failed"], 2);
    for entry in files {
        assert_eq!(entry["status"], "failed");
        assert_eq!(entry["failed_reason"], "symlink");
        assert_eq!(entry["mode"], "120000");
    }
}
