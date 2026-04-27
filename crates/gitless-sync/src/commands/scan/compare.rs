use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Identical,
    LocalOnlyChanged,
    RemoteOnlyChanged,
    Drift,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub path: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_mtime: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_last_commit_at: Option<DateTime<Utc>>,
    pub is_binary: bool,
}

pub fn classify(
    local_sha: Option<&str>,
    remote_sha: Option<&str>,
    local_mtime: Option<DateTime<Utc>>,
    remote_last_commit_at: Option<DateTime<Utc>>,
) -> Status {
    let _ = (local_sha, remote_sha, local_mtime, remote_last_commit_at);
    todo!("classify into 4 buckets per PRD; tie on equal timestamps -> Drift")
}
