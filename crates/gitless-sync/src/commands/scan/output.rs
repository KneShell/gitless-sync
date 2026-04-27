use chrono::{DateTime, Utc};
use serde::Serialize;

use super::compare::FileEntry;

pub const SCHEMA_VERSION: &str = "1.0";

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

pub fn serialize(report: &ScanReport, pretty: bool) -> Result<String, serde_json::Error> {
    if pretty {
        serde_json::to_string_pretty(report)
    } else {
        serde_json::to_string(report)
    }
}
