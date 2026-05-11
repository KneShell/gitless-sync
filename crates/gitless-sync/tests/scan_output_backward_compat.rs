#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! v1.0 / v1.1 / v1.2 호출자가 v1.3 출력 JSON을 backward-compat 정합
//! 파싱하는지 회귀 lock. spec-output-schema.md § v1.3 신규 Acceptance
//! Criteria "v1.0 / v1.1 / v1.2 호출자가 v1.3 JSON 파싱 시 추가 필드
//! (`presence` / `diff_meaningful`) 무시 + 기존 필드 정상 동작" 박음.
//!
//! P-task (2026-05-10) 신규 — output.rs 안 unit test가 LOC 300 게이트
//! 위협 (mock struct 다수 + 신규 v1.1 client cover) → integration test로
//! 이전. lock test의 의도가 wire-format stability 회귀 가드라 integration
//! 위치가 architecturally 정합 (lib output module 외부에서 시뮬레이션).
//! L-task (2026-05-10) 갱신 — V12 client struct 추가 + v1.3 entry (presence
//! != both, `diff_meaningful` Some/None) sample 확장 + v1.3 wire 신규 invariant.

use chrono::{DateTime, TimeZone, Utc};
use gitless_sync::commands::scan::compare::{
    FailedReason, FileEntry, LfsPointer, Presence, Status,
};
use gitless_sync::commands::scan::output::{SCHEMA_VERSION, ScanReport, Summary, serialize};
use serde::Deserialize;

/// v1.0 baseline 호출자 모방 — Phase 5/7/8 신규 필드 모름.
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
/// v1.2/v1.3 신규 enum 값도 graceful (forward-compat-aware v1.1 client
/// baseline 모방).
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

/// v1.2 baseline 호출자 모방 — Phase 7 신규 필드(`size_bytes`) 인지. v1.3
/// 신규 필드(`presence` / `diff_meaningful`)는 모름. v1.2 → v1.3 forward
/// compat 회귀 가드.
#[derive(Debug, Deserialize)]
struct V12ScanReport {
    schema_version: String,
    files: Option<Vec<V12FileEntry>>,
}

#[derive(Debug, Deserialize)]
struct V12FileEntry {
    path: String,
    status: Status,
    mode: String,
    failed_reason: Option<String>,
    lfs_pointer: Option<LfsPointer>,
    size_bytes: Option<u64>,
}

fn ts(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).unwrap()
}

fn hashed_entry(
    path: &str,
    status: Status,
    presence: Presence,
    diff_meaningful: Option<bool>,
    shas: (Option<&str>, Option<&str>),
) -> FileEntry {
    let (local_sha, remote_sha) = shas;
    FileEntry {
        path: path.into(),
        status,
        presence,
        local_sha: local_sha.map(str::to_string),
        remote_sha: remote_sha.map(str::to_string),
        local_mtime: local_sha.map(|_| ts(1_700_000_000)),
        remote_last_commit_at: remote_sha.map(|_| ts(1_700_000_000)),
        is_binary: false,
        mode: "100644".into(),
        diff_meaningful,
        failed_reason: None,
        lfs_pointer: None,
        size_bytes: None,
    }
}

fn size_gate_failed_entry(path: &str, reason: FailedReason, size: u64) -> FileEntry {
    FileEntry {
        path: path.into(),
        status: Status::Failed,
        presence: Presence::Both,
        local_sha: None,
        remote_sha: None,
        local_mtime: None,
        remote_last_commit_at: None,
        is_binary: false,
        mode: "100644".into(),
        diff_meaningful: None,
        failed_reason: Some(reason),
        lfs_pointer: None,
        size_bytes: Some(size),
    }
}

fn lfs_failed_entry() -> FileEntry {
    FileEntry {
        path: "vendor/lib.zip".into(),
        status: Status::Failed,
        presence: Presence::Both,
        local_sha: None,
        remote_sha: Some("def".into()),
        local_mtime: None,
        remote_last_commit_at: None,
        is_binary: false,
        mode: "100644".into(),
        diff_meaningful: None,
        failed_reason: Some(FailedReason::LfsPointer),
        lfs_pointer: Some(LfsPointer {
            oid: "?".into(),
            size: 0,
        }),
        size_bytes: None,
    }
}

