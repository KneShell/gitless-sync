#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! End-to-end error-mapping tests for `gitless-sync scan` (PRD scenarios 10,
//! 11, 12, 15).
//!
//! Asserts the `GitlessError` variant + its `exit_code()` / `error_code()`
//! projections, which is what `main.rs` maps to the binary's exit code and
//! stderr JSON. Scenario 15 (partial failure) is Windows-only because std API
//! alone can't reliably trigger a hash failure cross-platform.

mod common;

use std::fs;

use tempfile::TempDir;

use gitless_sync::commands::scan::build_report;
use gitless_sync::shared::error::GitlessError;

use common::{TestGhClient, args_for, err_resp, ok_resp, tree_args};

// ---- PRD 시나리오 10: 인증 실패 → AuthFailed (exit 2 + AUTH_FAILED) ---------

#[test]
fn scenario_10_auth_failed_when_gh_returns_bad_credentials() {
    let dir = TempDir::new().unwrap();
    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        err_resp("gh: Bad credentials (HTTP 401)"),
    );

    let args = args_for(dir.path(), "o/r");
    let err = build_report(&args, &mock).expect_err("build_report should propagate AuthFailed");

    assert!(matches!(err, GitlessError::AuthFailed));
    assert_eq!(err.exit_code(), 2);
    assert_eq!(err.to_stderr_payload().error_code, "AUTH_FAILED");
}

// ---- PRD 시나리오 11: rate limit → RateLimitExceeded (exit 3 + RATE_LIMIT_EXCEEDED) ---

#[test]
fn scenario_11_rate_limit_when_gh_returns_primary_rate_limit_stderr() {
    let dir = TempDir::new().unwrap();
    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        err_resp("gh: API rate limit exceeded for user XXX. (HTTP 403)"),
    );

    let args = args_for(dir.path(), "o/r");
    let err = build_report(&args, &mock).expect_err("build_report should propagate rate limit");

    assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
    assert_eq!(err.exit_code(), 3);
    assert_eq!(err.to_stderr_payload().error_code, "RATE_LIMIT_EXCEEDED");
}

#[test]
fn scenario_11_secondary_rate_limit_maps_to_same_variant_and_exit_code() {
    let dir = TempDir::new().unwrap();
    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        err_resp("gh: You have exceeded a secondary rate limit ... (HTTP 403)"),
    );

    let args = args_for(dir.path(), "o/r");
    let err =
        build_report(&args, &mock).expect_err("build_report should propagate secondary rate limit");

    assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
    assert_eq!(err.exit_code(), 3);
    assert_eq!(err.to_stderr_payload().error_code, "RATE_LIMIT_EXCEEDED");
}

// ---- PRD 시나리오 12: trees truncated → TreesTruncated (exit 5) ------------

#[test]
fn scenario_12_trees_truncated_when_response_flag_set() {
    let dir = TempDir::new().unwrap();
    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        ok_resp(br#"{"sha":"x","tree":[],"truncated":true}"#),
    );

    let args = args_for(dir.path(), "o/r");
    let err = build_report(&args, &mock).expect_err("build_report should propagate truncation");

    assert!(matches!(err, GitlessError::TreesTruncated));
    assert_eq!(err.exit_code(), 5);
    assert_eq!(err.to_stderr_payload().error_code, "TREES_TRUNCATED");
}

// ---- PRD 시나리오 15: partial failure (Windows: file lock 활용) -------------
//
// std API만으로 크로스 플랫폼 hash 실패 유도가 어려워 Windows 전용. CLAUDE.md
// "OS: Windows 1차 타겟" 정책에 맞춰 Windows에서만 실행한다. Unix는 walker.rs의
// `skips_symlinks_on_unix`처럼 별도 cfg 분기가 가능하지만, partial failure 검증
// 자체는 본 시나리오로 단조 충족된다.

#[cfg(windows)]
#[test]
fn scenario_15_partial_failure_when_local_file_unreadable() {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;

    use gitless_sync::commands::scan::run_with_client;

    use common::run_to_json;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("locked.md");
    fs::write(&path, "secret\n").unwrap();

    // share_mode(0)으로 후속 CreateFile은 ERROR_SHARING_VIOLATION을 받는다. walker는
    // 디렉토리 enumeration의 캐시된 metadata로 entry를 발견하지만 try_hash_local의
    // fs::read는 핸들 open 단계에서 실패 → Status::Failed.
    let _lock = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .expect("acquire exclusive lock on locked.md");

    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        ok_resp(br#"{"sha":"x","tree":[],"truncated":false}"#),
    );

    let args = args_for(dir.path(), "o/r");

    // (a) build_report로 stdout JSON 형상 검증.
    let json = run_to_json(&args, &mock);
    assert_eq!(json["summary"]["failed"], 1);
    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], "locked.md");
    assert_eq!(files[0]["status"], "failed");

    // (b) run_with_client는 failed_count > 0을 PartialFailure로 매핑 (exit 4).
    let err = run_with_client(&args, &mock)
        .expect_err("run_with_client should map failed_count > 0 to PartialFailure");
    match err {
        GitlessError::PartialFailure { failed_count } => assert_eq!(failed_count, 1),
        other => panic!("expected PartialFailure, got {other:?}"),
    }
    assert_eq!(err.exit_code(), 4);
}
