//! End-to-end integration tests for `gitless-sync` (T11 / PRD scenarios 1-4, 9-15).
//!
//! Each test launches the real `gitless-sync` binary via `assert_cmd`, points it
//! at a `mockito` HTTP server through the `GITLESS_API_BASE` environment
//! variable (testability scaffolding wired in `main.rs`), and asserts on
//! stdout / stderr / exit code.

use std::fs::{self, File, FileTimes};
use std::path::Path;
use std::time::{Duration, SystemTime};

use assert_cmd::Command;
use mockito::{Matcher, Server};
use predicates::prelude::*;
use serde_json::Value;
use sha1::{Digest, Sha1};
use tempfile::TempDir;

const COMMIT_DATE_ISO: &str = "2023-11-14T22:13:20Z";
const COMMIT_DATE_SECS: u64 = 1_700_000_000;

fn commits_body(date: &str) -> String {
    format!(
        r#"[{{"sha":"c1","commit":{{"author":{{"name":"a","email":"e","date":"{date}"}},"committer":{{"name":"c","email":"e","date":"{date}"}},"message":"m"}},"url":"u"}}]"#
    )
}

/// Mirror of `shared::hash::blob_hash`. Recreated here because integration
/// tests live outside the binary crate and cannot import its private modules.
fn blob_hash(content: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", content.len()).as_bytes());
    hasher.update(content);
    hasher.finalize().iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn pin_mtime(path: &Path, secs: u64) {
    let f = File::options()
        .write(true)
        .open(path)
        .expect("open for set_times");
    let when = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
    f.set_times(FileTimes::new().set_modified(when))
        .expect("set_times");
}

fn base_cmd(dir: &Path, server_url: Option<&str>) -> Command {
    let mut cmd = Command::cargo_bin("gitless-sync").expect("locate gitless-sync binary");
    cmd.env_remove("GITHUB_TOKEN");
    if let Some(url) = server_url {
        cmd.env("GITLESS_API_BASE", url);
    } else {
        cmd.env_remove("GITLESS_API_BASE");
    }
    cmd.args([
        "--repo",
        "o/r",
        "--token",
        "literal:tok",
        "--local",
        dir.to_str().expect("utf-8 tempdir path"),
    ]);
    cmd
}

fn parse_json(stdout: &[u8]) -> Value {
    serde_json::from_slice(stdout).expect("stdout is valid JSON")
}

#[test]
fn scenario_1_to_4_classifies_all_four_statuses() {
    let dir = TempDir::new().unwrap();

    let identical_content = "identical-content\n";
    fs::write(dir.path().join("identical.md"), identical_content).unwrap();
    let identical_sha = blob_hash(identical_content.as_bytes());

    fs::write(dir.path().join("local_only.md"), "local-only\n").unwrap();

    let drift_path = dir.path().join("drift.md");
    fs::write(&drift_path, "local-version\n").unwrap();
    pin_mtime(&drift_path, COMMIT_DATE_SECS);

    let mut server = Server::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[
            {{"path":"identical.md","mode":"100644","type":"blob","sha":"{identical_sha}","size":18}},
            {{"path":"remote_only.md","mode":"100644","type":"blob","sha":"deadbeef","size":3}},
            {{"path":"drift.md","mode":"100644","type":"blob","sha":"remote-different","size":14}}
        ],"truncated":false}}"#
    );
    let _trees = server
        .mock("GET", "/repos/o/r/git/trees/main")
        .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
        .with_status(200)
        .with_body(&trees_body)
        .create();
    let _commits = server
        .mock("GET", "/repos/o/r/commits")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("sha".into(), "main".into()),
            Matcher::UrlEncoded("path".into(), "drift.md".into()),
            Matcher::UrlEncoded("per_page".into(), "1".into()),
        ]))
        .with_status(200)
        .with_body(commits_body(COMMIT_DATE_ISO))
        .create();

    let mut cmd = base_cmd(dir.path(), Some(&server.url()));
    cmd.arg("scan");
    let out = cmd.output().expect("run gitless-sync");
    assert!(
        out.status.success(),
        "expected success, exit={:?}, stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    let json = parse_json(&out.stdout);
    let summary = &json["summary"];
    assert_eq!(summary["identical"].as_u64().unwrap(), 1);
    assert_eq!(summary["local_only_changed"].as_u64().unwrap(), 1);
    assert_eq!(summary["remote_only_changed"].as_u64().unwrap(), 1);
    assert_eq!(summary["drift"].as_u64().unwrap(), 1);
    assert_eq!(summary["failed"].as_u64().unwrap(), 0);

    let files = json["files"].as_array().unwrap();
    let by_path: std::collections::HashMap<&str, &str> = files
        .iter()
        .map(|f| (f["path"].as_str().unwrap(), f["status"].as_str().unwrap()))
        .collect();
    assert_eq!(by_path.get("identical.md"), Some(&"identical"));
    assert_eq!(by_path.get("local_only.md"), Some(&"local_only_changed"));
    assert_eq!(by_path.get("remote_only.md"), Some(&"remote_only_changed"));
    assert_eq!(by_path.get("drift.md"), Some(&"drift"));
}

