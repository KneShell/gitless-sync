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

// v1.0/v1.1/v1.2 backward-compat lock test는 wire-format stability 회귀
// 가드 — `tests/scan_output_backward_compat.rs` integration test에서
// 다룸. SCHEMA_VERSION은 production 코드에서 직접 사용되어 컴파일 시
// 보장 + 본 unit module은 production 공식 production-only.
