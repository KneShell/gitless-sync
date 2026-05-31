#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! End-to-end tests for `gitless-sync init` (PRD scenarios 16, 17, 18, 19).
//!
//! Covers the stdout-TOML / stderr-hint redirect contract (ADR 0004) and the
//! init → scan round-trip via `gitless-sync.toml` fallback.

mod common;

use std::fs;

use tempfile::TempDir;

use gitless_sync::commands::init::{InitArgs, run as init_run};
use gitless_sync::commands::scan::run_with_client;
use gitless_sync::shared::config::Config;
use gitless_sync::shared::error::GitlessError;

use common::{TestGhClient, args_for, ok_resp, tree_args};

// ---- PRD 시나리오 16: init 정상 emit (round-trip into Config) ----------------

#[test]
fn scenario_16_init_emits_toml_that_round_trips_through_config() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let args = InitArgs {
        repo: "owner/name".to_string(),
        branch: Some("dev".to_string()),
        ignore: vec!["dist/".to_string(), "*.tmp".to_string()],
    };
    init_run(&args, &mut stdout, &mut stderr).expect("init should succeed for valid args");

    let toml_text = String::from_utf8(stdout).expect("emitted stdout bytes are utf-8");
    let parsed: Config = toml::from_str(&toml_text).expect("emitted toml should parse");

    assert_eq!(parsed.repo.as_deref(), Some("owner/name"));
    assert_eq!(parsed.branch.as_deref(), Some("dev"));
    assert_eq!(
        parsed.ignore,
        vec!["dist/".to_string(), "*.tmp".to_string()]
    );
}

// ---- PRD 시나리오 17: init 빈 repo → CONFIG error (exit 1) -------------------
//
// Plan/spec 텍스트는 `error_code() == "CONFIG"`라 적혀 있지만 실제 production
// 매핑은 `error.rs`의 `Self::Config(_) => "CONFIG_ERROR"` (다른 variant
// 매핑과 형식 일관: AUTH_FAILED / RATE_LIMIT_EXCEEDED / TREES_TRUNCATED).
// 본 테스트는 코드 baseline을 신뢰원으로 삼는다 — 시나리오 17의 contract는
// "Config variant + exit 1 + 메시지에 'repo not specified' 포함"으로 충족된다.

#[test]
fn scenario_17_init_repo_unspecified_returns_config_error() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let args = InitArgs {
        repo: String::new(),
        branch: None,
        ignore: vec![],
    };
    let err = init_run(&args, &mut stdout, &mut stderr).expect_err("empty repo must error");

    assert!(matches!(err, GitlessError::Config(_)));
    assert_eq!(err.exit_code(), 1);
    assert_eq!(err.to_stderr_payload().error_code, "CONFIG_ERROR");

    let GitlessError::Config(msg) = &err else {
        panic!("expected Config variant, got {err:?}");
    };
    assert!(
        msg.contains("repo not specified"),
        "expected 'repo not specified' substring, got: {msg}"
    );

    assert!(stdout.is_empty(), "stdout must stay empty on error");
    assert!(stderr.is_empty(), "stderr hint must not emit on error path");
}

// ---- PRD 시나리오 18: init success emits stderr hint ------------------------

#[test]
fn scenario_18_init_emits_redirect_hint_to_stderr_on_success() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let args = InitArgs {
        repo: "a/b".to_string(),
        branch: None,
        ignore: vec![],
    };
    init_run(&args, &mut stdout, &mut stderr).expect("init should succeed");

    let hint = String::from_utf8(stderr).expect("stderr bytes are utf-8");
    assert!(
        hint.contains("redirect stdout"),
        "expected 'redirect stdout' substring, got: {hint}"
    );
    assert!(
        hint.contains("gitless-sync.toml"),
        "expected hint to mention target file, got: {hint}"
    );
}

// ---- PRD 시나리오 19: init → scan round-trip --------------------------------
//
// `init`이 emit한 TOML을 tempdir에 영구화한 뒤 같은 tempdir 기반 `ScanArgs`로
// `run_with_client`를 호출. `args.repo = None`으로 두면 `build_report`가
// `gitless-sync.toml`의 `repo` 필드로 fallback한다 (`shared::config::load` +
// `args.repo.as_deref().or(cfg.repo.as_deref())` 경로). 호출이 `Ok(())`로
// 끝났다는 사실 자체가 toml 로드 + 매칭 stub 사용을 입증한다 — toml fallback이
// 실패했다면 `GitlessError::Config("repo not specified")`로 떨어졌을 것.

#[test]
fn scenario_19_init_output_round_trips_into_scan_run() {
    let dir = TempDir::new().unwrap();

    let mut emitted_stdout = Vec::new();
    let mut emitted_stderr = Vec::new();
    let init_args = InitArgs {
        repo: "rt-owner/rt-repo".to_string(),
        branch: Some("main".to_string()),
        ignore: vec![],
    };
    init_run(&init_args, &mut emitted_stdout, &mut emitted_stderr).expect("init should succeed");

    let toml_path = dir.path().join("gitless-sync.toml");
    fs::write(&toml_path, &emitted_stdout).expect("persist emitted toml to tempdir");

    let parsed: Config =
        toml::from_str(std::str::from_utf8(&emitted_stdout).unwrap()).expect("emitted toml parses");
    assert_eq!(parsed.repo.as_deref(), Some("rt-owner/rt-repo"));
    assert_eq!(parsed.branch.as_deref(), Some("main"));

    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("rt-owner/rt-repo", "main"),
        ok_resp(br#"{"sha":"x","tree":[],"truncated":false}"#),
    );

    let mut args = args_for(dir.path(), "rt-owner/rt-repo");
    args.repo = None;

    run_with_client(&args, &mock, &mut Vec::new())
        .expect("scan should load repo from emitted toml and succeed against the stub");
}
