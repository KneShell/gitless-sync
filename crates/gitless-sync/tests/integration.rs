//! End-to-end integration tests for `gitless-sync` (M4a + M4b).
//!
//! M4a covers PRD scenarios 1-4 (4-state classification) and 9 (.gitignore +
//! `--ignore` 합집합) through the library entry points with a stubbed
//! `GhClient` implementation. M4b adds scenarios 10-15 (auth / rate limit /
//! truncated / summary-only / status filter / partial failure). The JSON we
//! parse here is byte-identical to the string `run_with_client` writes to
//! stdout in production: both paths run `build_report` followed by
//! `output::serialize`, so verifying the parsed JSON exercises the same data
//! flow without requiring stdout capture. Error scenarios assert the
//! `GitlessError` variant + its `exit_code()` / `error_code()` projections,
//! which is what `main.rs` maps to the binary's exit code and stderr JSON.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;
use tempfile::TempDir;

use gitless_sync::commands::init::{InitArgs, run as init_run};
use gitless_sync::commands::scan::output::serialize;
use gitless_sync::commands::scan::{Backend, ScanArgs, build_report, run_with_client};
use gitless_sync::shared::config::Config;
use gitless_sync::shared::error::GitlessError;
use gitless_sync::shared::gh::{GhClient, GhResponse};
use gitless_sync::shared::hash::blob_hash;
use gitless_sync::shared::normalize::prepare_for_hash;

// ---- TestGhClient: argv → canned response (mirrors the in-crate MockGhClient)
//
// `graphql_response` provides a single wildcard stub for `gh api graphql ...`
// invocations. Production `build_query` is module-private and the query string
// changes per chunk, so an exact-argv match would require duplicating the
// query builder here. The wildcard pattern matches any `api graphql ...` argv
// and lets scenario tests inject one canonical response per scan.

struct TestGhClient {
    responses: HashMap<Vec<String>, GhResponse>,
    graphql_response: Option<GhResponse>,
}

impl TestGhClient {
    fn new() -> Self {
        Self {
            responses: HashMap::new(),
            graphql_response: None,
        }
    }

    fn stub(&mut self, args: Vec<String>, response: GhResponse) {
        self.responses.insert(args, response);
    }

    fn stub_graphql(&mut self, response: GhResponse) {
        self.graphql_response = Some(response);
    }
}

impl GhClient for TestGhClient {
    fn api(&self, args: &[String]) -> Result<GhResponse, GitlessError> {
        if args.first().map(String::as_str) == Some("api")
            && args.get(1).map(String::as_str) == Some("graphql")
            && let Some(r) = self.graphql_response.as_ref()
        {
            return Ok(r.clone());
        }
        match self.responses.get(args) {
            Some(r) => Ok(r.clone()),
            None => Err(GitlessError::Http(format!(
                "TestGhClient: no stub registered for args {args:?}"
            ))),
        }
    }
}

fn ok_resp(body: &[u8]) -> GhResponse {
    GhResponse {
        stdout: body.to_vec(),
        stderr: String::new(),
        exit_code: 0,
    }
}

fn tree_args(repo: &str, branch: &str) -> Vec<String> {
    vec![
        "api".to_string(),
        format!("repos/{repo}/git/trees/{branch}?recursive=1"),
    ]
}

fn commits_args(repo: &str, branch: &str, path: &str) -> Vec<String> {
    vec![
        "api".to_string(),
        "-X".to_string(),
        "GET".to_string(),
        format!("repos/{repo}/commits"),
        "-F".to_string(),
        format!("sha={branch}"),
        "-F".to_string(),
        format!("path={path}"),
        "-F".to_string(),
        "per_page=1".to_string(),
    ]
}

fn args_for(dir: &Path, repo: &str) -> ScanArgs {
    ScanArgs {
        repo: Some(repo.to_string()),
        branch: "main".to_string(),
        local: dir.to_str().unwrap().to_string(),
        ignore: vec![],
        keep_bom: false,
        pretty: false,
        summary_only: false,
        status: None,
        backend: Backend::Rest,
        verbose: 0,
    }
}

fn lf_blob_hash(text_lf: &str) -> String {
    let (prepared, _) = prepare_for_hash(text_lf.as_bytes(), false);
    blob_hash(&prepared)
}

fn read_mtime_rfc3339(path: &Path) -> String {
    let modified = fs::metadata(path).unwrap().modified().unwrap();
    let dt: DateTime<Utc> = modified.into();
    dt.to_rfc3339_opts(SecondsFormat::Nanos, true)
}

