#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Path-key normalization scenarios for `gitless-sync scan` (Phase 5 D + D1).
//!
//! Asserts the case-sensitive comparison policy from `spec-classification.md`
//! plus the case-collision promotion introduced in D1: when a path appears on
//! exactly one side AND the other side has a different-case sibling, the
//! unmatched path is promoted to `Status::Failed` + `failed_reason:
//! "case_collision"` per `spec-domain-pitfalls.md` § Windows NTFS local-side
//! case detection.

mod common;

use std::fs;

use tempfile::TempDir;

use common::{TestGhClient, args_for, lf_blob_hash, ok_resp, run_to_json, tree_args};

// ---- 케이스 1: 로컬 한 case + 원격 다른 case → 양쪽 case_collision (D1) -----
//
// 로컬 `Foo.txt` + 원격 `foo.txt` 같은 diagonal mismatch는 양쪽 모두 unmatched
// + 상대측에 case-folded sibling이 박혀있는 case. D1은 두 path 모두 Failed +
// failed_reason: "case_collision"로 promote.

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
    // case_collision short-circuits before the Commits API — no commits stub.

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["identical"], 0);
    assert_eq!(json["summary"]["local_only_changed"], 0);
    assert_eq!(json["summary"]["remote_only_changed"], 0);
    assert_eq!(json["summary"]["drift"], 0);
    assert_eq!(json["summary"]["failed"], 2);

    let files = json["files"].as_array().unwrap();
    let by_path: std::collections::HashMap<&str, &serde_json::Value> = files
        .iter()
        .map(|e| (e["path"].as_str().unwrap(), e))
        .collect();
    assert_eq!(by_path["Foo.txt"]["status"], "failed");
    assert_eq!(by_path["Foo.txt"]["failed_reason"], "case_collision");
    assert_eq!(by_path["foo.txt"]["status"], "failed");
    assert_eq!(by_path["foo.txt"]["failed_reason"], "case_collision");
}

// ---- 케이스 2: 원격에 두 case 박힘 + 로컬은 한 case만 박힘 (canonical D1) ---
//
// 원격 `README.md` + `Readme.md` 둘 다 박힌 case + 로컬은 `README.md`만 박힘
// (case-insensitive volume이 두 case 박는 걸 허용 안 함). 일치하는 case는
// identical + 다른 case는 case_collision (D1).

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
    assert_eq!(json["summary"]["remote_only_changed"], 0);
    assert_eq!(json["summary"]["local_only_changed"], 0);
    assert_eq!(json["summary"]["drift"], 0);
    assert_eq!(json["summary"]["failed"], 1);

    let files = json["files"].as_array().unwrap();
    let by_path: std::collections::HashMap<&str, &serde_json::Value> = files
        .iter()
        .map(|e| (e["path"].as_str().unwrap(), e))
        .collect();
    assert_eq!(by_path["README.md"]["status"], "identical");
    assert_eq!(by_path["Readme.md"]["status"], "failed");
    assert_eq!(by_path["Readme.md"]["failed_reason"], "case_collision");
}
