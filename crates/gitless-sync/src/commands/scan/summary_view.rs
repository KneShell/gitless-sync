//! `--summary-only` mode `files[]` projection.
//!
//! `FilesView::SummaryFailed` carries minimal failed-entry rows
//! (path + presence + `failed_reason`) per `spec-output-schema.md` § v1.5.
//! `failed_reason == None` (`hash_io` signal) collapses the row to
//! `path + presence` two fields via `#[serde(skip_serializing_if)]`.

use serde::Serialize;

use super::compare::{FailedReason, FileEntry, Presence, Status};

#[derive(Debug, Serialize)]
pub struct SummaryFailedEntry {
    pub path: String,
    pub presence: Presence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_reason: Option<FailedReason>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum FilesView {
    Full(Vec<FileEntry>),
    SummaryFailed(Vec<SummaryFailedEntry>),
}

/// Project the entry vector for `ScanReport.files` per CLI flags.
///
/// Branches per `spec-output-schema.md` § `--summary-only` 출력:
/// - `summary_only && failed_count == 0` → `None` (v1.4 baseline omit).
/// - `summary_only && failed_count > 0` → `Some(SummaryFailed([..]))` —
///   failed-only rows, `path` + `presence` + `failed_reason` per entry.
/// - else → `Some(Full(entries))`.
#[must_use]
pub fn project_files(
    summary_only: bool,
    entries: Vec<FileEntry>,
    failed_count: usize,
) -> Option<FilesView> {
    if !summary_only {
        return Some(FilesView::Full(entries));
    }
    if failed_count == 0 {
        return None;
    }
    let rows: Vec<SummaryFailedEntry> = entries
        .into_iter()
        .filter(|e| e.status == Status::Failed)
        .map(|e| SummaryFailedEntry {
            path: e.path,
            presence: e.presence,
            failed_reason: e.failed_reason,
        })
        .collect();
    Some(FilesView::SummaryFailed(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed_entry(path: &str, presence: Presence, reason: Option<FailedReason>) -> FileEntry {
        FileEntry {
            path: path.into(),
            status: Status::Failed,
            presence,
            local_sha: None,
            remote_sha: None,
            local_mtime: None,
            remote_last_commit_at: None,
            is_binary: false,
            mode: "100644".into(),
            diff_meaningful: None,
            failed_reason: reason,
            lfs_pointer: None,
            size_bytes: None,
        }
    }

    fn identical_entry(path: &str) -> FileEntry {
        FileEntry {
            path: path.into(),
            status: Status::Identical,
            presence: Presence::Both,
            local_sha: Some("abc".into()),
            remote_sha: Some("abc".into()),
            local_mtime: None,
            remote_last_commit_at: None,
            is_binary: false,
            mode: "100644".into(),
            diff_meaningful: Some(false),
            failed_reason: None,
            lfs_pointer: None,
            size_bytes: None,
        }
    }

    #[test]
    fn project_returns_full_when_summary_only_disabled() {
        let entries = vec![identical_entry("a.md")];
        let view = project_files(false, entries, 0).expect("Some Full");
        match view {
            FilesView::Full(v) => assert_eq!(v.len(), 1),
            FilesView::SummaryFailed(_) => panic!("expected Full"),
        }
    }

    #[test]
    fn project_returns_none_when_summary_only_and_no_failures() {
        let entries = vec![identical_entry("a.md")];
        assert!(project_files(true, entries, 0).is_none());
    }

    #[test]
    fn project_returns_summary_failed_only_when_summary_only_and_failures() {
        let entries = vec![
            identical_entry("a.md"),
            failed_entry("b.md", Presence::Both, Some(FailedReason::Submodule)),
            failed_entry("c.md", Presence::LocalOnly, None),
        ];
        let view = project_files(true, entries, 2).expect("Some SummaryFailed");
        let FilesView::SummaryFailed(rows) = view else {
            panic!("expected SummaryFailed");
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].path, "b.md");
        assert_eq!(rows[0].presence, Presence::Both);
        assert_eq!(rows[0].failed_reason, Some(FailedReason::Submodule));
        assert_eq!(rows[1].path, "c.md");
        assert_eq!(rows[1].presence, Presence::LocalOnly);
        assert_eq!(rows[1].failed_reason, None);
    }

    /// Task L acceptance: a fully-populated [`FileEntry`] passed through
    /// [`project_files`] yields a row carrying `path` / `presence` /
    /// `failed_reason` only — rich fields (`sha` / `mtime` / `size` / `mode` /
    /// `diff_meaningful` / `lfs_pointer`) drop at both the type and wire level.
    #[test]
    fn project_strips_full_failed_entry_to_three_field_minimal_row() {
        use super::super::compare::LfsPointer;
        use chrono::{TimeZone, Utc};

        let rich = FileEntry {
            path: "vendor/lib.zip".into(),
            status: Status::Failed,
            presence: Presence::Both,
            local_sha: Some("abc123".into()),
            remote_sha: Some("def456".into()),
            local_mtime: Some(Utc.timestamp_opt(1, 0).unwrap()),
            remote_last_commit_at: Some(Utc.timestamp_opt(2, 0).unwrap()),
            is_binary: true,
            mode: "100755".into(),
            diff_meaningful: Some(true),
            failed_reason: Some(FailedReason::LfsPointer),
            lfs_pointer: Some(LfsPointer {
                oid: "sha256:abc".into(),
                size: 9999,
            }),
            size_bytes: Some(12345),
        };
        let view = project_files(true, vec![rich], 1).expect("Some SummaryFailed");
        let FilesView::SummaryFailed(rows) = view else {
            panic!("expected SummaryFailed");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path, "vendor/lib.zip");
        assert_eq!(rows[0].presence, Presence::Both);
        assert_eq!(rows[0].failed_reason, Some(FailedReason::LfsPointer));

        let value = serde_json::to_value(&rows[0]).unwrap();
        let obj = value.as_object().unwrap();
        assert_eq!(obj.len(), 3);
        for stripped in [
            "status",
            "local_sha",
            "remote_sha",
            "local_mtime",
            "remote_last_commit_at",
            "is_binary",
            "mode",
            "diff_meaningful",
            "lfs_pointer",
            "size_bytes",
        ] {
            assert!(
                !obj.contains_key(stripped),
                "key {stripped} must be stripped"
            );
        }
    }

    #[test]
    fn summary_failed_entry_wire_emits_three_fields() {
        let entry = SummaryFailedEntry {
            path: "v.zip".into(),
            presence: Presence::Both,
            failed_reason: Some(FailedReason::LfsPointer),
        };
        let value = serde_json::to_value(&entry).unwrap();
        let obj = value.as_object().unwrap();
        assert_eq!(obj.len(), 3);
        assert_eq!(obj["path"], "v.zip");
        assert_eq!(obj["presence"], "both");
        assert_eq!(obj["failed_reason"], "lfs_pointer");
    }

    #[test]
    fn summary_failed_entry_wire_omits_hash_io_failed_reason() {
        // failed_reason == None ⇒ hash_io signal per spec line 342.
        // wire emits only `path` + `presence` (key absent).
        let entry = SummaryFailedEntry {
            path: "x".into(),
            presence: Presence::Both,
            failed_reason: None,
        };
        let value = serde_json::to_value(&entry).unwrap();
        let obj = value.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert!(obj.contains_key("path"));
        assert!(obj.contains_key("presence"));
        assert!(!obj.contains_key("failed_reason"));
    }

    #[test]
    fn files_view_untagged_full_serializes_as_array() {
        let view = FilesView::Full(vec![identical_entry("a.md")]);
        let value = serde_json::to_value(&view).unwrap();
        let arr = value.as_array().expect("array");
        assert_eq!(arr.len(), 1);
    }

    #[test]
    fn files_view_untagged_summary_failed_serializes_as_array() {
        let view = FilesView::SummaryFailed(vec![SummaryFailedEntry {
            path: "x".into(),
            presence: Presence::Both,
            failed_reason: None,
        }]);
        let value = serde_json::to_value(&view).unwrap();
        let arr = value.as_array().expect("array");
        assert_eq!(arr.len(), 1);
    }
}