fn commits_body_with_date(date: &str) -> String {
    format!(
        r#"[{{"sha":"c1","commit":{{"author":{{"name":"a","email":"a@e","date":"{date}"}},"committer":{{"name":"c","email":"c@e","date":"{date}"}},"message":"m"}},"url":"u"}}]"#
    )
}

fn run_to_json(args: &ScanArgs, client: &TestGhClient) -> Value {
    let (report, _failed) = build_report(args, client).expect("build_report");
    let json = serialize(&report, false).expect("serialize");
    serde_json::from_str(&json).expect("parse JSON")
}

// ---- PRD 시나리오 1: 양쪽 SHA 동일 → Identical ----------------------------

#[test]
fn scenario_1_identical_when_shas_match() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
    let local_sha = lf_blob_hash("alpha\n");

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_sha}","size":6}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));
    // No commits stub: identical entries skip the Commits API (G-003).

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["identical"], 1);
    assert_eq!(json["summary"]["local_only_changed"], 0);
    assert_eq!(json["summary"]["remote_only_changed"], 0);
    assert_eq!(json["summary"]["drift"], 0);
    assert_eq!(json["summary"]["failed"], 0);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], "a.md");
    assert_eq!(files[0]["status"], "identical");
}

// ---- PRD 시나리오 2: 원격 last_commit < 로컬 mtime → LocalOnlyChanged ------

#[test]
fn scenario_2_local_only_changed_when_remote_commit_older() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha-local\n").unwrap();

    let mut mock = TestGhClient::new();
    let trees_body = r#"{"sha":"x","tree":[{"path":"a.md","mode":"100644","type":"blob","sha":"deadbeef","size":12}],"truncated":false}"#;
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));
    mock.stub(
        commits_args("o/r", "main", "a.md"),
        ok_resp(commits_body_with_date("2020-01-01T00:00:00Z").as_bytes()),
    );

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["local_only_changed"], 1);
    assert_eq!(json["summary"]["identical"], 0);
    assert_eq!(json["summary"]["remote_only_changed"], 0);
    assert_eq!(json["summary"]["drift"], 0);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files[0]["status"], "local_only_changed");
}

// ---- PRD 시나리오 3: 로컬 mtime < 원격 last_commit → RemoteOnlyChanged -----

#[test]
fn scenario_3_remote_only_changed_when_local_mtime_older() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha-local\n").unwrap();

    let mut mock = TestGhClient::new();
    let trees_body = r#"{"sha":"x","tree":[{"path":"a.md","mode":"100644","type":"blob","sha":"deadbeef","size":12}],"truncated":false}"#;
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));
    mock.stub(
        commits_args("o/r", "main", "a.md"),
        ok_resp(commits_body_with_date("2099-01-01T00:00:00Z").as_bytes()),
    );

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["remote_only_changed"], 1);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files[0]["status"], "remote_only_changed");
}

// ---- PRD 시나리오 4: 양쪽 다른 SHA + 시간 동률 → Drift (G-005) -----------

#[test]
fn scenario_4_drift_when_times_tie() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.md");
    fs::write(&path, "alpha-local\n").unwrap();
    let mtime_str = read_mtime_rfc3339(&path);

    let mut mock = TestGhClient::new();
    let trees_body = r#"{"sha":"x","tree":[{"path":"a.md","mode":"100644","type":"blob","sha":"deadbeef","size":12}],"truncated":false}"#;
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));
    mock.stub(
        commits_args("o/r", "main", "a.md"),
        ok_resp(commits_body_with_date(&mtime_str).as_bytes()),
    );

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["drift"], 1);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files[0]["status"], "drift");
}

// ---- err_resp helper for M4b error scenarios ----------------------------

fn err_resp(stderr: &str) -> GhResponse {
    GhResponse {
        stdout: Vec::new(),
        stderr: stderr.to_string(),
        exit_code: 1,
    }
}

// ---- PRD 시나리오 9: .gitignore + --ignore 합집합 ------------------------

