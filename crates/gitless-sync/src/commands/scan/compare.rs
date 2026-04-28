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
    match (local_sha, remote_sha) {
        (Some(a), Some(b)) if a == b => Status::Identical,
        (Some(_), None) => Status::LocalOnlyChanged,
        (None, Some(_)) => Status::RemoteOnlyChanged,
        (Some(_), Some(_)) => match (local_mtime, remote_last_commit_at) {
            (Some(l), Some(r)) if r < l => Status::LocalOnlyChanged,
            (Some(l), Some(r)) if l < r => Status::RemoteOnlyChanged,
            _ => Status::Drift,
        },
        (None, None) => unreachable!("classify must not be called with both SHAs absent"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    #[test]
    fn identical_when_shas_match() {
        let s = classify(Some("abc"), Some("abc"), Some(ts(100)), Some(ts(200)));
        assert_eq!(s, Status::Identical);
    }

    #[test]
    fn identical_ignores_timestamps() {
        let s = classify(Some("deadbeef"), Some("deadbeef"), None, None);
        assert_eq!(s, Status::Identical);
    }

    #[test]
    fn local_only_when_remote_missing() {
        let s = classify(Some("abc"), None, Some(ts(100)), None);
        assert_eq!(s, Status::LocalOnlyChanged);
    }

    #[test]
    fn remote_only_when_local_missing() {
        let s = classify(None, Some("abc"), None, Some(ts(100)));
        assert_eq!(s, Status::RemoteOnlyChanged);
    }

    #[test]
    fn local_only_when_remote_older() {
        let s = classify(Some("a"), Some("b"), Some(ts(200)), Some(ts(100)));
        assert_eq!(s, Status::LocalOnlyChanged);
    }

    #[test]
    fn remote_only_when_local_older() {
        let s = classify(Some("a"), Some("b"), Some(ts(100)), Some(ts(200)));
        assert_eq!(s, Status::RemoteOnlyChanged);
    }

    #[test]
    fn drift_on_equal_timestamps() {
        let s = classify(Some("a"), Some("b"), Some(ts(100)), Some(ts(100)));
        assert_eq!(s, Status::Drift);
    }

    #[test]
    fn drift_when_local_mtime_missing() {
        let s = classify(Some("a"), Some("b"), None, Some(ts(100)));
        assert_eq!(s, Status::Drift);
    }

    #[test]
    fn drift_when_remote_commit_missing() {
        let s = classify(Some("a"), Some("b"), Some(ts(100)), None);
        assert_eq!(s, Status::Drift);
    }

    #[test]
    fn drift_when_both_times_missing() {
        let s = classify(Some("a"), Some("b"), None, None);
        assert_eq!(s, Status::Drift);
    }

    #[test]
    #[should_panic(expected = "classify must not be called with both SHAs absent")]
    fn panics_when_both_shas_missing() {
        let _ = classify(None, None, Some(ts(100)), Some(ts(100)));
    }
}
