#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Integration fixture for `.gitattributes` hash routing — Phase 5 task S.
//!
//! K1~K4 + K1.5 unit tests in `shared/normalize.rs::tests` and
//! `shared/gitattributes_*tests.rs` already cover each whitelist branch in
//! isolation. This file exercises the same five branches (text=auto /
//! binary / eol=lf / eol=crlf / unsupported) plus a multi-level depth
//! fixture through the full integration harness — real `.gitattributes`
//! files in tempdir → `GitAttributes::load` → walker → `classify_path` →
//! `prepare_for_hash` → `blob_hash` → compare → output → serialized JSON.
//!
//! Acceptance per `spec-domain-pitfalls.md` § `.gitattributes` 화이트리스트
//! + `spec-hash-and-normalize.md` § Acceptance Criteria:
//! - text=auto: NUL-bearing file forced to LF-normalized text.
//! - binary: NUL-free CRLF file kept as raw bytes (no LF normalize).
//! - eol=lf: CRLF normalized to LF.
//! - eol=crlf: CRLF preserved (raw bytes hashed).
//! - unsupported (`working-tree-encoding=...`): pipeline short-circuits to
//!   `Status::Failed` + `failed_reason: "gitattributes_unsupported"` (Phase
//!   5.13 task AA). `prepare_for_hash` defensively still returns the v0.1
//!   default output — the surface decision lives in `pipeline::
//!   try_short_circuit_failed`'s `.gitattributes` match arm.
//! - multi-level: nested `.gitattributes` overrides root via depth winner
//!   + last-wins precedence (K4).
//!
//! All entries hash to identical local/remote bytes so the SHA-equality
//! skip (G-003) avoids the Commits API. Any commits call would surface as
//! a `TestGhClient: no stub registered` Http error and fail the test —
//! contract-guarded.

mod common;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;

use serde_json::Value;
use tempfile::TempDir;

use common::{TestGhClient, args_for, ok_resp, run_to_json, tree_args};
use gitless_sync::shared::hash::blob_hash;

const ROOT_ATTRS: &str = "\
text-auto.bom text=auto
binary.dat binary
eol-lf.sh eol=lf
eol-crlf.cfg eol=crlf
weird.foo working-tree-encoding=UTF-16
*.txt eol=crlf
";

const NESTED_ATTRS: &str = "*.txt eol=lf\n";

// Per-scenario raw bytes (local content == remote raw). Each path's branch
// is verified via the published local_sha — production prepare_for_hash
// must produce the same bytes the test recomputes inline.
const TEXT_AUTO_RAW: &[u8] = b"a\x00b\r\nc";
const BINARY_RAW: &[u8] = b"hello\r\nworld\r\n";
const EOL_LF_RAW: &[u8] = b"line\r\n";
const EOL_CRLF_RAW: &[u8] = b"hello\r\nworld\r\n";
const UNSUPPORTED_RAW: &[u8] = b"hello\r\nworld\r\n";
const MULTI_ROOT_RAW: &[u8] = b"hello\r\nworld\r\n";
const MULTI_NESTED_RAW: &[u8] = b"hello\r\nworld\r\n";

fn text_auto_expected_hash() -> String {
    blob_hash(b"a\x00b\nc")
}

fn binary_expected_hash() -> String {
    blob_hash(BINARY_RAW)
}

fn eol_lf_expected_hash() -> String {
    blob_hash(b"line\n")
}

fn eol_crlf_expected_hash() -> String {
    blob_hash(EOL_CRLF_RAW)
}

fn unsupported_expected_hash() -> String {
    blob_hash(b"hello\nworld\n")
}

fn multi_root_expected_hash() -> String {
    blob_hash(MULTI_ROOT_RAW)
}

fn multi_nested_expected_hash() -> String {
    blob_hash(b"hello\nworld\n")
}

/// `.gitattributes` itself falls to the default branch (file name doesn't
/// match any whitelisted pattern). LF-only fixtures normalize to themselves.
fn gitattributes_default_hash(content: &str) -> String {
    assert!(
        !content.contains('\r'),
        "fixture must be LF-only so default normalize == raw bytes"
    );
    blob_hash(content.as_bytes())
}