#[test]
fn scenario_9_gitignore_and_ignore_arg_form_union() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();
    fs::create_dir(dir.path().join("build")).unwrap();
    fs::write(dir.path().join("build").join("artifact.bin"), "x").unwrap();
    fs::write(dir.path().join("debug.log"), "trace").unwrap();
    fs::write(dir.path().join("notes.md"), "alpha\n").unwrap();

    let mut mock = TestGhClient::new();
    mock.stub(
        tree_args("o/r", "main"),
        ok_resp(br#"{"sha":"x","tree":[],"truncated":false}"#),
    );

    let mut args = args_for(dir.path(), "o/r");
    args.ignore = vec!["*.log".to_string()];

    let json = run_to_json(&args, &mock);
    let files = json["files"].as_array().unwrap();
    let paths: Vec<&str> = files.iter().map(|e| e["path"].as_str().unwrap()).collect();

    assert!(
        !paths.iter().any(|p| p.starts_with("build/")),
        "expected `.gitignore` to prune `build/`, got: {paths:?}"
    );
    assert!(
        !paths.contains(&"debug.log"),
        "expected `--ignore *.log` to prune `debug.log`, got: {paths:?}"
    );
    assert!(
        paths.contains(&"notes.md"),
        "expected `notes.md` to survive both ignore sources, got: {paths:?}"
    );
}

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

// ---- PRD 시나리오 13: --summary-only drops files[] field --------------------

#[test]
fn scenario_13_summary_only_drops_files_array() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
    let local_sha = lf_blob_hash("alpha\n");

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_sha}","size":6}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));

    let mut args = args_for(dir.path(), "o/r");
    args.summary_only = true;

    let json = run_to_json(&args, &mock);
    assert_eq!(json["summary"]["identical"], 1);
    assert!(
        json.get("files").is_none(),
        "summary-only must omit `files` field, got: {json}"
    );
}

// ---- PRD 시나리오 14: --status filter narrows files[] -----------------------

#[test]
fn scenario_14_status_filter_keeps_only_matching_entries() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("identical.md"), "alpha\n").unwrap();
    fs::write(dir.path().join("local_only.md"), "beta\n").unwrap();
    let local_a = lf_blob_hash("alpha\n");

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"identical.md","mode":"100644","type":"blob","sha":"{local_a}","size":6}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));

    let mut args = args_for(dir.path(), "o/r");
    args.status = Some("local_only_changed".to_string());

    let json = run_to_json(&args, &mock);
    // summary counts every classified entry; only `files[]` is filtered down.
    assert_eq!(json["summary"]["identical"], 1);
    assert_eq!(json["summary"]["local_only_changed"], 1);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["status"], "local_only_changed");
    assert_eq!(files[0]["path"], "local_only.md");
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

    run_with_client(&args, &mock)
        .expect("scan should load repo from emitted toml and succeed against the stub");
}

// ===========================================================================
// P5c — PRD scenarios 20-25 (GraphQL backend + mtime cache)
// ===========================================================================
//
// Scenarios 20-21 exercise the GraphQL backend (ADR 0006) end-to-end through
// `build_report` / `run_with_client`. Scenarios 22-23, 25 exercise the mtime
// cache (ADR 0009) by running the same `ScanArgs` twice and asserting the
// observable effect on the second pass — `build_report` writes the cache to
// the OS user-cache directory between calls. Scenario 24 asserts cross-backend
// equivalence: REST and GraphQL must produce byte-identical `summary` and
// `files[]` sets when fed equivalent stub data.
//
// Tests rely on each scenario owning a unique `repo` slug so the per-scan
// cache file is isolated from siblings, and they delete the cache file up
// front to start every run from a known state.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Duration;

