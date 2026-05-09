#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Integration fixtures for Phase 5.13 task AA — three new
//! `failed_reason` plumbings:
//!
//! - `encoding` — local file carrying a UTF-16 BOM. `try_hash_local` reads
//!   the raw bytes once, calls `try_decode_text`, and surfaces
//!   `Some(FailedReason::Encoding)` for `Utf16Bom { .. }`/`Unknown` results.
//! - `nfd_collision` — two on-disk files whose NFD vs NFC raw bytes both
//!   normalize to the same NFC comparison key (`commands/scan/walker.rs`'s
//!   `to_nfc` collapse). The pre-dedup `Vec<LocalFile>` carries both copies;
//!   `nfd_collision::detect` flags the duplicated key.
//! - `gitattributes_unsupported` — covered separately by
//!   `scan_gitattributes.rs::unsupported_attribute_classifies_as_failed_with_gitattributes_unsupported_reason`.
//!
//! Identical entries are crafted so the SHA-equality skip (G-003) avoids the
//! Commits API; any stray Commits call surfaces as `TestGhClient: no stub
//! registered` and fails the test.

mod common;

use std::collections::HashMap;
use std::fs;

use serde_json::Value;
use tempfile::TempDir;

use common::{TestGhClient, args_for, ok_resp, run_to_json, tree_args};

fn files_by_path(json: &Value) -> HashMap<String, Value> {
    json["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["path"].as_str().unwrap().to_string(), f.clone()))
        .collect()
}

#[test]
fn local_utf16_bom_file_classifies_as_failed_with_encoding_reason() {
    // UTF-16 LE BOM (FF FE) is out of v0.2 conversion scope. The hash chain
    // still computes a SHA over raw bytes (b-policy), but try_hash_local
    // surfaces `Some(FailedReason::Encoding)` so pipeline demotes the entry
    // to `Status::Failed` + `failed_reason: "encoding"`. local_sha must be
    // omitted on Failed entries (PreState::Failed branch in pipeline).
    let dir = TempDir::new().unwrap();
    let utf16 = [0xFFu8, 0xFE, b'A', 0];
    fs::write(dir.path().join("weird.txt"), utf16).unwrap();

    let mut mock = TestGhClient::new();
    let trees = r#"{"sha":"x","tree":[{"path":"weird.txt","mode":"100644","type":"blob","sha":"deadbeef","size":4}],"truncated":false}"#;
    mock.stub(tree_args("o/r", "main"), ok_resp(trees.as_bytes()));

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    let files = files_by_path(&json);
    let entry = &files["weird.txt"];

    assert_eq!(entry["status"], "failed");
    assert_eq!(entry["failed_reason"], "encoding");
    let obj = entry.as_object().unwrap();
    assert!(
        !obj.contains_key("local_sha"),
        "Failed entries omit local_sha (pre_entry_to_file::PreState::Failed)"
    );
    assert!(
        !obj.contains_key("lfs_pointer"),
        "lfs_pointer companion is reserved for failed_reason=lfs_pointer"
    );
    assert_eq!(entry["remote_sha"], "deadbeef");
    assert_eq!(json["summary"]["failed"], 1);
    assert_eq!(json["summary"]["identical"], 0);
}

#[test]
fn coexisting_nfd_and_nfc_files_classify_as_failed_with_nfd_collision_reason() {
    // Two raw filename forms (NFD jamo `\u{1100}\u{1161}.txt` and NFC
    // `\u{AC00}.txt`) coexist on disk. NTFS preserves raw bytes — both files
    // are independent. walker normalizes each to the NFC key `가.txt`, so
    // `Vec<LocalFile>` carries two entries for the same key. `nfd_collision::
    // detect` flags the key before the HashMap dedup. The single output
    // entry is promoted to `Status::Failed` + `failed_reason: "nfd_collision"`.
    let dir = TempDir::new().unwrap();
    let nfd = format!("{}{}.txt", '\u{1100}', '\u{1161}');
    let nfc = format!("{}.txt", '\u{AC00}');
    fs::write(dir.path().join(&nfd), b"alpha").unwrap();
    fs::write(dir.path().join(&nfc), b"beta").unwrap();

    let mut mock = TestGhClient::new();
    // Remote tree carries the NFC form once; the path normalization is
    // symmetric on the remote side. SHA value doesn't matter — collision
    // short-circuits before SHA comparison.
    let trees = r#"{"sha":"x","tree":[{"path":"가.txt","mode":"100644","type":"blob","sha":"deadbeef","size":4}],"truncated":false}"#;
    mock.stub(tree_args("o/r", "main"), ok_resp(trees.as_bytes()));

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    let files = files_by_path(&json);
    let entry = &files["가.txt"];

    assert_eq!(entry["status"], "failed");
    assert_eq!(entry["failed_reason"], "nfd_collision");
    let obj = entry.as_object().unwrap();
    assert!(!obj.contains_key("local_sha"));
    assert!(!obj.contains_key("lfs_pointer"));
    assert_eq!(json["summary"]["failed"], 1);
}