#[test]
fn scenario_9_gitignore_and_ignore_arg_form_union() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".gitignore"), "*.bak\n").unwrap();
    fs::write(dir.path().join("keep.md"), "keep\n").unwrap();
    fs::write(dir.path().join("drop.bak"), "drop-bak\n").unwrap();
    fs::write(dir.path().join("also-drop.log"), "drop-log\n").unwrap();

    let mut server = Server::new();
    let _trees = server
        .mock("GET", "/repos/o/r/git/trees/main")
        .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
        .with_status(200)
        .with_body(r#"{"sha":"x","tree":[],"truncated":false}"#)
        .create();

    let mut cmd = base_cmd(dir.path(), Some(&server.url()));
    cmd.args(["--ignore", "*.log", "scan"]);
    let out = cmd.output().expect("run gitless-sync");
    assert!(
        out.status.success(),
        "expected success, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json = parse_json(&out.stdout);
    let files = json["files"].as_array().unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f["path"].as_str().unwrap()).collect();
    assert!(
        paths.contains(&"keep.md"),
        "expected keep.md, got: {paths:?}"
    );
    assert!(
        paths.contains(&".gitignore"),
        "expected .gitignore, got: {paths:?}"
    );
    assert!(
        !paths.contains(&"drop.bak"),
        "drop.bak should be filtered by .gitignore, got: {paths:?}"
    );
    assert!(
        !paths.contains(&"also-drop.log"),
        "also-drop.log should be filtered by --ignore, got: {paths:?}"
    );
}

#[test]
fn scenario_10_token_missing_yields_auth_failed() {
    let dir = TempDir::new().unwrap();

    let mut cmd = Command::cargo_bin("gitless-sync").expect("locate binary");
    cmd.env_remove("GITHUB_TOKEN")
        .env_remove("GITLESS_API_BASE")
        .args([
            "--repo",
            "o/r",
            "--local",
            dir.path().to_str().unwrap(),
            "scan",
        ]);
    cmd.assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("AUTH_FAILED"));
}

#[test]
fn scenario_11_rate_limit_yields_exit_3() {
    let dir = TempDir::new().unwrap();
    let mut server = Server::new();
    let _trees = server
        .mock("GET", "/repos/o/r/git/trees/main")
        .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
        .with_status(403)
        .with_header("x-ratelimit-remaining", "0")
        .with_header("x-ratelimit-reset", "1700000000")
        .with_body(r#"{"message":"rate limit"}"#)
        .create();

    let mut cmd = base_cmd(dir.path(), Some(&server.url()));
    cmd.arg("scan");
    cmd.assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("RATE_LIMIT_EXCEEDED"));
}

