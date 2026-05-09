use chrono::{DateTime, Utc};
use serde::Serialize;

use super::compare::FileEntry;

pub const SCHEMA_VERSION: &str = "1.1";

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::scan::compare::{FailedReason, FileEntry, LfsPointer, Status};
    use chrono::{DateTime, TimeZone, Utc};
    use serde::Deserialize;

    // O-task audit (2026-05-09): spec-output-schema.md § Acceptance line
    // "v1.0 호출자가 v1.1 JSON 파싱 시 추가 필드 무시 + 기존 필드 정상 동작"
    // 박음. v1.1 신규 필드(`mode` / `failed_reason` / `lfs_pointer`)는
    // v1.0-shaped struct에 없어도 deserialize가 통과해야 함을 강제.

    /// v1.0 baseline 호출자 모양의 모방 — Phase 5 신규 필드 없음.
    /// `serde::Deserialize`의 기본 동작(미지의 필드 무시)이 깨지면 본 모방
    /// 구조체가 v1.1 출력 JSON을 파싱하지 못하고 실패. backward-compat
    /// contract의 lock test.
    #[derive(Debug, Deserialize)]
    struct V10ScanReport {
        schema_version: String,
        scanned_at: DateTime<Utc>,
        repo: String,
        branch: String,
        local_root: String,
        summary: V10Summary,
        files: Option<Vec<V10FileEntry>>,
    }

    #[derive(Debug, Deserialize)]
    struct V10Summary {
        identical: usize,
        local_only_changed: usize,
        remote_only_changed: usize,
        drift: usize,
        failed: usize,
    }

    #[derive(Debug, Deserialize)]
    struct V10FileEntry {
        path: String,
        status: Status,
        local_sha: Option<String>,
        remote_sha: Option<String>,
        local_mtime: Option<DateTime<Utc>>,
        remote_last_commit_at: Option<DateTime<Utc>>,
        is_binary: bool,
    }

    fn ts(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    fn v1_1_sample_report() -> ScanReport {
        // 한 Hashed identical entry + 한 Failed-with-lfs_pointer entry —
        // v1.1 신규 필드 3개(`mode` / `failed_reason` / `lfs_pointer`) 모두
        // 활성화된 모양으로 박음. v1.0 호출자가 본 JSON을 파싱해도 신규
        // 필드를 무시하고 v1.0 필드만 정합하게 읽혀야 함.
        let identical = FileEntry {
            path: "notes/foo.md".into(),
            status: Status::Identical,
            local_sha: Some("abc".into()),
            remote_sha: Some("abc".into()),
            local_mtime: Some(ts(1_700_000_000)),
            remote_last_commit_at: Some(ts(1_700_000_000)),
            is_binary: false,
            mode: "100644".into(),
            failed_reason: None,
            lfs_pointer: None,
        };
        let lfs_failed = FileEntry {
            path: "vendor/lib.zip".into(),
            status: Status::Failed,
            local_sha: None,
            remote_sha: Some("def".into()),
            local_mtime: None,
            remote_last_commit_at: None,
            is_binary: false,
            mode: "100644".into(),
            failed_reason: Some(FailedReason::LfsPointer),
            lfs_pointer: Some(LfsPointer {
                oid: "?".into(),
                size: 0,
            }),
        };
        ScanReport {
            schema_version: SCHEMA_VERSION.to_string(),
            scanned_at: ts(1_700_000_500),
            repo: "owner/name".into(),
            branch: "main".into(),
            local_root: "/tmp/root".into(),
            summary: Summary {
                identical: 1,
                local_only_changed: 0,
                remote_only_changed: 0,
                drift: 0,
                failed: 1,
            },
            files: Some(vec![identical, lfs_failed]),
        }
    }

    fn v1_1_sample_json() -> String {
        serialize(&v1_1_sample_report(), false).expect("serialize must succeed")
    }

    fn parse_v1_0(json: &str) -> V10ScanReport {
        serde_json::from_str(json).expect("v1.0 client must parse v1.1 JSON")
    }

    /// `#[serde(deny_unknown_fields)]`을 우연히 박는 회귀 lock — v1.0 모양
    /// 구조체로 v1.1 JSON envelope 필드(`schema_version` / `repo` / `branch`
    /// / `local_root` / `scanned_at` / `summary`)가 정합 deserialize.
    #[test]
    fn v1_0_client_parses_v1_1_envelope_fields() {
        let parsed = parse_v1_0(&v1_1_sample_json());
        assert_eq!(parsed.schema_version, "1.1");
        assert_eq!(parsed.repo, "owner/name");
        assert_eq!(parsed.branch, "main");
        assert_eq!(parsed.local_root, "/tmp/root");
        assert_eq!(parsed.scanned_at, ts(1_700_000_500));
        assert_eq!(parsed.summary.identical, 1);
        assert_eq!(parsed.summary.local_only_changed, 0);
        assert_eq!(parsed.summary.remote_only_changed, 0);
        assert_eq!(parsed.summary.drift, 0);
        assert_eq!(parsed.summary.failed, 1);
    }

    /// v1.0 baseline Identical entry — v1.0 모든 필드 정합 (`mode` /
    /// `failed_reason` / `lfs_pointer`는 v1.0 struct에 없어 자연 무시).
    #[test]
    fn v1_0_client_parses_v1_1_identical_entry_fields() {
        let parsed = parse_v1_0(&v1_1_sample_json());
        let files = parsed.files.expect("files must be present");
        let ident = &files[0];
        assert_eq!(ident.path, "notes/foo.md");
        assert_eq!(ident.status, Status::Identical);
        assert_eq!(ident.local_sha.as_deref(), Some("abc"));
        assert_eq!(ident.remote_sha.as_deref(), Some("abc"));
        assert_eq!(ident.local_mtime, Some(ts(1_700_000_000)));
        assert_eq!(ident.remote_last_commit_at, Some(ts(1_700_000_000)));
        assert!(!ident.is_binary);
    }

    /// v1.1 Failed-with-lfs_pointer entry — v1.0 호출자에게 status는
    /// `failed`로 보이고 v1.0 Optional 필드만 정합. 신규 필드는 무시.
    #[test]
    fn v1_0_client_parses_v1_1_failed_lfs_entry_fields() {
        let parsed = parse_v1_0(&v1_1_sample_json());
        let files = parsed.files.expect("files must be present");
        let lfs_failed = &files[1];
        assert_eq!(lfs_failed.path, "vendor/lib.zip");
        assert_eq!(lfs_failed.status, Status::Failed);
        assert!(lfs_failed.local_sha.is_none());
        assert_eq!(lfs_failed.remote_sha.as_deref(), Some("def"));
        assert!(lfs_failed.local_mtime.is_none());
        assert!(lfs_failed.remote_last_commit_at.is_none());
        assert!(!lfs_failed.is_binary);
    }

    fn raw_files(json: &str) -> Vec<serde_json::Value> {
        let raw: serde_json::Value = serde_json::from_str(json).expect("raw JSON must parse");
        raw["files"].as_array().expect("raw files array").clone()
    }

    /// v1.0 호출자 backward-compat — Identical entry는 `failed_reason` /
    /// `lfs_pointer`를 wire에 박지 않음. v1.0 baseline 호출자가 v1.1
    /// 출력을 보고 신규 필드 부재로 v1.0 동작 그대로 유지.
    #[test]
    fn v1_1_json_omits_failed_reason_and_lfs_pointer_for_identical_entry() {
        let files = raw_files(&v1_1_sample_json());
        let ident = files[0].as_object().expect("identical entry object");
        assert_eq!(
            ident.get("mode"),
            Some(&serde_json::Value::String("100644".into()))
        );
        assert!(!ident.contains_key("failed_reason"));
        assert!(!ident.contains_key("lfs_pointer"));
    }

    /// v1.1 신규 필드 wire format — Failed-with-lfs_pointer entry는
    /// `failed_reason: "lfs_pointer"` + placeholder `lfs_pointer: {oid:
    /// "?", size: 0}` 박힘. v1.1 호출자가 본 정보로 LFS fetch 결정.
    #[test]
    fn v1_1_json_includes_failed_reason_and_lfs_pointer_for_lfs_entry() {
        let files = raw_files(&v1_1_sample_json());
        let lfs_failed = &files[1];
        assert_eq!(
            lfs_failed["failed_reason"],
            serde_json::Value::String("lfs_pointer".into())
        );
        assert_eq!(
            lfs_failed["lfs_pointer"]["oid"],
            serde_json::Value::String("?".into())
        );
        assert_eq!(
            lfs_failed["lfs_pointer"]["size"],
            serde_json::Value::Number(0.into())
        );
    }
}
