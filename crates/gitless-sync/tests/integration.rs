//! End-to-end integration tests for `gitless-sync` (M4a).
//!
//! Drives PRD scenarios 1-4 (4-state classification) and 9 (.gitignore +
//! `--ignore` 합집합) through the library entry points with a stubbed
//! `GhClient` implementation. The JSON we parse here is byte-identical to the
//! string `run_with_client` writes to stdout in production: both paths run
//! `build_report` followed by `output::serialize`, so verifying the parsed
//! JSON exercises the same data flow without requiring stdout capture.

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
