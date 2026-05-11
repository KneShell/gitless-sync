#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Phase 8 task K + v0.4.2 issue #1 regression — end-to-end pipeline for
//! "sha differ but normalize-equal" (cosmetic SHA drift).
//!
//! Local stores LF, remote stores CRLF. Trees-API sha differs from
//! `local_sha` (raw bytes differ by the CR), but `prepare_for_hash`
//! LF-normalizes both sides → recomputed remote hash matches `local_sha`
//! → `normalize_equal=true`.
//!
//! v1.3 (Phase 8): scenario classified as `Status::Drift` + `diff_meaningful=false`
//! (caller hint, but status still mismatched spec-hash-and-normalize.md § 목적).
//! v1.4 (v0.4.2 issue #1 fix): scenario classifies as `Status::Identical`
//! per `spec-classification.md` § Status 정의 (sha-differ + `normalize_equal=Some(true)`
//! → Identical). The triple-pin (`status=identical`, `presence=both`,
//! `diff_meaningful=false`) ensures the test fails on three distinct regressions:
//! classify cosmetic-Identical arm removal, presence enum drop, or
//! `diff_meaningful` degradation.

mod common;

use std::fs;

use tempfile::TempDir;

use common::{
    TestGhClient, args_for, commits_args, commits_body_with_date, lf_blob_hash, ok_resp,
    read_mtime_rfc3339, run_to_json, stub_blob, tree_args,
};

#[test]
fn f1_crlf_remote_lf_local_yields_identical_via_normalize_equal() {
    // Local file stores LF — `local_sha` is `blob_hash(prepare_for_hash(b"hello\n"))`,
    // which is just `blob_hash(b"hello\n")` for an LF input (no normalization
    // change). Remote Trees-API reports a different sha (the raw blob hash of
    // CRLF bytes). Pipeline pass 1.5 fetches the blob, runs `prepare_for_hash`
    // on the CRLF payload → LF → recomputed hash matches `local_sha` →
    // `normalize_equal=true`. v1.4 (v0.4.2 issue #1 fix): classify promotes
    // sha-differ + normalize_equal=Some(true) → Status::Identical, overriding
    // the timestamp arm (which would have landed Drift via G-005 tie).
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("hello.md");
    fs::write(&path, "hello\n").unwrap();
    let mtime_str = read_mtime_rfc3339(&path);

    // `local_sha` derived purely from the LF input (no CRLF on disk locally).
    let local_sha = lf_blob_hash("hello\n");
    // Trees response carries a placeholder sha distinct from `local_sha` —
    // the actual remote-sha matching CRLF bytes is not load-bearing here, only
    // the inequality (forces `fetch_normalize_equal_map` to do its job).
    let remote_trees_sha = "remote-crlf-trees-sha";
    assert_ne!(
        local_sha.as_str(),
        remote_trees_sha,
        "fixture invariant: trees sha must differ from local_sha so the \
         normalize-equal pass actually fetches the blob"
    );

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"hello.md","mode":"100644","type":"blob","sha":"{remote_trees_sha}","size":7}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));
    // Remote blob carries CRLF — prepare_for_hash strips it back to LF on the
    // way to the recomputed hash.
    stub_blob(&mut mock, "o/r", remote_trees_sha, b"hello\r\n");
    // sha-differ entry still triggers the Commits API call from
    // `extract_commit_paths` (it filters before classify runs). Stub a
    // timestamp tie so a regression that drops the cosmetic-Identical arm
    // would fall through to Drift via G-005 (clear failure mode).
    mock.stub(
        commits_args("o/r", "main", "hello.md"),
        ok_resp(commits_body_with_date(&mtime_str).as_bytes()),
    );

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);

    // Summary — single entry, classified as Identical via cosmetic-SHA fix
    // (v1.4 issue #1). drift count must be 0.
    assert_eq!(json["summary"]["identical"], 1);
    assert_eq!(json["summary"]["drift"], 0);
    assert_eq!(json["summary"]["local_only_changed"], 0);
    assert_eq!(json["summary"]["remote_only_changed"], 0);
    assert_eq!(json["summary"]["failed"], 0);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    let entry = &files[0];
    assert_eq!(entry["path"], "hello.md");
    assert_eq!(entry["status"], "identical");
    assert_eq!(entry["presence"], "both");
    // Load-bearing assertion: `false` discriminates from both `null` (the
    // "unknown" arm — would mean the blob fetch / normalize pipeline broke)
    // and `true` (the "real semantic diff" arm — would mean prepare_for_hash
    // failed to LF-normalize the remote payload).
    assert_eq!(entry["diff_meaningful"], serde_json::Value::Bool(false));
    assert_eq!(entry["local_sha"], local_sha);
    assert_eq!(entry["remote_sha"], remote_trees_sha);
}
