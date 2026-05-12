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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    LocalOnly,
    Both,
    RemoteOnly,
}

/// Reason a path was promoted to [`Status::Failed`]. Maps to schema
/// `failed_reason`. v1.5까지 caller는 `None`을 `hash_io` sentinel로 emit
/// 했으나 v1.6부터 explicit [`FailedReason::HashIo`] variant로 변경 —
/// `Status::Failed` entry는 항상 `Some(_)`. `None`은 Failed 외 entry용.
/// See `spec-output-schema.md` § v1.5 → v1.6 변경.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailedReason {
    HashIo,
    CaseCollision,
    Submodule,
    Symlink,
    LongPath,
    LfsPointer,
    Encoding,
    NfdCollision,
    GitattributesUnsupported,
    FileTooLarge,
    MemoryExceeded,
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
    pub presence: Presence,
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
    pub diff_meaningful: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_reason: Option<FailedReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lfs_pointer: Option<LfsPointer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_failed_reason_round_trip(variant: FailedReason, expected: &str) {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{expected}\""));
        let parsed: FailedReason = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, variant);
    }

    #[test]
    fn failed_reason_hash_io_serializes_snake_case() {
        assert_failed_reason_round_trip(FailedReason::HashIo, "hash_io");
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

    #[test]
    fn failed_reason_file_too_large_serializes_snake_case() {
        assert_failed_reason_round_trip(FailedReason::FileTooLarge, "file_too_large");
    }

    #[test]
    fn failed_reason_memory_exceeded_serializes_snake_case() {
        assert_failed_reason_round_trip(FailedReason::MemoryExceeded, "memory_exceeded");
    }

    fn sample_entry(failed_reason: Option<FailedReason>) -> FileEntry {
        FileEntry {
            path: "x".into(),
            status: Status::Failed,
            presence: Presence::Both,
            local_sha: None,
            remote_sha: None,
            local_mtime: None,
            remote_last_commit_at: None,
            is_binary: false,
            mode: "100644".into(),
            diff_meaningful: None,
            failed_reason,
            lfs_pointer: None,
            size_bytes: None,
        }
    }

    #[test]
    fn failed_reason_none_is_skipped_in_serialized_entry() {
        // v1.6: `None`은 Failed 외 entry용. wire는 `skip_serializing_if` 그대로
        // 통과 — `hash_io` sentinel 의미는 v1.6에서 제거 (`HashIo` variant explicit).
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

    #[test]
    fn size_bytes_field_omitted_when_none() {
        let json = serde_json::to_value(sample_entry(None)).unwrap();
        assert!(!json.as_object().unwrap().contains_key("size_bytes"));
    }
}
