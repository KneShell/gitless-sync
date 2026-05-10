use chrono::{DateTime, Utc};
use serde::Serialize;

use super::compare::FileEntry;

pub const SCHEMA_VERSION: &str = "1.2";

#[derive(Debug, Default, Serialize)]
pub struct Summary {
    pub identical: usize,
    pub local_only_changed: usize,
    pub remote_only_changed: usize,
    pub drift: usize,
    pub failed: usize,
}

#[derive(Debug, Serialize)]
pub struct ScanReport {
    pub schema_version: String,
    pub scanned_at: DateTime<Utc>,
    pub repo: String,
    pub branch: String,
    pub local_root: String,
    pub summary: Summary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileEntry>>,
}

/// Serialize a [`ScanReport`] into the stdout JSON shape (`spec-output-schema.md`).
///
/// # Errors
/// Returns the underlying [`serde_json::Error`] if serialization fails.
/// In practice the report is composed of `serde::Serialize` types with no
/// fallible implementations, so callers may treat this as effectively total.
pub fn serialize(report: &ScanReport, pretty: bool) -> Result<String, serde_json::Error> {
    if pretty {
        serde_json::to_string_pretty(report)
    } else {
        serde_json::to_string(report)
    }
}

// v1.0/v1.1 client struct 모방 backward-compat 회귀 가드는
// `tests/scan_output_backward_compat.rs` integration test에서 다룸 (P task).
// 본 unit module은 ScanReport JSON wire-format 직접 invariant 검증 — P와
// 직교 layer.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::scan::compare::{FailedReason, FileEntry, LfsPointer, Status};
    use chrono::TimeZone;

    fn ts(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    fn empty_report() -> ScanReport {
        ScanReport {
            schema_version: SCHEMA_VERSION.to_string(),
            scanned_at: ts(1),
            repo: "owner/name".into(),
            branch: "main".into(),
            local_root: "/r".into(),
            summary: Summary::default(),
            files: None,
        }
    }

    fn report_with(files: Vec<FileEntry>) -> ScanReport {
        ScanReport {
            files: Some(files),
            ..empty_report()
        }
    }

    fn size_gate_entry(reason: FailedReason, size: u64) -> FileEntry {
        FileEntry {
            path: "p".into(),
            status: Status::Failed,
            local_sha: None,
            remote_sha: None,
            local_mtime: None,
            remote_last_commit_at: None,
            is_binary: false,
            mode: "100644".into(),
            failed_reason: Some(reason),
            lfs_pointer: None,
            size_bytes: Some(size),
        }
    }

    fn identical_entry() -> FileEntry {
        FileEntry {
            path: "i".into(),
            status: Status::Identical,
            local_sha: Some("abc".into()),
            remote_sha: Some("abc".into()),
            local_mtime: Some(ts(1)),
            remote_last_commit_at: Some(ts(1)),
            is_binary: false,
            mode: "100644".into(),
            failed_reason: None,
            lfs_pointer: None,
            size_bytes: None,
        }
    }

    fn parse(report: &ScanReport) -> serde_json::Value {
        serde_json::from_str(&serialize(report, false).unwrap()).unwrap()
    }

    /// Acceptance #1: `report.schema_version` == `"1.2"`. `SCHEMA_VERSION`
    /// constant + `ScanReport` 직렬화 결과의 `schema_version` field 두 layer.
    #[test]
    fn schema_version_field_serializes_as_1_2() {
        assert_eq!(SCHEMA_VERSION, "1.2");
        assert_eq!(parse(&empty_report())["schema_version"], "1.2");
    }

    /// Acceptance #2: `failed_reason` enum 11 값 cover (10 variant + `None`
    /// special case `hash_io`). `compare.rs::tests` round-trip과 직교 — 본
    /// test는 `ScanReport` 안 entry 직렬화 시 wire `snake_case` 일관성 박음.
    #[test]
    fn failed_reason_eleven_schema_values_serialize_within_scan_report() {
        let cases: [(Option<FailedReason>, Option<&str>); 11] = [
            (None, None),
            (Some(FailedReason::CaseCollision), Some("case_collision")),
            (Some(FailedReason::Submodule), Some("submodule")),
            (Some(FailedReason::Symlink), Some("symlink")),
            (Some(FailedReason::LongPath), Some("long_path")),
            (Some(FailedReason::LfsPointer), Some("lfs_pointer")),
            (Some(FailedReason::Encoding), Some("encoding")),
            (Some(FailedReason::NfdCollision), Some("nfd_collision")),
            (
                Some(FailedReason::GitattributesUnsupported),
                Some("gitattributes_unsupported"),
            ),
            (Some(FailedReason::FileTooLarge), Some("file_too_large")),
            (Some(FailedReason::MemoryExceeded), Some("memory_exceeded")),
        ];
        for (variant, wire) in cases {
            let entry = FileEntry {
                failed_reason: variant,
                ..identical_entry()
            };
            let json = parse(&report_with(vec![entry]));
            let obj = json["files"][0].as_object().unwrap();
            match wire {
                None => assert!(!obj.contains_key("failed_reason"), "None must omit"),
                Some(s) => assert_eq!(obj["failed_reason"], s),
            }
        }
    }

    /// Acceptance #3: `failed_reason == "file_too_large"` entry는
    /// `size_bytes` field 포함. 100 MB 초과 size 그대로 wire에 박힘.
    #[test]
    fn file_too_large_entry_emits_size_bytes_above_100mb() {
        let report = report_with(vec![size_gate_entry(
            FailedReason::FileTooLarge,
            157_286_400,
        )]);
        let entry = &parse(&report)["files"][0];
        assert_eq!(entry["failed_reason"], "file_too_large");
        assert_eq!(entry["size_bytes"], 157_286_400u64);
    }

    /// Acceptance #4: `failed_reason == "memory_exceeded"` entry는
    /// `size_bytes` field 포함. 50~100 MB 사이 size 그대로 wire에 박힘.
    #[test]
    fn memory_exceeded_entry_emits_size_bytes_between_50_100mb() {
        let report = report_with(vec![size_gate_entry(
            FailedReason::MemoryExceeded,
            62_914_560,
        )]);
        let entry = &parse(&report)["files"][0];
        assert_eq!(entry["failed_reason"], "memory_exceeded");
        assert_eq!(entry["size_bytes"], 62_914_560u64);
    }

    /// Acceptance #5: `failed_reason` 가 `size_gate` 외 entry는 `size_bytes`
    /// field omit (`#[serde(skip_serializing_if = "Option::is_none")]` 박힘).
    #[test]
    fn non_size_gate_entries_omit_size_bytes_field() {
        let lfs_failed = FileEntry {
            path: "lfs".into(),
            status: Status::Failed,
            failed_reason: Some(FailedReason::LfsPointer),
            lfs_pointer: Some(LfsPointer {
                oid: "?".into(),
                size: 0,
            }),
            ..identical_entry()
        };
        let json = parse(&report_with(vec![identical_entry(), lfs_failed]));
        for entry in json["files"].as_array().unwrap() {
            assert!(!entry.as_object().unwrap().contains_key("size_bytes"));
        }
    }

    /// Acceptance #6: `file_too_large` / `memory_exceeded` entry는
    /// `is_binary: false`. size pre-flight short-circuit으로 local read 전
    /// 격하 (Phase 5.13.1 task EE 정합).
    #[test]
    fn size_gate_entries_emit_is_binary_false() {
        let report = report_with(vec![
            size_gate_entry(FailedReason::FileTooLarge, 200_000_000),
            size_gate_entry(FailedReason::MemoryExceeded, 60_000_000),
        ]);
        let json = parse(&report);
        for entry in json["files"].as_array().unwrap() {
            assert_eq!(entry["is_binary"], serde_json::Value::Bool(false));
        }
    }

    /// Acceptance #7: v1.0 / v1.1 호출자가 v1.2 JSON 파싱 시 backward-compat.
    /// P task의 client struct 모방과 직교 layer — wire JSON에 v1.0 + v1.1
    /// 필수 필드가 박힘 invariant. v1.2 신규 `size_bytes`는 not-applicable
    /// entry에서 omit.
    #[test]
    fn v1_2_wire_shape_preserves_v1_0_and_v1_1_required_fields() {
        let json = parse(&report_with(vec![identical_entry()]));
        for key in [
            "schema_version",
            "scanned_at",
            "repo",
            "branch",
            "local_root",
            "summary",
            "files",
        ] {
            assert!(json.get(key).is_some(), "v1.0 envelope key {key} missing");
        }
        let entry = json["files"][0].as_object().unwrap();
        for key in [
            "path",
            "status",
            "local_sha",
            "remote_sha",
            "local_mtime",
            "remote_last_commit_at",
            "is_binary",
        ] {
            assert!(entry.contains_key(key), "v1.0 entry key {key} missing");
        }
        assert!(entry.contains_key("mode"), "v1.1 mode field missing");
        assert!(!entry.contains_key("size_bytes"));
    }
}
