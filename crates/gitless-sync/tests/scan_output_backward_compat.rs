#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! v1.0 / v1.1 호출자가 v1.2 출력 JSON을 backward-compat 정합 파싱하는지
//! 회귀 lock. spec-output-schema.md § v1.2 신규 Acceptance Criteria
//! "v1.0 / v1.1 호출자가 v1.2 JSON 파싱 시 추가 필드(`size_bytes`) +
//! 추가 enum 값(`file_too_large` / `memory_exceeded`) 무시 + 기존 필드
//! 정상 동작" 박음.
//!
//! P-task (2026-05-10) 신규 — output.rs 안 unit test가 LOC 300 게이트
//! 위협 (mock struct 다수 + 신규 v1.1 client cover) → integration test로
//! 이전. lock test의 의도가 wire-format stability 회귀 가드라 integration
//! 위치가 architecturally 정합 (lib output module 외부에서 시뮬레이션).

use chrono::{DateTime, TimeZone, Utc};
use gitless_sync::commands::scan::compare::{FailedReason, FileEntry, LfsPointer, Status};
use gitless_sync::commands::scan::output::{SCHEMA_VERSION, ScanReport, Summary, serialize};
use serde::Deserialize;

/// v1.0 baseline 호출자 모방 — Phase 5/7 신규 필드 모름.
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

/// v1.1 baseline 호출자 모방 — Phase 5 신규 필드(`mode` / `failed_reason`
/// / `lfs_pointer`) 인지. `failed_reason`는 `Option<String>`으로 받아
/// v1.2 신규 enum 값(`file_too_large` / `memory_exceeded`)도 graceful
/// (forward-compat-aware v1.1 client baseline 모방).
#[derive(Debug, Deserialize)]
struct V11ScanReport {
    schema_version: String,
    files: Option<Vec<V11FileEntry>>,
}

#[derive(Debug, Deserialize)]
struct V11FileEntry {
    path: String,
    status: Status,
    mode: String,
    failed_reason: Option<String>,
    lfs_pointer: Option<LfsPointer>,
}

fn ts(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).unwrap()
}

fn size_gate_failed_entry(path: &str, reason: FailedReason, size: u64) -> FileEntry {
    FileEntry {
        path: path.into(),
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

fn v1_2_sample_report() -> ScanReport {
    // identical + lfs_failed (v1.1 신규 필드 활성) + file_too_large +
    // memory_exceeded (v1.2 신규 size 게이트 entry). 4 entry로 v1.0 /
    // v1.1 client backward-compat lock 시나리오 모두 cover.
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
        size_bytes: None,
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
        size_bytes: None,
    };
    let too_large = size_gate_failed_entry(
        "media/big-video.mp4",
        FailedReason::FileTooLarge,
        157_286_400,
    );
    let mem_exceeded = size_gate_failed_entry(
        "data/largish-archive.tar",
        FailedReason::MemoryExceeded,
        62_914_560,
    );
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
            failed: 3,
        },
        files: Some(vec![identical, lfs_failed, too_large, mem_exceeded]),
    }
}

fn v1_2_sample_json() -> String {
    serialize(&v1_2_sample_report(), false).expect("serialize must succeed")
}

fn parse_v1_0(json: &str) -> V10ScanReport {
    serde_json::from_str(json).expect("v1.0 client must parse v1.2 JSON")
}

fn parse_v1_1(json: &str) -> V11ScanReport {
    serde_json::from_str(json).expect("v1.1 client must parse v1.2 JSON")
}

fn raw_files(json: &str) -> Vec<serde_json::Value> {
    let raw: serde_json::Value = serde_json::from_str(json).expect("raw JSON must parse");
    raw["files"].as_array().expect("raw files array").clone()
}

/// `#[serde(deny_unknown_fields)]`을 우연히 박는 회귀 lock — v1.0 모양
/// 구조체로 v1.2 JSON envelope 필드 정합 deserialize.
#[test]
fn v1_0_client_parses_v1_2_envelope_fields() {
    let parsed = parse_v1_0(&v1_2_sample_json());
    assert_eq!(parsed.schema_version, "1.2");
    assert_eq!(parsed.repo, "owner/name");
    assert_eq!(parsed.branch, "main");
    assert_eq!(parsed.local_root, "/tmp/root");
    assert_eq!(parsed.scanned_at, ts(1_700_000_500));
    assert_eq!(parsed.summary.identical, 1);
    assert_eq!(parsed.summary.failed, 3);
}

