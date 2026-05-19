//! Shared helpers for `gitless-sync` integration tests.
//!
//! Cargo treats `tests/common/mod.rs` as a module (not a separate test crate),
//! so each domain test file (`scan_dogfooding.rs`, `scan_errors.rs`,
//! `init_redirect.rs`, `scan_backend_parity.rs`) pulls helpers in via
//! `mod common;`. Only a subset of helpers is used by each consumer — the
//! file-level `#![allow(dead_code)]` keeps clippy quiet.

#![allow(dead_code)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;

use gitless_sync::commands::scan::output::serialize;
use gitless_sync::commands::scan::{Backend, ScanArgs, build_report};
use gitless_sync::shared::error::GitlessError;
use gitless_sync::shared::gh::{GhClient, GhResponse};
use gitless_sync::shared::hash::blob_hash;

// ---- TestGhClient: argv → canned response (mirrors the in-crate MockGhClient)
//
// `graphql_response` provides a single wildcard stub for `gh api graphql ...`
// invocations. Production `build_query` is module-private and the query string
// changes per chunk, so an exact-argv match would require duplicating the
// query builder here. The wildcard pattern matches any `api graphql ...` argv
// and lets scenario tests inject one canonical response per scan.

pub struct TestGhClient {
    responses: HashMap<Vec<String>, GhResponse>,
    graphql_response: Option<GhResponse>,
}

impl TestGhClient {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            graphql_response: None,
        }
    }

    pub fn stub(&mut self, args: Vec<String>, response: GhResponse) {
        self.responses.insert(args, response);
    }

    pub fn stub_graphql(&mut self, response: GhResponse) {
        self.graphql_response = Some(response);
    }
}

impl Default for TestGhClient {
    fn default() -> Self {
        Self::new()
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

pub fn ok_resp(body: &[u8]) -> GhResponse {
    GhResponse {
        stdout: body.to_vec(),
        stderr: String::new(),
        exit_code: 0,
    }
}

pub fn err_resp(stderr: &str) -> GhResponse {
    GhResponse {
        stdout: Vec::new(),
        stderr: stderr.to_string(),
        exit_code: 1,
    }
}

pub fn tree_args(repo: &str, branch: &str) -> Vec<String> {
    vec![
        "api".to_string(),
        format!("repos/{repo}/git/trees/{branch}?recursive=1"),
    ]
}

pub fn commits_args(repo: &str, branch: &str, path: &str) -> Vec<String> {
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

/// Phase 8 task I — scan now fetches remote blobs for sha-differ Hashed
/// entries to compute `normalize_equal`. Tests that exercise drift paths
/// must register a blob stub for each remote sha referenced in the Trees
/// response. `content` is the raw bytes the remote blob returns; the
/// helper base64-encodes them per GitHub Blobs API contract.
pub fn stub_blob(mock: &mut TestGhClient, repo: &str, sha: &str, content: &[u8]) {
    let b64 = BASE64_STANDARD.encode(content);
    let body = format!(
        r#"{{"sha":"{sha}","content":"{b64}","encoding":"base64","size":{},"url":"u"}}"#,
        content.len()
    );
    let args = vec!["api".to_string(), format!("repos/{repo}/git/blobs/{sha}")];
    mock.stub(args, ok_resp(body.as_bytes()));
}

pub fn args_for(dir: &Path, repo: &str) -> ScanArgs {
    ScanArgs {
        repo: Some(repo.to_string()),
        branch: None,
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

pub fn lf_blob_hash(text_lf: &str) -> String {
    // Inputs are pre-normalized LF text (no BOM, no CRLF) — direct hash
    // matches `prepare_for_hash`'s output for the unspecified branch.
    blob_hash(text_lf.as_bytes())
}

pub fn read_mtime_rfc3339(path: &Path) -> String {
    let modified = fs::metadata(path).unwrap().modified().unwrap();
    let dt: DateTime<Utc> = modified.into();
    dt.to_rfc3339_opts(SecondsFormat::Nanos, true)
}

pub fn commits_body_with_date(date: &str) -> String {
    format!(
        r#"[{{"sha":"c1","commit":{{"author":{{"name":"a","email":"a@e","date":"{date}"}},"committer":{{"name":"c","email":"c@e","date":"{date}"}},"message":"m"}},"url":"u"}}]"#
    )
}

pub fn run_to_json(args: &ScanArgs, client: &TestGhClient) -> Value {
    let (report, _failed) = build_report(args, client).expect("build_report");
    let json = serialize(&report, false).expect("serialize");
    serde_json::from_str(&json).expect("parse JSON")
}

pub fn graphql_ok_body(entries: &[(&str, &str)]) -> String {
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

pub fn graphql_err_body(code: &str, message: &str) -> String {
    format!(
        r#"{{"data":null,"errors":[{{"message":"{message}","extensions":{{"code":"{code}"}}}}]}}"#
    )
}
