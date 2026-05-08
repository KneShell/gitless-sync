#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! GraphQL backend (ADR 0006) end-to-end + cross-backend parity (PRD scenarios
//! 20, 21, 24).
//!
//! Scenarios 20-21 exercise the GraphQL backend end-to-end through
//! `build_report` / `run_with_client`. Scenario 24 asserts cross-backend
//! equivalence: REST and GraphQL must produce byte-identical `summary` and
//! `files[]` sets when fed equivalent stub data.
//!
//! Scenarios 22, 23, 25 (mtime cache matrix) were retired by ADR 0008
//! (2026-05-07) — cache itself was removed after P6c measured speedup ≈ 1.0x.

mod common;

use std::fs;

use tempfile::TempDir;

use gitless_sync::commands::scan::Backend;
use gitless_sync::commands::scan::build_report;
use gitless_sync::shared::error::GitlessError;

use common::{
    TestGhClient, args_for, commits_args, commits_body_with_date, graphql_err_body,
    graphql_ok_body, ok_resp, run_to_json, tree_args,
};

// ---- PRD 시나리오 20: GraphQL backend 정상 (drift trigger → committedDate 매핑) ----

#[test]
fn scenario_20_graphql_backend_returns_normal_scan_report() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha-local\n").unwrap();

    let repo = "p5c-test/scenario-20-graphql-ok";
    let branch = "main";

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

// ---- PRD 시나리오 24: REST / GraphQL 결과 정합성 (summary + files set 동일) -----

#[test]
fn scenario_24_cross_backend_produces_identical_report() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("a.md"), "alpha-local\n").unwrap();
    let repo = "p5c-test/scenario-24-cross-backend";
    let branch = "main";

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
