#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Empty-file fixture for `gitless-sync scan` (Phase 5 task I).
//!
//! Asserts that a real 0-byte file on disk against a remote empty blob
//! (`e69de29bb2d1d6434b8b29ae775ad8c2e48c5391`, git's well-known empty
//! blob SHA-1) classifies as `Status::Identical`. Pairs with the
//! unit-level `hash::tests::empty_blob_matches_git` constant check by
//! exercising the full walker → compare → output pipeline through the
//! integration harness.

mod common;

use std::fs;

use tempfile::TempDir;

use common::{TestGhClient, args_for, lf_blob_hash, ok_resp, run_to_json, tree_args};

/// Git's empty blob SHA-1 — well-known constant. `blob_hash(&[])` and
/// `lf_blob_hash("")` both reproduce it: no normalization can shift a
/// 0-byte input.
const EMPTY_BLOB_SHA: &str = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";

#[test]
fn lf_blob_hash_empty_matches_git_constant() {
    assert_eq!(lf_blob_hash(""), EMPTY_BLOB_SHA);
}

#[test]
fn local_empty_file_identical_to_remote_empty_blob() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("empty.txt"), b"").unwrap();

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"empty.txt","mode":"100644","type":"blob","sha":"{EMPTY_BLOB_SHA}","size":0}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));
    // Identical entries skip the Commits API (G-003) — no commits stub.

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["identical"], 1);
    assert_eq!(json["summary"]["local_only_changed"], 0);
    assert_eq!(json["summary"]["remote_only_changed"], 0);
    assert_eq!(json["summary"]["drift"], 0);
    assert_eq!(json["summary"]["failed"], 0);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], "empty.txt");
    assert_eq!(files[0]["status"], "identical");
    assert_eq!(files[0]["local_sha"], EMPTY_BLOB_SHA);
    assert_eq!(files[0]["remote_sha"], EMPTY_BLOB_SHA);
}
