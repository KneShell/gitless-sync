//! Integration tests covering how `--status` / `--summary-only` / `-v`
//! shape the `build_report` output.
//!
//! Parsing of `--status` itself is delegated to clap (see
//! `commands/scan/args.rs::StatusFilter` + `main.rs` Cli derive); invalid
//! tokens are rejected at clap parse time with a "possible values" echo,
//! so they never reach `build_report` as a [`Status`] vector.

#![cfg(test)]

use std::fs;

use tempfile::TempDir;

use super::args::StatusFilter;
use super::compare::{FailedReason, Status};
use super::summary_view::FilesView;
use crate::commands::scan::build_report;
use crate::commands::scan::output;
use crate::commands::scan::test_helpers::{
    COMMITS_BODY, args_for, stub_blob, stub_commits, stub_tree,
};
use crate::shared::gh::MockGhClient;
use crate::shared::hash::blob_hash;

#[test]
fn build_report_summary_only_drops_files_field() {
    // Spec v1.5 #2: `summary.failed == 0` ⇒ `files` field omitted, JSON
    // string "files" absent. Failed-zero invariant preserves v1.4 baseline.
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
    let local_a = blob_hash(b"alpha\n");

    let mut mock = MockGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_a}","size":6}}],"truncated":false}}"#
    );
    stub_tree(&mut mock, "o/r", "main", &trees_body);

    let mut args = args_for(dir.path(), Some("o/r"));
    args.summary_only = true;
    let (report, _) = build_report(&args, &mock).unwrap();
    assert_eq!(report.summary.failed, 0);
    assert!(report.files.is_none());
    assert_eq!(report.summary.identical, 1);
    let json = output::serialize(&report, false).unwrap();
    assert!(!json.contains("\"files\""));
}

#[test]
fn build_report_status_filter_keeps_only_matching_entries() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("identical.md"), "alpha\n").unwrap();
    fs::write(dir.path().join("local_only.md"), "beta\n").unwrap();
    let local_a = blob_hash(b"alpha\n");

    let mut mock = MockGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"identical.md","mode":"100644","type":"blob","sha":"{local_a}","size":6}}],"truncated":false}}"#
    );
    stub_tree(&mut mock, "o/r", "main", &trees_body);

    let mut args = args_for(dir.path(), Some("o/r"));
    args.status = Some(vec![StatusFilter::LocalOnlyChanged]);
    let (report, _) = build_report(&args, &mock).unwrap();

    assert_eq!(report.summary.identical, 1);
    assert_eq!(report.summary.local_only_changed, 1);

    let FilesView::Full(entries) = report.files.unwrap() else {
        panic!("expected Full view");
    };
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].status, Status::LocalOnlyChanged);
    assert_eq!(entries[0].path, "local_only.md");
}

#[test]
fn build_report_status_filter_supports_multiple_values() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("identical.md"), "alpha\n").unwrap();
    fs::write(dir.path().join("local_only.md"), "beta\n").unwrap();
    let local_a = blob_hash(b"alpha\n");

    let mut mock = MockGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"identical.md","mode":"100644","type":"blob","sha":"{local_a}","size":6}},{{"path":"remote_only.md","mode":"100644","type":"blob","sha":"deadbeef","size":3}}],"truncated":false}}"#
    );
    stub_tree(&mut mock, "o/r", "main", &trees_body);

    let mut args = args_for(dir.path(), Some("o/r"));
    args.status = Some(vec![
        StatusFilter::LocalOnlyChanged,
        StatusFilter::RemoteOnlyChanged,
    ]);
    let (report, _) = build_report(&args, &mock).unwrap();

    let FilesView::Full(entries) = report.files.unwrap() else {
        panic!("expected Full view");
    };
    assert_eq!(entries.len(), 2);
    for e in &entries {
        assert!(matches!(
            e.status,
            Status::LocalOnlyChanged | Status::RemoteOnlyChanged
        ));
    }
}

#[test]
fn build_report_summary_only_overrides_status_filter() {
    // Spec v1.5 #5: summary-only + `--status drift` ⇒ filter ignored,
    // SummaryFailed projection emits failed entries even when fixture has
    // a sha-differ Hashed entry the filter would otherwise keep.
    // Plan body "failed N + drift M" — the Hashed entry's final timestamp
    // arm (system clock vs COMMITS_BODY's 2024 date) is incidental to
    // proving the override; the contract is "failed visible despite filter".
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("hashed.md"), "local\n").unwrap();

    let mut mock = MockGhClient::new();
    let trees_body = r#"{"sha":"x","tree":[{"path":"hashed.md","mode":"100644","type":"blob","sha":"sha-remote","size":6},{"path":"vendor/lib","mode":"160000","type":"commit","sha":"sm1","size":0}],"truncated":false}"#;
    stub_tree(&mut mock, "o/r", "main", trees_body);
    stub_blob(&mut mock, "o/r", "sha-remote", b"remote\n");
    stub_commits(&mut mock, "o/r", "main", "hashed.md", COMMITS_BODY);

    let mut args = args_for(dir.path(), Some("o/r"));
    args.summary_only = true;
    args.status = Some(vec![StatusFilter::Drift]);
    let (report, _) = build_report(&args, &mock).unwrap();

    assert_eq!(report.summary.failed, 1);
    let hashed_total = report.summary.identical
        + report.summary.local_only_changed
        + report.summary.remote_only_changed
        + report.summary.drift;
    assert!(hashed_total >= 1, "fixture must produce ≥1 Hashed entry");

    let FilesView::SummaryFailed(rows) = report.files.as_ref().expect("Some(SummaryFailed)") else {
        panic!("expected SummaryFailed view");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].path, "vendor/lib");
    assert_eq!(rows[0].failed_reason, Some(FailedReason::Submodule));
}