fn v1_3_baseline_entries() -> Vec<FileEntry> {
    // [0] Identical, [1] lfs_pointer Failed, [2] file_too_large,
    // [3] memory_exceeded — v1.0/v1.1/v1.2 client backward-compat baseline.
    vec![
        hashed_entry(
            "notes/foo.md",
            Status::Identical,
            Presence::Both,
            Some(false),
            (Some("abc"), Some("abc")),
        ),
        lfs_failed_entry(),
        size_gate_failed_entry(
            "media/big-video.mp4",
            FailedReason::FileTooLarge,
            157_286_400,
        ),
        size_gate_failed_entry(
            "data/largish-archive.tar",
            FailedReason::MemoryExceeded,
            62_914_560,
        ),
    ]
}

fn v1_3_new_entries() -> Vec<FileEntry> {
    // [4] Drift normalize-diff, [5] Drift normalize-equal (F1 evidence),
    // [6] LocalOnly, [7] RemoteOnly — v1.3 신규 wire invariant cover.
    vec![
        hashed_entry(
            "src/changed.rs",
            Status::Drift,
            Presence::Both,
            Some(true),
            (Some("aaa"), Some("bbb")),
        ),
        hashed_entry(
            "notes/bom-only-drift.md",
            Status::Drift,
            Presence::Both,
            Some(false),
            (Some("111"), Some("222")),
        ),
        hashed_entry(
            "drafts/local-new.md",
            Status::LocalOnlyChanged,
            Presence::LocalOnly,
            None,
            (Some("jkl"), None),
        ),
        hashed_entry(
            "remote/orphan.md",
            Status::RemoteOnlyChanged,
            Presence::RemoteOnly,
            None,
            (None, Some("mno")),
        ),
    ]
}

fn v1_3_sample_report() -> ScanReport {
    let mut entries = v1_3_baseline_entries();
    entries.extend(v1_3_new_entries());
    ScanReport {
        schema_version: SCHEMA_VERSION.to_string(),
        scanned_at: ts(1_700_000_500),
        repo: "owner/name".into(),
        branch: "main".into(),
        local_root: "/tmp/root".into(),
        summary: Summary {
            identical: 1,
            local_only_changed: 1,
            remote_only_changed: 1,
            drift: 2,
            failed: 3,
        },
        files: Some(entries),
    }
}

fn v1_3_sample_json() -> String {
    serialize(&v1_3_sample_report(), false).expect("serialize must succeed")
}

fn parse_v1_0(json: &str) -> V10ScanReport {
    serde_json::from_str(json).expect("v1.0 client must parse v1.3 JSON")
}

fn parse_v1_1(json: &str) -> V11ScanReport {
    serde_json::from_str(json).expect("v1.1 client must parse v1.3 JSON")
}

fn parse_v1_2(json: &str) -> V12ScanReport {
    serde_json::from_str(json).expect("v1.2 client must parse v1.3 JSON")
}

fn raw_files(json: &str) -> Vec<serde_json::Value> {
    let raw: serde_json::Value = serde_json::from_str(json).expect("raw JSON must parse");
    raw["files"].as_array().expect("raw files array").clone()
}

/// `#[serde(deny_unknown_fields)]`을 우연히 박는 회귀 lock — v1.0 모양
/// 구조체로 v1.3 JSON envelope 필드 정합 deserialize.
#[test]
fn v1_0_client_parses_v1_3_envelope_fields() {
    let parsed = parse_v1_0(&v1_3_sample_json());
    assert_eq!(parsed.schema_version, "1.4");
    assert_eq!(parsed.repo, "owner/name");
    assert_eq!(parsed.branch, "main");
    assert_eq!(parsed.local_root, "/tmp/root");
    assert_eq!(parsed.scanned_at, ts(1_700_000_500));
    assert_eq!(parsed.summary.identical, 1);
    assert_eq!(parsed.summary.failed, 3);
}

