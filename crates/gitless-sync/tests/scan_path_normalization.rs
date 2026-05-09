#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Path-key normalization scenarios for `gitless-sync scan` (Phase 5 D).
//!
//! Asserts the case-sensitive comparison policy from `spec-classification.md`:
//! `README.md` and `Readme.md` are different path keys even though Windows NTFS
//! treats them as the same file. The pipeline keys local/remote maps with
//! exact-bytes strings, so the two cases must surface as separate entries.

mod common;

use std::fs;

use tempfile::TempDir;

use common::{TestGhClient, args_for, lf_blob_hash, ok_resp, run_to_json, tree_args};

// ---- 케이스 1: 로컬 한 case + 원격 다른 case (같은 내용) → 양쪽 분리 ---------
//
// Linux origin은 `Foo.txt` / `foo.txt`를 따로 박을 수 있고, NTFS는
// case-preserving + case-insensitive로 둘 중 하나만 표면화. 도구는 case-sensitive
// 비교를 박아 한쪽은 local_only_changed, 다른 쪽은 remote_only_changed로 분리.

#[test]
fn case_difference_between_local_and_remote_surfaces_as_separate_paths() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("Foo.txt"), "alpha\n").unwrap();
    let same_sha = lf_blob_hash("alpha\n");

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"foo.txt","mode":"100644","type":"blob","sha":"{same_sha}","size":6}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));
    // Both entries hit the (None, Some) / (Some, None) branches in `classify`,
    // which short-circuits before the Commits API — no commits stub needed.

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["identical"], 0);
    assert_eq!(json["summary"]["local_only_changed"], 1);
    assert_eq!(json["summary"]["remote_only_changed"], 1);
    assert_eq!(json["summary"]["drift"], 0);
    assert_eq!(json["summary"]["failed"], 0);

    let files = json["files"].as_array().unwrap();
    let by_path: std::collections::HashMap<&str, &serde_json::Value> = files
        .iter()
        .map(|e| (e["path"].as_str().unwrap(), e))
        .collect();
    assert_eq!(
        by_path["Foo.txt"]["status"], "local_only_changed",
        "Foo.txt exists locally but not at remote case"
    );
    assert_eq!(
        by_path["foo.txt"]["status"], "remote_only_changed",
        "foo.txt exists remotely but not at local case"
    );
}

// ---- 케이스 2: 원격에 두 case 박힘 + 로컬은 한 case만 박힘 ------------------
//
// Linux origin이 `README.md` / `Readme.md` 둘 다 박을 수 있는 vault. 로컬은
// `README.md`만 박혀있을 때, 일치하는 case는 identical + 다른 case는
// remote_only_changed로 분리.

#[test]
fn remote_with_two_cases_keeps_them_distinct_against_single_local() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("README.md"), "alpha\n").unwrap();
    let upper_sha = lf_blob_hash("alpha\n");
    let lower_sha = lf_blob_hash("different\n");

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"README.md","mode":"100644","type":"blob","sha":"{upper_sha}","size":6}},{{"path":"Readme.md","mode":"100644","type":"blob","sha":"{lower_sha}","size":10}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["identical"], 1);
    assert_eq!(json["summary"]["remote_only_changed"], 1);
    assert_eq!(json["summary"]["local_only_changed"], 0);
    assert_eq!(json["summary"]["drift"], 0);
    assert_eq!(json["summary"]["failed"], 0);

    let files = json["files"].as_array().unwrap();
    let by_path: std::collections::HashMap<&str, &serde_json::Value> = files
        .iter()
        .map(|e| (e["path"].as_str().unwrap(), e))
        .collect();
    assert_eq!(by_path["README.md"]["status"], "identical");
    assert_eq!(by_path["Readme.md"]["status"], "remote_only_changed");
}