// Task N — F3 summary-only failed-visibility scenarios (schema v1.5 #2~#5).

fn submodule_tree_body(path: &str, sha: &str) -> String {
    format!(
        r#"{{"sha":"x","tree":[{{"path":"{path}","mode":"160000","type":"commit","sha":"{sha}","size":0}}],"truncated":false}}"#
    )
}

#[test]
fn build_report_summary_only_emits_no_files_when_failed_zero() {
    // Scenario 1 (spec v1.5 #2): failed == 0 ⇒ `files` field omit + JSON
    // string "files" absent. v1.4 baseline preserved.
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
    let local_a = blob_hash(b"alpha\n");

    let mut mock = MockGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_a}","size":6}}],"truncated":false}}"#
    );
    stub_tree(&mut mock, "o/r", "main", &trees_body);

    let mut args = args_for(dir.path(), Some("o/r"));
    args.summary_only = true;
    let (report, _) = build_report(&args, &mock).unwrap();
    assert_eq!(report.summary.failed, 0);
    assert!(report.files.is_none());
    let json = output::serialize(&report, false).unwrap();
    assert!(!json.contains("\"files\""));
}

#[test]
fn build_report_summary_only_emits_minimal_three_field_entry_when_failed_one() {
    // Scenario 2 (spec v1.5 #3 + #4): failed == 1 ⇒ `files[]` length 1 with
    // exactly `path` + `presence` + `failed_reason` three keys. Detail
    // fields (`status` / `local_sha` / `mode` / `lfs_pointer` / ...) all
    // stripped at wire level.
    let dir = TempDir::new().unwrap();
    let mut mock = MockGhClient::new();
    stub_tree(
        &mut mock,
        "o/r",
        "main",
        &submodule_tree_body("vendor/lib", "sm1"),
    );

    let mut args = args_for(dir.path(), Some("o/r"));
    args.summary_only = true;
    let (report, _) = build_report(&args, &mock).unwrap();
    assert_eq!(report.summary.failed, 1);

    let FilesView::SummaryFailed(rows) = report.files.as_ref().expect("Some(SummaryFailed)") else {
        panic!("expected SummaryFailed view");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].path, "vendor/lib");
    assert_eq!(rows[0].failed_reason, Some(FailedReason::Submodule));

    let json = output::serialize(&report, false).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let obj = value["files"][0].as_object().unwrap();
    // len==3 + 3 expected keys ⇒ no detail fields slipped through.
    assert_eq!(obj.len(), 3);
    for key in ["path", "presence", "failed_reason"] {
        assert!(obj.contains_key(key), "expected key {key}");
    }
}

#[test]
fn build_report_summary_only_emits_only_failed_entries_when_failed_n_among_others() {
    // Scenario 3 (spec v1.5 #3): failed N + identical/drift M ⇒ SummaryFailed
    // rows length N. Non-failed status entries (identical / drift / ...) drop.
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("ok.md"), "alpha\n").unwrap();
    let local_a = blob_hash(b"alpha\n");

    let mut mock = MockGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"ok.md","mode":"100644","type":"blob","sha":"{local_a}","size":6}},{{"path":"sub_a","mode":"160000","type":"commit","sha":"sm-a","size":0}},{{"path":"sub_b","mode":"160000","type":"commit","sha":"sm-b","size":0}}],"truncated":false}}"#
    );
    stub_tree(&mut mock, "o/r", "main", &trees_body);

    let mut args = args_for(dir.path(), Some("o/r"));
    args.summary_only = true;
    let (report, _) = build_report(&args, &mock).unwrap();
    assert_eq!(report.summary.identical, 1);
    assert_eq!(report.summary.failed, 2);

    let FilesView::SummaryFailed(rows) = report.files.as_ref().expect("Some(SummaryFailed)") else {
        panic!("expected SummaryFailed view");
    };
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(row.failed_reason, Some(FailedReason::Submodule));
    }
}

#[test]
fn build_report_summary_only_emits_failed_when_status_filter_drift_present() {
    // Scenario 4 (spec v1.5 #5): summary-only + `--status drift` ⇒ filter
    // ignored, SummaryFailed still emits failed entries. Failed-only fixture
    // — `_overrides_status_filter` covers the mixed Hashed+Failed case.
    let dir = TempDir::new().unwrap();
    let mut mock = MockGhClient::new();
    stub_tree(
        &mut mock,
        "o/r",
        "main",
        &submodule_tree_body("vendor/lib", "sm1"),
    );

    let mut args = args_for(dir.path(), Some("o/r"));
    args.summary_only = true;
    args.status = Some(vec![StatusFilter::Drift]);
    let (report, _) = build_report(&args, &mock).unwrap();
    assert_eq!(report.summary.failed, 1);

    let FilesView::SummaryFailed(rows) = report.files.as_ref().expect("Some(SummaryFailed)") else {
        panic!("expected SummaryFailed view");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].failed_reason, Some(FailedReason::Submodule));
}

#[test]
fn build_report_verbose_levels_do_not_change_report() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
    let local_a = blob_hash(b"alpha\n");

    for level in [0u8, 1, 2] {
        let mut mock = MockGhClient::new();
        let trees_body = format!(
            r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_a}","size":6}}],"truncated":false}}"#
        );
        stub_tree(&mut mock, "o/r", "main", &trees_body);

        let mut args = args_for(dir.path(), Some("o/r"));
        args.verbose = level;
        let (report, _) = build_report(&args, &mock).unwrap();
        assert_eq!(report.summary.identical, 1);
        assert!(report.files.is_some());
    }
}