/// v1.0 baseline Identical entry — v1.0 모든 필드 정합 (v1.1/v1.2
/// 신규 필드는 v1.0 struct에 없어 자연 무시).
#[test]
fn v1_0_client_parses_v1_2_identical_entry_fields() {
    let parsed = parse_v1_0(&v1_2_sample_json());
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

/// v1.0 client가 v1.2 신규 size 게이트 Failed entry를 status="failed"
/// 로만 인지 (`failed_reason` / `size_bytes`는 자연 무시).
#[test]
fn v1_0_client_parses_v1_2_size_gate_entries_as_failed() {
    let parsed = parse_v1_0(&v1_2_sample_json());
    let files = parsed.files.expect("files must be present");
    let too_large = &files[2];
    assert_eq!(too_large.path, "media/big-video.mp4");
    assert_eq!(too_large.status, Status::Failed);
    assert!(!too_large.is_binary);
    let mem_exceeded = &files[3];
    assert_eq!(mem_exceeded.path, "data/largish-archive.tar");
    assert_eq!(mem_exceeded.status, Status::Failed);
}

/// v1.1 client envelope + v1.1 신규 필드 정합. v1.2 신규 `size_bytes`
/// 필드는 `V11FileEntry`에 없어 자연 무시. `lfs_pointer`는 정상 read.
#[test]
fn v1_1_client_parses_v1_2_envelope_and_lfs_entry() {
    let parsed = parse_v1_1(&v1_2_sample_json());
    assert_eq!(parsed.schema_version, "1.2");
    let files = parsed.files.expect("files must be present");
    let lfs_failed = &files[1];
    assert_eq!(lfs_failed.path, "vendor/lib.zip");
    assert_eq!(lfs_failed.status, Status::Failed);
    assert_eq!(lfs_failed.mode, "100644");
    assert_eq!(lfs_failed.failed_reason.as_deref(), Some("lfs_pointer"));
    let pointer = lfs_failed
        .lfs_pointer
        .as_ref()
        .expect("lfs_pointer present");
    assert_eq!(pointer.oid, "?");
    assert_eq!(pointer.size, 0);
}

/// v1.1 client가 v1.2 신규 enum 값(`file_too_large` / `memory_exceeded`)
/// 을 graceful read (`Option<String>` baseline). `size_bytes` 필드는
/// 자연 무시. `lfs_pointer`는 None.
#[test]
fn v1_1_client_parses_v1_2_size_gate_enum_values_gracefully() {
    let parsed = parse_v1_1(&v1_2_sample_json());
    let files = parsed.files.expect("files must be present");
    let too_large = &files[2];
    assert_eq!(too_large.status, Status::Failed);
    assert_eq!(too_large.failed_reason.as_deref(), Some("file_too_large"));
    assert!(too_large.lfs_pointer.is_none());
    let mem_exceeded = &files[3];
    assert_eq!(
        mem_exceeded.failed_reason.as_deref(),
        Some("memory_exceeded")
    );
    assert!(mem_exceeded.lfs_pointer.is_none());
}

/// Identical entry는 `failed_reason` / `lfs_pointer` / `size_bytes`를
/// wire에 박지 않음. v1.0 호출자가 신규 필드 부재로 v1.0 동작 그대로.
#[test]
fn v1_2_json_omits_optional_fields_for_identical_entry() {
    let files = raw_files(&v1_2_sample_json());
    let ident = files[0].as_object().expect("identical entry object");
    assert_eq!(
        ident.get("mode"),
        Some(&serde_json::Value::String("100644".into()))
    );
    assert!(!ident.contains_key("failed_reason"));
    assert!(!ident.contains_key("lfs_pointer"));
    assert!(!ident.contains_key("size_bytes"));
}

/// v1.1 신규 필드 wire format 유지 — Failed-with-lfs_pointer entry.
/// v1.2에서도 placeholder `{oid: "?", size: 0}` 박음.
#[test]
fn v1_2_json_includes_failed_reason_and_lfs_pointer_for_lfs_entry() {
    let files = raw_files(&v1_2_sample_json());
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
