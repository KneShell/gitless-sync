#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Integration fixture for Phase 7.1 task G — Trees `truncated:true`
//! recovery routed through `fetch_tree_with_fallback`.
//!
//! Acceptance per `spec-github-api.md` § Trees truncation handling +
//! `docs/ralph/implementation-plan.md` Phase 7.1 task G:
//! - Recursive `gh api ...trees/{branch}?recursive=1` returns
//!   `truncated:true` → `commands::scan` enters the sub-tree fallback
//!   without surfacing `GitlessError::TreesTruncated`.
//! - The fallback resolves `refs/heads/{branch}` → commit sha → root
//!   tree sha (one extra round-trip per `spec-github-api.md` § sha
//!   일관성) and walks the tree one layer at a time.
//! - Blob entries discovered across multiple layers (root + nested +
//!   doubly nested) aggregate into a single `ScanReport` whose
//!   `files[]` carries forward-slash paths and whose summary counters
//!   match the descent.
//!
//! Wire-level cap-trip + the inner-truncated propagation are already
//! covered by `shared/github/trees/mod.rs::tests` and
//! `shared/github/trees/fallback/recursive/walk.rs::tests`. This file
//! pins the integration value the unit tests cannot reach: the depth-2
//! path-prefix join (`docs/api/spec.md`) produced by the fallback
//! actually surfaces in the published `ScanReport.files[].path` shape,
//! end-to-end through `build_report` + `output::serialize`.
//!
//! Identical SHAs short-circuit the Commits API via G-003 — no commits
//! stub is registered. Any stray commits call would surface as
//! `TestGhClient: no stub registered for args ...` and fail the test,
//! which is part of the contract guarded here.

mod common;

use std::collections::HashMap;
use std::fs;

use serde_json::Value;
use tempfile::TempDir;

use common::{TestGhClient, args_for, lf_blob_hash, ok_resp, run_to_json, tree_args};

/// `gh api repos/{repo}/git/refs/heads/{branch}` — first leg of the two
/// step root tree sha resolve invoked by `resolve_root_tree_sha` per
/// `spec-github-api.md` § sha 일관성.
fn ref_heads_args(repo: &str, branch: &str) -> Vec<String> {
    vec![
        "api".to_string(),
        format!("repos/{repo}/git/refs/heads/{branch}"),
    ]
}

/// `gh api repos/{repo}/git/commits/{sha}` — second leg of the resolve.
fn commit_object_args(repo: &str, sha: &str) -> Vec<String> {
    vec!["api".to_string(), format!("repos/{repo}/git/commits/{sha}")]
}

/// `gh api repos/{repo}/git/trees/{tree_sha}` — non-recursive sub-tree
/// fetch invoked by `fetch_subtree_recursive`. Distinct from
/// `common::tree_args` which carries the `?recursive=1` query the
/// initial `fetch_tree` call uses.
fn sub_tree_args(repo: &str, tree_sha: &str) -> Vec<String> {
    vec![
        "api".to_string(),
        format!("repos/{repo}/git/trees/{tree_sha}"),
    ]
}