#[test]
fn scenario_12_truncated_yields_exit_5() {
    let dir = TempDir::new().unwrap();
    let mut server = Server::new();
    let _trees = server
        .mock("GET", "/repos/o/r/git/trees/main")
        .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
        .with_status(200)
        .with_body(r#"{"sha":"x","tree":[],"truncated":true}"#)
        .create();

    let mut cmd = base_cmd(dir.path(), Some(&server.url()));
    cmd.arg("scan");
    cmd.assert()
        .failure()
        .code(5)
        .stderr(predicate::str::contains("TREES_TRUNCATED"));
}

#[test]
fn scenario_13_summary_only_omits_files_field() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
    let local_a = blob_hash(b"alpha\n");

    let mut server = Server::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_a}","size":6}}],"truncated":false}}"#
    );
    let _trees = server
        .mock("GET", "/repos/o/r/git/trees/main")
        .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
        .with_status(200)
        .with_body(&trees_body)
        .create();

    let mut cmd = base_cmd(dir.path(), Some(&server.url()));
    cmd.args(["scan", "--summary-only"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"files\"").not())
        .stdout(predicate::str::contains("\"summary\""));
}

#[test]
fn scenario_14_status_drift_filters_files_but_keeps_summary() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("identical.md"), "alpha\n").unwrap();
    let identical_sha = blob_hash(b"alpha\n");

    let drift_path = dir.path().join("drift.md");
    fs::write(&drift_path, "local-version\n").unwrap();
    pin_mtime(&drift_path, COMMIT_DATE_SECS);

    let mut server = Server::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[
            {{"path":"identical.md","mode":"100644","type":"blob","sha":"{identical_sha}","size":6}},
            {{"path":"drift.md","mode":"100644","type":"blob","sha":"remote-different","size":14}}
        ],"truncated":false}}"#
    );
    let _trees = server
        .mock("GET", "/repos/o/r/git/trees/main")
        .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
        .with_status(200)
        .with_body(&trees_body)
        .create();
    let _commits = server
        .mock("GET", "/repos/o/r/commits")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("sha".into(), "main".into()),
            Matcher::UrlEncoded("path".into(), "drift.md".into()),
            Matcher::UrlEncoded("per_page".into(), "1".into()),
        ]))
        .with_status(200)
        .with_body(commits_body(COMMIT_DATE_ISO))
        .create();

    let mut cmd = base_cmd(dir.path(), Some(&server.url()));
    cmd.args(["scan", "--status", "drift"]);
    let out = cmd.output().expect("run gitless-sync");
    assert!(
        out.status.success(),
        "expected success, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json = parse_json(&out.stdout);
    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1, "expected only drift entries, got {files:?}");
    assert_eq!(files[0]["status"].as_str().unwrap(), "drift");
    assert_eq!(files[0]["path"].as_str().unwrap(), "drift.md");

    let summary = &json["summary"];
    assert_eq!(
        summary["identical"].as_u64().unwrap(),
        1,
        "summary should reflect total counts, not the filter"
    );
    assert_eq!(summary["drift"].as_u64().unwrap(), 1);
}

#[test]
fn scenario_15_partial_failure_yields_exit_4() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("locked.md");
    fs::write(&target, "secret\n").unwrap();

    let mut server = Server::new();
    let _trees = server
        .mock("GET", "/repos/o/r/git/trees/main")
        .match_query(Matcher::UrlEncoded("recursive".into(), "1".into()))
        .with_status(200)
        .with_body(r#"{"sha":"x","tree":[],"truncated":false}"#)
        .create();

    // Block reads on locked.md while the binary runs.
    #[cfg(windows)]
    let _lock = {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&target)
            .expect("open exclusive")
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&target).unwrap().permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&target, perms).unwrap();
    }

    let mut cmd = base_cmd(dir.path(), Some(&server.url()));
    cmd.arg("scan");
    let out = cmd.output().expect("run gitless-sync");

    // Restore unix perms so TempDir cleanup can delete the file.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&target).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&target, perms).unwrap();
    }

    assert_eq!(
        out.status.code(),
        Some(4),
        "expected exit 4, stderr={}, stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let json = parse_json(&out.stdout);
    assert!(
        json["summary"]["failed"].as_u64().unwrap() >= 1,
        "expected summary.failed >= 1, got: {json}"
    );
    let files = json["files"].as_array().unwrap();
    assert!(
        files
            .iter()
            .any(|f| f["status"] == "failed" && f["path"] == "locked.md"),
        "expected locked.md as failed entry, got: {files:?}"
    );
}
