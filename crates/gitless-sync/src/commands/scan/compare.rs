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

/// Reason a path was promoted to [`Status::Failed`]. Maps to
/// `failed_reason` in the v1.1 output schema (`spec-output-schema.md`).
/// Omitted (`None`) is treated as `hash_io` for v1.0 backward-compat —
/// don't set it explicitly for hash-IO failures.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailedReason {
    CaseCollision,
    Submodule,
    Symlink,
    LongPath,
    LfsPointer,
    Encoding,
    NfdCollision,
    GitattributesUnsupported,
}

/// LFS pointer companion for a [`Status::Failed`] entry whose
/// `failed_reason` is [`FailedReason::LfsPointer`]. `scan` does not fetch
/// blobs and emits the placeholder `{oid: "?", size: 0}`; `diff` (later)
/// parses the actual pointer text. See `spec-output-schema.md` § v1.1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LfsPointer {
    pub oid: String,
    pub size: u64,
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
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_reason: Option<FailedReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lfs_pointer: Option<LfsPointer>,
}

/// Classify a single path into one of the 4-state categories.
///
/// # Panics
/// Panics when both `local_sha` and `remote_sha` are `None` — the caller
/// guarantees at least one side is present (`spec-classification.md`).
#[must_use]
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

    // N-task audit (2026-05-09): `failed_reason` enum vs spec-error-contracts.md
    // § Per-file Pitfall Reasons 정합 검증. `FailedReason` 5 variant
    // serde snake_case round-trip + `LfsPointer` placeholder shape 박음.
    // `hash_io` / `encoding` / `nfd_collision` / `gitattributes_unsupported`는
    // enum 미박힘 (None special case 또는 enum-spec'd-but-unimplemented).

    fn assert_failed_reason_round_trip(variant: FailedReason, expected: &str) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected}\""));
        let parsed: FailedReason = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, variant);
    }

    #[test]
    fn failed_reason_case_collision_serializes_snake_case() {
        assert_failed_reason_round_trip(FailedReason::CaseCollision, "case_collision");
    }

    #[test]
    fn failed_reason_submodule_serializes_snake_case() {
        assert_failed_reason_round_trip(FailedReason::Submodule, "submodule");
    }

    #[test]
    fn failed_reason_symlink_serializes_snake_case() {
        assert_failed_reason_round_trip(FailedReason::Symlink, "symlink");
    }

    #[test]
    fn failed_reason_long_path_serializes_snake_case() {
        assert_failed_reason_round_trip(FailedReason::LongPath, "long_path");
    }

    #[test]
    fn failed_reason_lfs_pointer_serializes_snake_case() {
        assert_failed_reason_round_trip(FailedReason::LfsPointer, "lfs_pointer");
    }

    #[test]
    fn failed_reason_encoding_serializes_snake_case() {
        assert_failed_reason_round_trip(FailedReason::Encoding, "encoding");
    }

    #[test]
    fn failed_reason_nfd_collision_serializes_snake_case() {
        assert_failed_reason_round_trip(FailedReason::NfdCollision, "nfd_collision");
    }

    #[test]
    fn failed_reason_gitattributes_unsupported_serializes_snake_case() {
        assert_failed_reason_round_trip(
            FailedReason::GitattributesUnsupported,
            "gitattributes_unsupported",
        );
    }

    fn sample_entry(failed_reason: Option<FailedReason>) -> FileEntry {
        FileEntry {
            path: "x".into(),
            status: Status::Failed,
            local_sha: None,
            remote_sha: None,
            local_mtime: None,
            remote_last_commit_at: None,
            is_binary: false,
            mode: "100644".into(),
            failed_reason,
            lfs_pointer: None,
        }
    }

    #[test]
    fn failed_reason_none_is_skipped_in_serialized_entry() {
        // v1.0 backward-compat: `hash_io` 동작은 None 박음, JSON에 key 자체 미노출.
        let json = serde_json::to_value(sample_entry(None)).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("failed_reason"));
        assert!(!obj.contains_key("lfs_pointer"));
    }

    #[test]
    fn failed_reason_some_is_emitted_in_serialized_entry() {
        let json = serde_json::to_value(sample_entry(Some(FailedReason::Submodule))).unwrap();
        assert_eq!(
            json.as_object().unwrap().get("failed_reason"),
            Some(&serde_json::Value::String("submodule".into()))
        );
    }

    #[test]
    fn lfs_pointer_placeholder_serializes_with_question_oid_and_zero_size() {
        // spec-output-schema.md § v1.1 + spec-error-contracts.md § lfs_pointer
        // 박음: scan은 blob fetch 안 함 → placeholder `{oid: "?", size: 0}`.
        let placeholder = LfsPointer {
            oid: "?".into(),
            size: 0,
        };
        let json = serde_json::to_value(&placeholder).unwrap();
        assert_eq!(json["oid"], serde_json::Value::String("?".into()));
        assert_eq!(json["size"], serde_json::Value::Number(0.into()));
    }

    #[test]
    fn lfs_pointer_round_trips_through_json() {
        let placeholder = LfsPointer {
            oid: "?".into(),
            size: 0,
        };
        let json = serde_json::to_string(&placeholder).unwrap();
        let parsed: LfsPointer = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, placeholder);
    }
}