fn files_by_path(json: &Value) -> HashMap<String, Value> {
    json["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e["path"].as_str().unwrap().to_string(), e.clone()))
        .collect()
}

/// One sha per blob the descent must surface — owned `String` so the
/// caller can hand `&str` references into format! arguments without
/// re-hashing per stub.
struct BlobShas {
    a: String,
    main: String,
    intro: String,
    spec: String,
}

/// Materialize a 4-blob layout under `dir` whose paths line up with the
/// fallback descent in [`stub_fallback_responses`]. Returns the
/// LF-blob shas the remote stubs must echo so G-003 short-circuits the
/// Commits API. NFC-only ASCII content keeps the hash deterministic on
/// every host.
fn prepare_local_tree(dir: &std::path::Path) -> BlobShas {
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::create_dir_all(dir.join("docs").join("api")).unwrap();
    fs::write(dir.join("a.md"), "alpha").unwrap();
    fs::write(dir.join("src").join("main.rs"), "beta").unwrap();
    fs::write(dir.join("docs").join("intro.md"), "gamma").unwrap();
    fs::write(dir.join("docs").join("api").join("spec.md"), "delta").unwrap();
    BlobShas {
        a: lf_blob_hash("alpha"),
        main: lf_blob_hash("beta"),
        intro: lf_blob_hash("gamma"),
        spec: lf_blob_hash("delta"),
    }
}

/// Wire the seven `gh api` responses the fallback must consume:
/// the truncated `recursive=1` trigger, the two `ref → commit` resolve
/// legs, and four sub-tree responses spanning three descent layers
/// (root + `src` + `docs` + `docs/api`).
fn stub_fallback_responses(mock: &mut TestGhClient, shas: &BlobShas) {
    mock.stub(
        tree_args("o/r", "main"),
        ok_resp(br#"{"sha":"x","tree":[],"truncated":true}"#),
    );
    mock.stub(
        ref_heads_args("o/r", "main"),
        ok_resp(br#"{"ref":"refs/heads/main","object":{"sha":"c0","type":"commit"}}"#),
    );
    mock.stub(
        commit_object_args("o/r", "c0"),
        ok_resp(br#"{"sha":"c0","tree":{"sha":"root_tree"},"message":"m"}"#),
    );
    let root_body = format!(
        r#"{{"sha":"root_tree","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{}","size":5}},{{"path":"src","mode":"040000","type":"tree","sha":"src_tree"}},{{"path":"docs","mode":"040000","type":"tree","sha":"docs_tree"}}],"truncated":false}}"#,
        shas.a
    );
    mock.stub(
        sub_tree_args("o/r", "root_tree"),
        ok_resp(root_body.as_bytes()),
    );
    let src_body = format!(
        r#"{{"sha":"src_tree","tree":[{{"path":"main.rs","mode":"100644","type":"blob","sha":"{}","size":4}}],"truncated":false}}"#,
        shas.main
    );
    mock.stub(
        sub_tree_args("o/r", "src_tree"),
        ok_resp(src_body.as_bytes()),
    );
    let docs_body = format!(
        r#"{{"sha":"docs_tree","tree":[{{"path":"intro.md","mode":"100644","type":"blob","sha":"{}","size":5}},{{"path":"api","mode":"040000","type":"tree","sha":"api_tree"}}],"truncated":false}}"#,
        shas.intro
    );
    mock.stub(
        sub_tree_args("o/r", "docs_tree"),
        ok_resp(docs_body.as_bytes()),
    );
    let api_body = format!(
        r#"{{"sha":"api_tree","tree":[{{"path":"spec.md","mode":"100644","type":"blob","sha":"{}","size":5}}],"truncated":false}}"#,
        shas.spec
    );
    mock.stub(
        sub_tree_args("o/r", "api_tree"),
        ok_resp(api_body.as_bytes()),
    );
}

#[test]
fn truncated_recursive_routes_through_fallback_and_aggregates_blobs_across_layers() {
    // Layout exercises three descent depths:
    //   root        → a.md (blob), src (tree), docs (tree)
    //   src         → main.rs (blob)
    //   docs        → intro.md (blob), api (tree)
    //   docs/api    → spec.md (blob)
    // The fallback performs 4 sub-tree calls + 2 resolve legs and
    // produces 4 published entries. The depth-2 `docs/api/spec.md`
    // path is the unique integration signal — wire-level unit tests
    // stop at depth 1.
    let dir = TempDir::new().unwrap();
    let shas = prepare_local_tree(dir.path());
    let mut mock = TestGhClient::new();
    stub_fallback_responses(&mut mock, &shas);

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);

    assert_eq!(json["schema_version"], "1.5");
    assert_eq!(json["summary"]["identical"], 4);
    assert_eq!(json["summary"]["local_only_changed"], 0);
    assert_eq!(json["summary"]["remote_only_changed"], 0);
    assert_eq!(json["summary"]["drift"], 0);
    assert_eq!(json["summary"]["failed"], 0);

    let files = files_by_path(&json);
    assert_eq!(files.len(), 4, "4 blobs aggregated across 3 descent layers");
    for (path, expected_sha) in [
        ("a.md", &shas.a),
        ("src/main.rs", &shas.main),
        ("docs/intro.md", &shas.intro),
        ("docs/api/spec.md", &shas.spec),
    ] {
        let entry = files
            .get(path)
            .unwrap_or_else(|| panic!("expected files[] to carry {path}"));
        assert_eq!(
            entry["status"], "identical",
            "{path} should classify Identical"
        );
        assert_eq!(entry["local_sha"], expected_sha.as_str());
        assert_eq!(entry["remote_sha"], expected_sha.as_str());
    }
    // Forward-slash path-prefix join is the integration-only signal —
    // wire-level unit tests stop at depth 1 so a backslash regression in
    // the fallback path-prefix would not surface there.
    assert!(
        files.contains_key("docs/api/spec.md"),
        "depth-2 fallback path must use forward-slash join (G-004)"
    );
}
