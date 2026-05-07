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

use gitless_sync::commands::scan::output::serialize;
use gitless_sync::commands::scan::{Backend, ScanArgs, build_report};
use gitless_sync::shared::error::GitlessError;
use gitless_sync::shared::gh::{GhClient, GhResponse};
use gitless_sync::shared::hash::blob_hash;
use gitless_sync::shared::normalize::prepare_for_hash;

// ---- TestGhClient: argv → canned response (mirrors the in-crate MockGhClient)

struct TestGhClient {
    responses: HashMap<Vec<String>, GhResponse>,
}

impl TestGhClient {
    fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    fn stub(&mut self, args: Vec<String>, response: GhResponse) {
        self.responses.insert(args, response);
    }
}

impl GhClient for TestGhClient {
    fn api(&self, args: &[String]) -> Result<GhResponse, GitlessError> {
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

    use gitless_sync::commands::scan::run_with_client;

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