fn graphql_ok_body(entries: &[(&str, &str)]) -> String {
    let mut alias_entries = String::new();
    for (i, (_, date)) in entries.iter().enumerate() {
        if i > 0 {
            alias_entries.push(',');
        }
        let _ = write!(
            alias_entries,
            r#""a{i}":{{"nodes":[{{"committedDate":"{date}"}}]}}"#
        );
    }
    format!(r#"{{"data":{{"repository":{{"ref":{{"target":{{{alias_entries}}}}}}}}},"errors":[]}}"#)
}

fn graphql_err_body(code: &str, message: &str) -> String {
    format!(
        r#"{{"data":null,"errors":[{{"message":"{message}","extensions":{{"code":"{code}"}}}}]}}"#
    )
}

/// Mirrors `crate::shared::cache::cache_path` so integration tests can locate
/// (and clobber) the cache file without exposing the production helper.
/// Sanitization rules must match `shared::cache::sanitize_component`; the spec
/// fixtures in `cache.rs::tests` keep them aligned.
fn cache_file_for(repo: &str, branch: &str) -> PathBuf {
    let base = dirs::cache_dir().expect("OS user-cache directory available");
    let filename = format!(
        "{}__{}.json",
        sanitize_component_local(repo),
        sanitize_component_local(branch),
    );
    base.join("gitless-sync").join(filename)
}

fn sanitize_component_local(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '/' {
            out.push_str("__");
        } else if matches!(c, '<' | '>' | ':' | '"' | '\\' | '|' | '?' | '*') {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    out
}

fn cleanup_cache_for(repo: &str, branch: &str) {
    let path = cache_file_for(repo, branch);
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
}

// ---- PRD 시나리오 20: GraphQL backend 정상 (drift trigger → committedDate 매핑) ----

#[test]
fn scenario_20_graphql_backend_returns_normal_scan_report() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha-local\n").unwrap();

    let repo = "p5c-test/scenario-20-graphql-ok";
    let branch = "main";
    cleanup_cache_for(repo, branch);

    let mut mock = TestGhClient::new();
    let trees_body = r#"{"sha":"x","tree":[{"path":"a.md","mode":"100644","type":"blob","sha":"remote-different","size":12}],"truncated":false}"#;
    mock.stub(tree_args(repo, branch), ok_resp(trees_body.as_bytes()));
    // Old committedDate < local mtime → LocalOnlyChanged. The point is to
    // prove the GraphQL response is consumed (not the REST commits stub).
    mock.stub_graphql(ok_resp(
        graphql_ok_body(&[("a.md", "2020-01-01T00:00:00Z")]).as_bytes(),
    ));

    let mut args = args_for(dir.path(), repo);
    args.backend = Backend::Graphql;
    let json = run_to_json(&args, &mock);

    assert_eq!(json["summary"]["local_only_changed"], 1);
    assert_eq!(json["summary"]["identical"], 0);
    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], "a.md");
    assert_eq!(files[0]["status"], "local_only_changed");
    assert!(files[0]["remote_last_commit_at"].is_string());
}

// ---- PRD 시나리오 21: GraphQL backend errors → RateLimit / Auth / NOT_FOUND ----

#[test]
fn scenario_21_graphql_rate_limited_extension_maps_to_rate_limit_exceeded() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "x\n").unwrap();
    let repo = "p5c-test/scenario-21-rate";
    let branch = "main";
    cleanup_cache_for(repo, branch);

    let mut mock = TestGhClient::new();
    let trees_body = r#"{"sha":"x","tree":[{"path":"a.md","mode":"100644","type":"blob","sha":"different","size":2}],"truncated":false}"#;
    mock.stub(tree_args(repo, branch), ok_resp(trees_body.as_bytes()));
    mock.stub_graphql(ok_resp(
        graphql_err_body("RATE_LIMITED", "throttled").as_bytes(),
    ));

    let mut args = args_for(dir.path(), repo);
    args.backend = Backend::Graphql;
    let err = build_report(&args, &mock).expect_err("graphql RATE_LIMITED must propagate");

    assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
    assert_eq!(err.exit_code(), 3);
    assert_eq!(err.to_stderr_payload().error_code, "RATE_LIMIT_EXCEEDED");
}

#[test]
fn scenario_21_graphql_unauthenticated_extension_maps_to_auth_failed() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "x\n").unwrap();
    let repo = "p5c-test/scenario-21-auth";
    let branch = "main";
    cleanup_cache_for(repo, branch);

    let mut mock = TestGhClient::new();
    let trees_body = r#"{"sha":"x","tree":[{"path":"a.md","mode":"100644","type":"blob","sha":"different","size":2}],"truncated":false}"#;
    mock.stub(tree_args(repo, branch), ok_resp(trees_body.as_bytes()));
    mock.stub_graphql(ok_resp(
        graphql_err_body("UNAUTHENTICATED", "login required").as_bytes(),
    ));

    let mut args = args_for(dir.path(), repo);
    args.backend = Backend::Graphql;
    let err = build_report(&args, &mock).expect_err("graphql UNAUTHENTICATED must propagate");

    assert!(matches!(err, GitlessError::AuthFailed));
    assert_eq!(err.exit_code(), 2);
    assert_eq!(err.to_stderr_payload().error_code, "AUTH_FAILED");
}