fn trees_body() -> String {
    let entries: [(&str, String, usize); 9] = [
        (
            ".gitattributes",
            gitattributes_default_hash(ROOT_ATTRS),
            ROOT_ATTRS.len(),
        ),
        ("binary.dat", binary_expected_hash(), BINARY_RAW.len()),
        ("eol-crlf.cfg", eol_crlf_expected_hash(), EOL_CRLF_RAW.len()),
        ("eol-lf.sh", eol_lf_expected_hash(), b"line\n".len()),
        (
            "nested/.gitattributes",
            gitattributes_default_hash(NESTED_ATTRS),
            NESTED_ATTRS.len(),
        ),
        (
            "nested/notes.txt",
            multi_nested_expected_hash(),
            b"hello\nworld\n".len(),
        ),
        (
            "notes.txt",
            multi_root_expected_hash(),
            MULTI_ROOT_RAW.len(),
        ),
        (
            "text-auto.bom",
            text_auto_expected_hash(),
            b"a\x00b\nc".len(),
        ),
        (
            "weird.foo",
            unsupported_expected_hash(),
            b"hello\nworld\n".len(),
        ),
    ];
    let mut body = String::from(r#"{"sha":"x","tree":["#);
    for (i, (path, sha, size)) in entries.iter().enumerate() {
        if i > 0 {
            body.push(',');
        }
        let _ = write!(
            body,
            r#"{{"path":"{path}","mode":"100644","type":"blob","sha":"{sha}","size":{size}}}"#
        );
    }
    body.push_str(r#"],"truncated":false}"#);
    body
}

fn scan_with_attributes() -> (TempDir, Value) {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    fs::write(root.join(".gitattributes"), ROOT_ATTRS).unwrap();
    fs::write(root.join("text-auto.bom"), TEXT_AUTO_RAW).unwrap();
    fs::write(root.join("binary.dat"), BINARY_RAW).unwrap();
    fs::write(root.join("eol-lf.sh"), EOL_LF_RAW).unwrap();
    fs::write(root.join("eol-crlf.cfg"), EOL_CRLF_RAW).unwrap();
    fs::write(root.join("weird.foo"), UNSUPPORTED_RAW).unwrap();
    fs::write(root.join("notes.txt"), MULTI_ROOT_RAW).unwrap();

    let nested = root.join("nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join(".gitattributes"), NESTED_ATTRS).unwrap();
    fs::write(nested.join("notes.txt"), MULTI_NESTED_RAW).unwrap();

    let mut mock = TestGhClient::new();
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body().as_bytes()));

    let json = run_to_json(&args_for(root, "o/r"), &mock);
    (dir, json)
}

fn files_by_path(json: &Value) -> HashMap<String, Value> {
    json["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e["path"].as_str().unwrap().to_string(), e.clone()))
        .collect()
}

#[test]
fn text_auto_branch_bypasses_nul_heuristic_and_normalizes_crlf() {
    let (_dir, json) = scan_with_attributes();
    let files = files_by_path(&json);
    let entry = &files["text-auto.bom"];

    assert_eq!(entry["status"], "identical");
    assert_eq!(
        entry["local_sha"],
        text_auto_expected_hash(),
        "text=auto must bypass NUL heuristic and apply LF normalize"
    );
    assert_eq!(entry["local_sha"], entry["remote_sha"]);
    assert_eq!(
        entry["is_binary"], false,
        "text=auto must classify as text even with NUL bytes"
    );
}

#[test]
fn binary_branch_keeps_crlf_raw_for_nul_free_input() {
    let (_dir, json) = scan_with_attributes();
    let files = files_by_path(&json);
    let entry = &files["binary.dat"];

    assert_eq!(entry["status"], "identical");
    assert_eq!(
        entry["local_sha"],
        binary_expected_hash(),
        "binary attribute must preserve CRLF (raw bytes hash)"
    );
    assert_eq!(
        entry["is_binary"], true,
        "binary attribute must mark is_binary=true regardless of NUL bytes"
    );
}

#[test]
fn eol_lf_branch_normalizes_crlf_via_full_pipeline() {
    let (_dir, json) = scan_with_attributes();
    let files = files_by_path(&json);
    let entry = &files["eol-lf.sh"];

    assert_eq!(entry["status"], "identical");
    assert_eq!(entry["local_sha"], eol_lf_expected_hash());
}

#[test]
fn eol_crlf_branch_preserves_crlf_diverging_from_lf_default() {
    let (_dir, json) = scan_with_attributes();
    let files = files_by_path(&json);
    let entry = &files["eol-crlf.cfg"];

    assert_eq!(entry["status"], "identical");
    assert_eq!(
        entry["local_sha"],
        eol_crlf_expected_hash(),
        "eol=crlf must hash raw bytes (CRLF preserved)"
    );
    // Routing pin: eol=crlf hash must differ from the default LF-normalized
    // hash of the same input. If routing regresses to default, this fires.
    assert_ne!(
        entry["local_sha"],
        Value::String(blob_hash(b"hello\nworld\n"))
    );
}

#[test]
fn unsupported_attribute_classifies_as_failed_with_gitattributes_unsupported_reason() {
    // Phase 5.13 task AA: pipeline short-circuits Unsupported attribute paths
    // to `Status::Failed` + `failed_reason: "gitattributes_unsupported"`.
    // `prepare_for_hash` still returns v0.1 default output defensively, but
    // the caller never publishes that hash — Failed entries omit local_sha.
    let (_dir, json) = scan_with_attributes();
    let files = files_by_path(&json);
    let entry = &files["weird.foo"];

    assert_eq!(entry["status"], "failed");
    assert_eq!(entry["failed_reason"], "gitattributes_unsupported");
    let obj = entry.as_object().unwrap();
    assert!(
        !obj.contains_key("local_sha"),
        "Failed entries must omit local_sha (pre_entry_to_file::PreState::Failed)"
    );
    assert!(
        !obj.contains_key("lfs_pointer"),
        "lfs_pointer companion is reserved for failed_reason=lfs_pointer"
    );
}

#[test]
fn multi_level_depth_winner_overrides_root_attribute() {
    // root: *.txt eol=crlf → notes.txt (root) preserves CRLF.
    // nested/.gitattributes: *.txt eol=lf → nested/notes.txt LF-normalized.
    // Both files have identical CRLF raw bytes; depth winner forces
    // different hashes — proving K4 priority is observable end-to-end.
    let (_dir, json) = scan_with_attributes();
    let files = files_by_path(&json);
    let root_entry = &files["notes.txt"];
    let nested_entry = &files["nested/notes.txt"];

    assert_eq!(root_entry["status"], "identical");
    assert_eq!(nested_entry["status"], "identical");
    assert_eq!(root_entry["local_sha"], multi_root_expected_hash());
    assert_eq!(nested_entry["local_sha"], multi_nested_expected_hash());
    assert_ne!(
        root_entry["local_sha"], nested_entry["local_sha"],
        "depth winner must produce different hashes for identical raw input"
    );
}

#[test]
fn gitattributes_aware_scan_reports_all_identical_with_v1_1_envelope() {
    let (_dir, json) = scan_with_attributes();

    assert_eq!(json["schema_version"], "1.4");
    assert_eq!(json["summary"]["identical"], 8);
    assert_eq!(json["summary"]["local_only_changed"], 0);
    assert_eq!(json["summary"]["remote_only_changed"], 0);
    assert_eq!(json["summary"]["drift"], 0);
    // Phase 5.13 AA: weird.foo (working-tree-encoding=UTF-16) is now
    // surfaced as Failed with failed_reason=gitattributes_unsupported.
    assert_eq!(json["summary"]["failed"], 1);

    let files = files_by_path(&json);
    assert_eq!(files.len(), 9);
}
