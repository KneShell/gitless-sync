#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! End-to-end happy-path tests for `gitless-sync scan` (PRD scenarios 1, 2, 3,
//! 4, 9, 13, 14).
//!
//! Covers the 4-state classification, `.gitignore` + `--ignore` 합집합, and
//! the two output-shape variants (`--summary-only`, `--status`). The JSON we
//! parse here is byte-identical to what `run_with_client` writes to stdout in
//! production: both paths run `build_report` followed by `output::serialize`.

mod common;

use std::fs;

use tempfile::TempDir;

use common::{
    TestGhClient, args_for, commits_args, commits_body_with_date, lf_blob_hash, ok_resp,
    read_mtime_rfc3339, run_to_json, tree_args,
};

// ---- PRD 시나리오 1: 양쪽 SHA 동일 → Identical ----------------------------

#[test]
fn scenario_1_identical_when_shas_match() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
    let local_sha = lf_blob_hash("alpha\n");

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_sha}","size":6}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));
    // No commits stub: identical entries skip the Commits API (G-003).

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["identical"], 1);
    assert_eq!(json["summary"]["local_only_changed"], 0);
    assert_eq!(json["summary"]["remote_only_changed"], 0);
    assert_eq!(json["summary"]["drift"], 0);
    assert_eq!(json["summary"]["failed"], 0);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], "a.md");
    assert_eq!(files[0]["status"], "identical");
}

// ---- PRD 시나리오 2: 원격 last_commit < 로컬 mtime → LocalOnlyChanged ------

#[test]
fn scenario_2_local_only_changed_when_remote_commit_older() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha-local\n").unwrap();

    let mut mock = TestGhClient::new();
    let trees_body = r#"{"sha":"x","tree":[{"path":"a.md","mode":"100644","type":"blob","sha":"deadbeef","size":12}],"truncated":false}"#;
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));
    mock.stub(
        commits_args("o/r", "main", "a.md"),
        ok_resp(commits_body_with_date("2020-01-01T00:00:00Z").as_bytes()),
    );

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["local_only_changed"], 1);
    assert_eq!(json["summary"]["identical"], 0);
    assert_eq!(json["summary"]["remote_only_changed"], 0);
    assert_eq!(json["summary"]["drift"], 0);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files[0]["status"], "local_only_changed");
}

// ---- PRD 시나리오 3: 로컬 mtime < 원격 last_commit → RemoteOnlyChanged -----

#[test]
fn scenario_3_remote_only_changed_when_local_mtime_older() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha-local\n").unwrap();

    let mut mock = TestGhClient::new();
    let trees_body = r#"{"sha":"x","tree":[{"path":"a.md","mode":"100644","type":"blob","sha":"deadbeef","size":12}],"truncated":false}"#;
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));
    mock.stub(
        commits_args("o/r", "main", "a.md"),
        ok_resp(commits_body_with_date("2099-01-01T00:00:00Z").as_bytes()),
    );

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["remote_only_changed"], 1);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files[0]["status"], "remote_only_changed");
}

// ---- PRD 시나리오 4: 양쪽 다른 SHA + 시간 동률 → Drift (G-005) -----------

#[test]
fn scenario_4_drift_when_times_tie() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.md");
    fs::write(&path, "alpha-local\n").unwrap();
    let mtime_str = read_mtime_rfc3339(&path);

    let mut mock = TestGhClient::new();
    let trees_body = r#"{"sha":"x","tree":[{"path":"a.md","mode":"100644","type":"blob","sha":"deadbeef","size":12}],"truncated":false}"#;
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));
    mock.stub(
        commits_args("o/r", "main", "a.md"),
        ok_resp(commits_body_with_date(&mtime_str).as_bytes()),
    );

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["drift"], 1);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files[0]["status"], "drift");
}

// ---- PRD 시나리오 9: .gitignore + --ignore 합집합 ------------------------

#[test]
fn scenario_9_gitignore_and_ignore_arg_form_union() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();
    fs::create_dir(dir.path().join("build")).unwrap();
    fs::write(dir.path().join("build").join("artifact.bin"), "x").unwrap();
    fs::write(dir.path().join("debug.log"), "trace").unwrap();
    fs::write(dir.path().join("notes.md"), "alpha\n").unwrap();

    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        ok_resp(br#"{"sha":"x","tree":[],"truncated":false}"#),
    );

    let mut args = args_for(dir.path(), "o/r");
    args.ignore = vec!["*.log".to_string()];

    let json = run_to_json(&args, &mock);
    let files = json["files"].as_array().unwrap();
    let paths: Vec<&str> = files.iter().map(|e| e["path"].as_str().unwrap()).collect();

    assert!(
        !paths.iter().any(|p| p.starts_with("build/")),
        "expected `.gitignore` to prune `build/`, got: {paths:?}"
    );
    assert!(
        !paths.contains(&"debug.log"),
        "expected `--ignore *.log` to prune `debug.log`, got: {paths:?}"
    );
    assert!(
        paths.contains(&"notes.md"),
        "expected `notes.md` to survive both ignore sources, got: {paths:?}"
    );
}

// ---- PRD 시나리오 13: --summary-only drops files[] field --------------------

#[test]
fn scenario_13_summary_only_drops_files_array() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
    let local_sha = lf_blob_hash("alpha\n");

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_sha}","size":6}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));

    let mut args = args_for(dir.path(), "o/r");
    args.summary_only = true;

    let json = run_to_json(&args, &mock);
    assert_eq!(json["summary"]["identical"], 1);
    assert!(
        json.get("files").is_none(),
        "summary-only must omit `files` field, got: {json}"
    );
}

// ---- PRD 시나리오 14: --status filter narrows files[] -----------------------

#[test]
fn scenario_14_status_filter_keeps_only_matching_entries() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("identical.md"), "alpha\n").unwrap();
    fs::write(dir.path().join("local_only.md"), "beta\n").unwrap();
    let local_a = lf_blob_hash("alpha\n");

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"identical.md","mode":"100644","type":"blob","sha":"{local_a}","size":6}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));

    let mut args = args_for(dir.path(), "o/r");
    args.status = Some("local_only_changed".to_string());

    let json = run_to_json(&args, &mock);
    // summary counts every classified entry; only `files[]` is filtered down.
    assert_eq!(json["summary"]["identical"], 1);
    assert_eq!(json["summary"]["local_only_changed"], 1);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["status"], "local_only_changed");
    assert_eq!(files[0]["path"], "local_only.md");
}
