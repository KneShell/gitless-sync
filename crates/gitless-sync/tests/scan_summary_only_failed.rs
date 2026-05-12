#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Task O — F3 integration test: `--summary-only` mode failed visibility.
//!
//! cli-ux-feedback.md § F3 motivation — 한 호출로 "어떤 파일이 실패했는지"
//! 식별 가능. v1.5 spec § `--summary-only` 출력 → failed entry 한정 minimal
//! shape (`path` + `presence` + `failed_reason` 3 field) emit. real-world
//! domain trap (`long_path`) fixture + end-to-end `build_report` + serialize
//! 통합 lock.
//!
//! `status_filter.rs` unit tests (Phase 9.3 task N)는 `build_report` 단까지
//! cover — 본 integration은 `tests/` 별도 crate에서 serialize wire JSON
//! 통과 contract 박음. `status_filter` 는 submodule fixture, 본 test는
//! `long_path` fixture — 두 도메인 함정 동시 cover (regression coverage
//! 분기).

mod common;

use tempfile::TempDir;

use common::{TestGhClient, args_for, ok_resp, run_to_json, tree_args};

#[test]
fn summary_only_emits_minimal_failed_entry_for_long_path_pitfall() {
    // Phase 5 domain trap `long_path` — comparison key (≥ 260 bytes) is
    // Windows-unrepresentable without `\\?\` prefix. Remote tree carries an
    // overlong path; `short_circuit::try_short_circuit_failed` promotes the
    // entry to `Status::Failed` + `failed_reason == "long_path"` before
    // `try_hash_local` runs. Local fs untouched — the path cannot land
    // locally anyway, so the fixture is OS-portable (Linux CI runner ok).
    //
    // F3 motivation: a single `scan --summary-only` call surfaces the
    // failed path; pre-v1.5 callers had to re-invoke `scan --status failed`,
    // doubling the Trees API cost.
    let dir = TempDir::new().unwrap();
    let long_path = "a".repeat(260);

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"{long_path}","mode":"100644","type":"blob","sha":"remote-sha","size":4}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));

    let mut args = args_for(dir.path(), "o/r");
    args.summary_only = true;

    let json = run_to_json(&args, &mock);

    assert_eq!(json["summary"]["failed"], 1);
    let files = json["files"]
        .as_array()
        .expect("files[] present when failed > 0 in summary-only mode");
    assert_eq!(files.len(), 1);

    // v1.5 minimal entry shape — `path` + `presence` + `failed_reason` 3 keys
    // (spec § v1.5 acceptance #4). Detail fields stripped at wire level.
    let entry = files[0].as_object().expect("entry object");
    assert_eq!(entry.len(), 3, "minimal entry must emit exactly 3 keys");
    assert_eq!(entry["path"].as_str().unwrap(), long_path);
    assert_eq!(entry["failed_reason"], "long_path");
    assert!(
        entry.contains_key("presence"),
        "presence key required for caller G2 (presence/status orthogonal) branching"
    );

    for stripped in [
        "status",
        "local_sha",
        "remote_sha",
        "local_mtime",
        "remote_last_commit_at",
        "is_binary",
        "mode",
        "diff_meaningful",
        "lfs_pointer",
        "size_bytes",
    ] {
        assert!(
            !entry.contains_key(stripped),
            "detail key {stripped} must not leak in summary-only mode"
        );
    }
}
