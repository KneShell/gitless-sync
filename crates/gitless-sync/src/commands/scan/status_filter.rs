//! `--status` filter parsing and the `summary_only` / `verbose` knobs that
//! shape the report's surface area.
//!
//! `parse_status_filter` is the only production export; tests here exercise
//! both the parser unit-level and the `build_report` integration shape so the
//! filter / summary / verbose interaction stays cohesive.

use super::compare::Status;
use crate::shared::error::GitlessError;

pub(super) fn parse_status_filter(raw: Option<&str>) -> Result<Option<Vec<Status>>, GitlessError> {
    let Some(s) = raw else {
        return Ok(None);
    };
    let mut out = Vec::new();
    for tok in s.split(',') {
        let trimmed = tok.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push(parse_status_token(trimmed)?);
    }
    if out.is_empty() {
        return Ok(None);
    }
    Ok(Some(out))
}

fn parse_status_token(s: &str) -> Result<Status, GitlessError> {
    match s {
        "identical" => Ok(Status::Identical),
        "local_only_changed" => Ok(Status::LocalOnlyChanged),
        "remote_only_changed" => Ok(Status::RemoteOnlyChanged),
        "drift" => Ok(Status::Drift),
        "failed" => Ok(Status::Failed),
        other => Err(GitlessError::Config(format!(
            "invalid --status value: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::commands::scan::build_report;
    use crate::commands::scan::output;
    use crate::commands::scan::test_helpers::{args_for, stub_tree};
    use crate::shared::gh::MockGhClient;
    use crate::shared::hash::blob_hash;

    // --- parse_status_filter ----------------------------------------------

    #[test]
    fn parse_status_filter_returns_none_when_arg_absent() {
        assert!(parse_status_filter(None).unwrap().is_none());
    }

    #[test]
    fn parse_status_filter_returns_none_for_empty_or_whitespace() {
        assert!(parse_status_filter(Some("")).unwrap().is_none());
        assert!(parse_status_filter(Some(" , ")).unwrap().is_none());
    }

    #[test]
    fn parse_status_filter_parses_single_value() {
        let v = parse_status_filter(Some("drift")).unwrap().unwrap();
        assert_eq!(v, vec![Status::Drift]);
    }

    #[test]
    fn parse_status_filter_parses_multiple_values_with_whitespace() {
        let v = parse_status_filter(Some("drift, local_only_changed ,identical"))
            .unwrap()
            .unwrap();
        assert_eq!(
            v,
            vec![Status::Drift, Status::LocalOnlyChanged, Status::Identical]
        );
    }

    #[test]
    fn parse_status_filter_accepts_all_known_tokens() {
        let v = parse_status_filter(Some(
            "identical,local_only_changed,remote_only_changed,drift,failed",
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            v,
            vec![
                Status::Identical,
                Status::LocalOnlyChanged,
                Status::RemoteOnlyChanged,
                Status::Drift,
                Status::Failed,
            ]
        );
    }

    #[test]
    fn parse_status_filter_errors_on_unknown_token() {
        let err = parse_status_filter(Some("nonsense")).unwrap_err();
        assert!(matches!(&err, GitlessError::Config(msg) if msg.contains("nonsense")));
        assert_eq!(err.exit_code(), 1);
    }

    // --- build_report status filter / summary_only / verbose --------------

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
        args.status = Some("local_only_changed".to_string());
        let (report, _) = build_report(&args, &mock).unwrap();

        assert_eq!(report.summary.identical, 1);
        assert_eq!(report.summary.local_only_changed, 1);

        let entries = report.files.unwrap();
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
        args.status = Some("local_only_changed,remote_only_changed".to_string());
        let (report, _) = build_report(&args, &mock).unwrap();

        let entries = report.files.unwrap();
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
        args.status = Some("drift".to_string());
        let (report, _) = build_report(&args, &mock).unwrap();

        assert!(report.files.is_none());
        assert_eq!(report.summary.identical, 1);
    }

    #[test]
    fn build_report_invalid_status_filter_yields_config_error() {
        let dir = TempDir::new().unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            r#"{"sha":"x","tree":[],"truncated":false}"#,
        );

        let mut args = args_for(dir.path(), Some("o/r"));
        args.status = Some("nonsense".to_string());
        let err = build_report(&args, &mock).unwrap_err();
        assert!(matches!(err, GitlessError::Config(_)));
        assert_eq!(err.exit_code(), 1);
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
}
