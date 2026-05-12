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
use super::compare::Status;
use super::summary_view::FilesView;
use crate::commands::scan::build_report;
use crate::commands::scan::output;
use crate::commands::scan::test_helpers::{args_for, stub_tree};
use crate::shared::gh::MockGhClient;
use crate::shared::hash::blob_hash;

#[test]
fn build_report_summary_only_drops_files_field() {
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
    args.status = Some(vec![StatusFilter::Drift]);
    let (report, _) = build_report(&args, &mock).unwrap();

    assert!(report.files.is_none());
    assert_eq!(report.summary.identical, 1);
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