#[test]
fn scenario_21_graphql_not_found_extension_falls_through_to_http() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "x\n").unwrap();
    let repo = "p5c-test/scenario-21-notfound";
    let branch = "main";
    cleanup_cache_for(repo, branch);

    let mut mock = TestGhClient::new();
    let trees_body = r#"{"sha":"x","tree":[{"path":"a.md","mode":"100644","type":"blob","sha":"different","size":2}],"truncated":false}"#;
    mock.stub(tree_args(repo, branch), ok_resp(trees_body.as_bytes()));
    mock.stub_graphql(ok_resp(
        graphql_err_body("NOT_FOUND", "object missing").as_bytes(),
    ));

    let mut args = args_for(dir.path(), repo);
    args.backend = Backend::Graphql;
    let err = build_report(&args, &mock).expect_err("graphql NOT_FOUND must propagate");

    match err {
        GitlessError::Http(msg) => {
            assert!(msg.contains("NOT_FOUND"), "got: {msg}");
            assert!(msg.contains("object missing"), "got: {msg}");
        }
        other => panic!("expected Http, got {other:?}"),
    }
}

// ---- PRD 시나리오 22: cache miss → hit (mtime 보존 + 내용 변경 → cache hit이면 1차 SHA 그대로) ----

#[test]
fn scenario_22_cache_hit_reuses_sha_when_mtime_preserved() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.md");
    fs::write(&path, "alpha\n").unwrap();
    let original_mtime = fs::metadata(&path).unwrap().modified().unwrap();
    let local_sha = lf_blob_hash("alpha\n");

    let repo = "p5c-test/scenario-22-cache-hit";
    let branch = "main";
    cleanup_cache_for(repo, branch);

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_sha}","size":6}}],"truncated":false}}"#
    );
    mock.stub(tree_args(repo, branch), ok_resp(trees_body.as_bytes()));

    let args = args_for(dir.path(), repo);

    // 1차 scan — cache miss → hash + populate. SHA matches the tree → identical.
    let json1 = run_to_json(&args, &mock);
    assert_eq!(json1["summary"]["identical"], 1);

    // Rewrite raw bytes with different content but restore the original mtime.
    // A cache *miss* on pass 2 would re-hash and detect divergence
    // (LocalOnlyChanged); a cache *hit* keeps the cached SHA → identical.
    fs::write(&path, "DIFFERENT-CONTENT-WOULD-HASH-ELSEWHERE\n").unwrap();
    let f = fs::OpenOptions::new().write(true).open(&path).unwrap();
    f.set_modified(original_mtime)
        .expect("std::fs::File::set_modified must succeed (Rust >= 1.75)");
    drop(f);

    let json2 = run_to_json(&args, &mock);
    assert_eq!(
        json2["summary"]["identical"], 1,
        "cache hit should reuse the cached SHA → identical preserved"
    );
    assert_eq!(json2["summary"]["local_only_changed"], 0);
}

// ---- PRD 시나리오 23: cache invalidate (mtime 변경 시 re-hash) ----

#[test]
fn scenario_23_cache_invalidate_rehashes_when_mtime_changes() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("a.md");
    fs::write(&path, "alpha\n").unwrap();
    let local_sha = lf_blob_hash("alpha\n");

    let repo = "p5c-test/scenario-23-cache-invalidate";
    let branch = "main";
    cleanup_cache_for(repo, branch);

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_sha}","size":6}}],"truncated":false}}"#
    );
    mock.stub(tree_args(repo, branch), ok_resp(trees_body.as_bytes()));
    // Pass 2 hashes the rewritten file → SHA diverges from tree, classifier
    // hits the drift branch and calls Commits API. Old commit date forces
    // LocalOnlyChanged so the cache-invalidate signal is unambiguous.
    mock.stub(
        commits_args(repo, branch, "a.md"),
        ok_resp(commits_body_with_date("2020-01-01T00:00:00Z").as_bytes()),
    );

    let args = args_for(dir.path(), repo);

    // 1차 scan — identical with cached SHA stored against the initial mtime.
    let json1 = run_to_json(&args, &mock);
    assert_eq!(json1["summary"]["identical"], 1);

    // Wait long enough for the mtime to actually advance (NTFS / ext4 / APFS
    // all resolve at ≤ 1 ms in practice; 50 ms is comfortably above noise).
    std::thread::sleep(Duration::from_millis(50));
    fs::write(&path, "BETA-LOCAL-DRIFT\n").unwrap();

    // mtime moved → cache lookup misses → re-hash. New SHA != tree sha →
    // LocalOnlyChanged.
    let json2 = run_to_json(&args, &mock);
    assert_eq!(
        json2["summary"]["identical"], 0,
        "stale mtime should invalidate cache and re-hash"
    );
    assert_eq!(json2["summary"]["local_only_changed"], 1);
}

// ---- PRD 시나리오 24: REST / GraphQL 결과 정합성 (summary + files set 동일) -----

