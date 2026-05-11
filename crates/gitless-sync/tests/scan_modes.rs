#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Integration fixture for tree-mode short-circuits: submodule (`160000`),
//! symlink (`120000`), and executable (`100755`) Phase 5 task R.
//!
//! Pipeline-level unit tests in `commands/scan/pipeline_tests*.rs` already
//! cover each mode against `assemble_entries` in isolation. This file
//! exercises the same cases through the full integration harness — Trees
//! API JSON parsing → walker → compare → output → serialized JSON — so a
//! regression in any layer between the JSON wire and the published
//! `files[].mode` / `failed_reason` shape surfaces here.
//!
//! Acceptance per `spec-domain-pitfalls.md` § Submodule / Symlink / 실행
//! 권한 + `spec-output-schema.md` § v1.1:
//! - submodule (`160000`) → `Status::Failed` + `failed_reason: "submodule"`
//!   + `mode: "160000"`. Trees entry uses `type: "commit"`.
//! - symlink (`120000`) → `Status::Failed` + `failed_reason: "symlink"`
//!   + `mode: "120000"`. Trees entry uses `type: "blob"`.
//! - executable (`100755`) + content equal → `Status::Identical`
//!   + `mode: "100755"`. Mode bit is reported, not promoted to drift.
//!
//! All three short-circuit before the Commits API: submodule/symlink via
//! `pipeline.rs::try_short_circuit_failed` and the identical executable via
//! the SHA-equality skip (G-003). Hence no commits stub is registered —
//! any commit call would surface as a `TestGhClient: no stub registered`
//! Http error and fail the test, which is part of the contract guarded
//! here.

mod common;

use std::collections::HashMap;
use std::fs;

use serde_json::Value;
use tempfile::TempDir;

use common::{TestGhClient, args_for, lf_blob_hash, ok_resp, run_to_json, tree_args};

const EXECUTABLE_BODY: &str = "#!/bin/sh\necho hi\n";

/// Trees response carrying all three special-mode entries — every test in
/// this file uses this same body so the acceptance "Trees API mock 응답에
/// submodule / symlink / 100755 entry 박음" is met verbatim per scan, with
/// each test focused on one entry's published JSON shape.
fn all_modes_trees_body(executable_sha: &str) -> String {
    format!(
        r#"{{"sha":"x","tree":[{{"path":"build.sh","mode":"100755","type":"blob","sha":"{executable_sha}","size":18}},{{"path":"vendor/lib","mode":"160000","type":"commit","sha":"deadbeefcafe","size":0}},{{"path":"link.txt","mode":"120000","type":"blob","sha":"feedface","size":12}}],"truncated":false}}"#
    )
}

/// Build the scenario's local tempdir + mock + run scan, returning the
/// full envelope. The on-disk executable matches the Trees SHA so the
/// SHA-equality skip fires (G-003); submodule/symlink rely on the mode
/// short-circuits — neither needs a local file or commits stub.
fn scan_all_modes() -> (TempDir, Value) {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("build.sh"), EXECUTABLE_BODY).unwrap();
    let executable_sha = lf_blob_hash(EXECUTABLE_BODY);

    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        ok_resp(all_modes_trees_body(&executable_sha).as_bytes()),
    );

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
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
fn trees_executable_100755_classifies_identical_and_carries_mode_bit() {
    let (_dir, json) = scan_all_modes();
    let files = files_by_path(&json);
    let exec = &files["build.sh"];
    let executable_sha = lf_blob_hash(EXECUTABLE_BODY);

    assert_eq!(exec["status"], "identical");
    assert_eq!(
        exec["mode"], "100755",
        "executable mode bit must propagate to v1.1 JSON without promoting to drift"
    );
    assert_eq!(exec["local_sha"], executable_sha);
    assert_eq!(exec["remote_sha"], executable_sha);
    let obj = exec.as_object().unwrap();
    assert!(
        !obj.contains_key("failed_reason") && !obj.contains_key("lfs_pointer"),
        "Identical entries must omit failed_reason and lfs_pointer"
    );
}

#[test]
fn trees_submodule_160000_classifies_failed_with_reason_and_mode() {
    let (_dir, json) = scan_all_modes();
    let files = files_by_path(&json);
    let entry = &files["vendor/lib"];

    assert_eq!(entry["status"], "failed");
    assert_eq!(entry["failed_reason"], "submodule");
    assert_eq!(entry["mode"], "160000");
    assert_eq!(
        entry["remote_sha"], "deadbeefcafe",
        "submodule pointer commit SHA is preserved for the caller"
    );
    assert!(entry.get("local_sha").is_none_or(Value::is_null));
    assert!(
        !entry.as_object().unwrap().contains_key("lfs_pointer"),
        "submodule entry must omit lfs_pointer (only LFS reason emits it)"
    );
}

#[test]
fn trees_symlink_120000_classifies_failed_with_reason_and_mode() {
    let (_dir, json) = scan_all_modes();
    let files = files_by_path(&json);
    let entry = &files["link.txt"];

    assert_eq!(entry["status"], "failed");
    assert_eq!(entry["failed_reason"], "symlink");
    assert_eq!(entry["mode"], "120000");
    assert_eq!(
        entry["remote_sha"], "feedface",
        "symlink target-blob SHA is preserved for the caller"
    );
    assert!(entry.get("local_sha").is_none_or(Value::is_null));
    assert!(
        !entry.as_object().unwrap().contains_key("lfs_pointer"),
        "symlink entry must omit lfs_pointer"
    );
}

#[test]
fn trees_mode_combo_summary_counts_match_v1_1_classification() {
    // Pins the joint shape: 1 identical (executable, content equal) + 2
    // failed (submodule + symlink). Per-entry detail belongs to the three
    // tests above; this one guards the summary contract so a regression
    // that flips a single mode also surfaces here.
    let (_dir, json) = scan_all_modes();

    assert_eq!(json["schema_version"], "1.4");
    assert_eq!(json["summary"]["identical"], 1);
    assert_eq!(json["summary"]["local_only_changed"], 0);
    assert_eq!(json["summary"]["remote_only_changed"], 0);
    assert_eq!(json["summary"]["drift"], 0);
    assert_eq!(json["summary"]["failed"], 2);

    let files = files_by_path(&json);
    assert_eq!(files.len(), 3);
}