/// v1.0 baseline Identical entry — v1.0 모든 필드 정합 (v1.1/v1.2/v1.3
/// 신규 필드는 v1.0 struct에 없어 자연 무시).
#[test]
fn v1_0_client_parses_v1_3_identical_entry_fields() {
    let parsed = parse_v1_0(&v1_3_sample_json());
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
fn v1_0_client_parses_v1_3_size_gate_entries_as_failed() {
    let parsed = parse_v1_0(&v1_3_sample_json());
    let files = parsed.files.expect("files must be present");
    let too_large = &files[2];
    assert_eq!(too_large.path, "media/big-video.mp4");
    assert_eq!(too_large.status, Status::Failed);
    assert!(!too_large.is_binary);
    let mem_exceeded = &files[3];
    assert_eq!(mem_exceeded.path, "data/largish-archive.tar");
    assert_eq!(mem_exceeded.status, Status::Failed);
}

/// v1.0 client가 v1.3 신규 entry (`drift` / `local_only` / `remote_only`)를
/// 기존 status enum으로만 인지 (`presence` / `diff_meaningful`는 자연 무시).
#[test]
fn v1_0_client_parses_v1_3_new_entries_as_existing_status() {
    let parsed = parse_v1_0(&v1_3_sample_json());
    let files = parsed.files.expect("files must be present");
    assert_eq!(files[4].status, Status::Drift);
    assert_eq!(files[5].status, Status::Drift);
    assert_eq!(files[6].status, Status::LocalOnlyChanged);
    assert_eq!(files[6].remote_sha, None);
    assert_eq!(files[7].status, Status::RemoteOnlyChanged);
    assert_eq!(files[7].local_sha, None);
}

/// v1.1 client envelope + v1.1 신규 필드 정합. v1.2/v1.3 신규 필드는
/// `V11FileEntry`에 없어 자연 무시. `lfs_pointer`는 정상 read.
#[test]
fn v1_1_client_parses_v1_3_envelope_and_lfs_entry() {
    let parsed = parse_v1_1(&v1_3_sample_json());
    assert_eq!(parsed.schema_version, "1.4");
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

/// v1.1 client가 v1.2 신규 enum 값(`file_too_large` / `memory_exceeded`)을
/// graceful read (`Option<String>` baseline). `size_bytes`/v1.3 신규 필드는
/// 자연 무시. `lfs_pointer`는 None.
#[test]
fn v1_1_client_parses_v1_3_size_gate_enum_values_gracefully() {
    let parsed = parse_v1_1(&v1_3_sample_json());
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

/// v1.2 client envelope + v1.1 baseline 필드 (`path`/`mode`/`lfs_pointer`)
/// 정합. v1.3 신규 필드(`presence` / `diff_meaningful`)는 `V12FileEntry`에
/// 없어 자연 무시.
#[test]
fn v1_2_client_parses_v1_3_envelope_and_v1_1_baseline_fields() {
    let parsed = parse_v1_2(&v1_3_sample_json());
    assert_eq!(parsed.schema_version, "1.4");
    let files = parsed.files.expect("files must be present");
    assert_eq!(files.len(), 8);
    let lfs_failed = &files[1];
    assert_eq!(lfs_failed.path, "vendor/lib.zip");
    assert_eq!(lfs_failed.mode, "100644");
    let pointer = lfs_failed
        .lfs_pointer
        .as_ref()
        .expect("lfs_pointer present");
    assert_eq!(pointer.oid, "?");
    assert_eq!(pointer.size, 0);
}

/// v1.2 client가 v1.2 size 게이트 entry `size_bytes` 그대로 read + v1.3 신규
/// `drift`/`local_only`/`remote_only` entry는 status로만 인지 (`presence` 모름).
#[test]
fn v1_2_client_parses_v1_3_size_gate_with_size_bytes_and_new_entries() {
    let parsed = parse_v1_2(&v1_3_sample_json());
    let files = parsed.files.expect("files must be present");
    let too_large = &files[2];
    assert_eq!(too_large.failed_reason.as_deref(), Some("file_too_large"));
    assert_eq!(too_large.size_bytes, Some(157_286_400));
    let mem_exceeded = &files[3];
    assert_eq!(
        mem_exceeded.failed_reason.as_deref(),
        Some("memory_exceeded")
    );
    assert_eq!(mem_exceeded.size_bytes, Some(62_914_560));
    // v1.3 신규 entry — v1.2 client는 status enum 그대로 인지, presence 모름.
    assert_eq!(files[4].status, Status::Drift);
    assert_eq!(files[6].status, Status::LocalOnlyChanged);
    assert!(files[6].size_bytes.is_none());
    assert_eq!(files[7].status, Status::RemoteOnlyChanged);
}

/// Identical entry는 `failed_reason` / `lfs_pointer` / `size_bytes`를 wire에
/// 박지 않음. v1.3 `presence`는 emit (`both`), `diff_meaningful`는 emit
/// (`false` — Some(false) 직렬화).
#[test]
fn v1_3_json_omits_optional_fields_for_identical_entry() {
    let files = raw_files(&v1_3_sample_json());
    let ident = files[0].as_object().expect("identical entry object");
    assert_eq!(
        ident.get("mode"),
        Some(&serde_json::Value::String("100644".into()))
    );
    assert!(!ident.contains_key("failed_reason"));
    assert!(!ident.contains_key("lfs_pointer"));
    assert!(!ident.contains_key("size_bytes"));
    assert_eq!(ident["presence"], serde_json::Value::String("both".into()));
    assert_eq!(ident["diff_meaningful"], serde_json::Value::Bool(false));
}

/// v1.1 신규 필드 wire format 유지 — Failed-with-lfs_pointer entry. v1.3
/// 에서도 placeholder `{oid: "?", size: 0}` 박음.
#[test]
fn v1_3_json_includes_failed_reason_and_lfs_pointer_for_lfs_entry() {
    let files = raw_files(&v1_3_sample_json());
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

/// v1.3 신규 wire — `presence` field가 모든 entry에 emit (Failed 포함).
/// spec § "presence 필드는 Failed entry에서도 누락 안 함" 박음.
#[test]
fn v1_3_json_emits_presence_for_all_entries_including_failed() {
    let files = raw_files(&v1_3_sample_json());
    let expected = [
        "both",        // [0] identical
        "both",        // [1] lfs_failed
        "both",        // [2] file_too_large
        "both",        // [3] memory_exceeded
        "both",        // [4] drift normalize-diff
        "both",        // [5] drift normalize-equal
        "local_only",  // [6] LocalOnly
        "remote_only", // [7] RemoteOnly
    ];
    for (i, entry) in files.iter().enumerate() {
        let obj = entry.as_object().expect("entry object");
        assert_eq!(
            obj["presence"],
            serde_json::Value::String(expected[i].into()),
            "files[{i}] presence mismatch"
        );
    }
}

/// v1.3 신규 wire — `diff_meaningful` 4-case lock. presence=both Hashed
/// entry는 Some(true)/Some(false) emit, presence != both 또는 Failed entry는
/// omit. spec § "LocalOnly/RemoteOnly entry는 `diff_meaningful` 필드 omit" +
/// "Failed entry는 `diff_meaningful` 필드 omit" 박음.
#[test]
fn v1_3_json_emits_diff_meaningful_only_for_presence_both_hashed_entries() {
    let files = raw_files(&v1_3_sample_json());
    // [0] identical (presence=both, sha same) → false
    assert_eq!(files[0]["diff_meaningful"], serde_json::Value::Bool(false));
    // [1] lfs_failed Failed → omit
    assert!(
        !files[1]
            .as_object()
            .unwrap()
            .contains_key("diff_meaningful")
    );
    // [2] file_too_large Failed → omit
    assert!(
        !files[2]
            .as_object()
            .unwrap()
            .contains_key("diff_meaningful")
    );
    // [3] memory_exceeded Failed → omit
    assert!(
        !files[3]
            .as_object()
            .unwrap()
            .contains_key("diff_meaningful")
    );
    // [4] Drift normalize-diff → true
    assert_eq!(files[4]["diff_meaningful"], serde_json::Value::Bool(true));
    // [5] Drift normalize-equal → false (F1 evidence)
    assert_eq!(files[5]["diff_meaningful"], serde_json::Value::Bool(false));
    // [6] LocalOnly (presence != both) → omit
    assert!(
        !files[6]
            .as_object()
            .unwrap()
            .contains_key("diff_meaningful")
    );
    // [7] RemoteOnly (presence != both) → omit
    assert!(
        !files[7]
            .as_object()
            .unwrap()
            .contains_key("diff_meaningful")
    );
}