#[test]
fn scenario_24_cross_backend_produces_identical_report() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha-local\n").unwrap();
    let repo = "p5c-test/scenario-24-cross-backend";
    let branch = "main";
    cleanup_cache_for(repo, branch);

    let trees_body = r#"{"sha":"x","tree":[{"path":"a.md","mode":"100644","type":"blob","sha":"deadbeef","size":12}],"truncated":false}"#;
    let commit_date = "2020-01-01T00:00:00Z";

    // REST scan.
    let mut rest_mock = TestGhClient::new();
    rest_mock.stub(tree_args(repo, branch), ok_resp(trees_body.as_bytes()));
    rest_mock.stub(
        commits_args(repo, branch, "a.md"),
        ok_resp(commits_body_with_date(commit_date).as_bytes()),
    );
    let mut rest_args = args_for(dir.path(), repo);
    rest_args.backend = Backend::Rest;
    let mut rest_json = run_to_json(&rest_args, &rest_mock);

    // GraphQL scan against equivalent stub data (same tree, same commit time).
    let mut graphql_mock = TestGhClient::new();
    graphql_mock.stub(tree_args(repo, branch), ok_resp(trees_body.as_bytes()));
    graphql_mock.stub_graphql(ok_resp(
        graphql_ok_body(&[("a.md", commit_date)]).as_bytes(),
    ));
    let mut graphql_args = args_for(dir.path(), repo);
    graphql_args.backend = Backend::Graphql;
    let mut graphql_json = run_to_json(&graphql_args, &graphql_mock);

    // `summary` must match exactly.
    assert_eq!(
        rest_json["summary"], graphql_json["summary"],
        "REST vs GraphQL summary diverged: rest={} graphql={}",
        rest_json["summary"], graphql_json["summary"]
    );

    // `files[]` set must match (order is BTreeSet-stable, so a direct compare
    // works — but we sort defensively in case future code introduces
    // backend-specific ordering).
    let rest_files = rest_json["files"].as_array_mut().unwrap();
    let graphql_files = graphql_json["files"].as_array_mut().unwrap();
    rest_files.sort_by(|a, b| a["path"].as_str().unwrap().cmp(b["path"].as_str().unwrap()));
    graphql_files.sort_by(|a, b| a["path"].as_str().unwrap().cmp(b["path"].as_str().unwrap()));
    assert_eq!(rest_files, graphql_files);
}

// ---- PRD 시나리오 25: cache 파일 손상 → graceful (warning + scan 정상) ----

#[test]
fn scenario_25_corrupt_cache_file_resets_gracefully() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
    let local_sha = lf_blob_hash("alpha\n");

    let repo = "p5c-test/scenario-25-cache-corrupt";
    let branch = "main";
    cleanup_cache_for(repo, branch);

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_sha}","size":6}}],"truncated":false}}"#
    );
    mock.stub(tree_args(repo, branch), ok_resp(trees_body.as_bytes()));

    let args = args_for(dir.path(), repo);

    // 1차 scan — populates the cache file on disk.
    let json1 = run_to_json(&args, &mock);
    assert_eq!(json1["summary"]["identical"], 1);

    // Corrupt the on-disk cache with non-JSON garbage. `Cache::load` must
    // detect the parse failure, emit a warning, and return `Cache::default()`.
    let cache_path = cache_file_for(repo, branch);
    assert!(
        cache_path.exists(),
        "first scan must have written a cache file at {}",
        cache_path.display()
    );
    fs::write(&cache_path, b"INVALID-JSON-GARBAGE-NOT-PARSABLE").unwrap();

    // 2차 scan — graceful fallback. Behaves like a cold run; the report shape
    // is identical to pass 1 because the file/tree haven't changed.
    let json2 = run_to_json(&args, &mock);
    assert_eq!(
        json2["summary"]["identical"], 1,
        "corrupt cache must reset to default and let the scan complete"
    );

    // The corrupt bytes must have been overwritten by `save` at the end of
    // pass 2 — round-tripping confirms the cache lifecycle recovered cleanly.
    let after = fs::read(&cache_path).unwrap();
    assert_ne!(
        after, b"INVALID-JSON-GARBAGE-NOT-PARSABLE",
        "save at end of pass 2 should overwrite the garbage with valid JSON"
    );
    let parsed: Value = serde_json::from_slice(&after).expect("rewritten cache must be valid JSON");
    assert_eq!(parsed["version"], 1, "version field after recovery");
}
