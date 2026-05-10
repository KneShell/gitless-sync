//! Commits API lookup — IO module.
//!
//! Fetches `commit.committer.date` for the subset of paths whose local SHA
//! differs from the remote SHA. REST backend issues one rayon-parallel call
//! per path (G-011: max 8 threads, ADR 0003); GraphQL backend issues one
//! alias-batched request per [`graphql::GRAPHQL_BATCH_SIZE`] chunk (ADR 0005).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rayon::prelude::*;

use super::args::Backend;
use super::graphql;
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::github;

/// Max concurrent `fetch_last_commit_at` calls (G-011: GitHub abuse detection avoidance).
pub(super) const MAX_COMMITS_CONCURRENCY: usize = 8;

/// Fetch `commit.committer.date` for each path and return a `path → date` map.
///
/// REST backend issues per-path rayon-parallel calls; GraphQL backend issues
/// alias-batched requests. The caller (pipeline) is responsible for filtering
/// down to paths that actually need a commit lookup.
pub(super) fn fetch_commit_map<C: GhClient + Sync>(
    paths: &[String],
    client: &C,
    repo: &str,
    branch: &str,
    backend: Backend,
) -> Result<HashMap<String, DateTime<Utc>>, GitlessError> {
    match backend {
        Backend::Rest => {
            let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();
            let commit_dates = fetch_commit_dates_parallel(client, repo, branch, &path_refs)?;
            Ok(paths.iter().cloned().zip(commit_dates).collect())
        }
        Backend::Graphql => graphql::fetch_last_commit_at_batch(client, repo, branch, paths),
    }
}

/// Fetch `commit.committer.date` for each path in parallel (G-011: max 8 threads).
fn fetch_commit_dates_parallel<C: GhClient + Sync>(
    client: &C,
    repo: &str,
    branch: &str,
    paths: &[&str],
) -> Result<Vec<DateTime<Utc>>, GitlessError> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(MAX_COMMITS_CONCURRENCY)
        .build()
        .map_err(|e| GitlessError::Config(format!("rayon thread pool build failed: {e}")))?;
    pool.install(|| {
        paths
            .par_iter()
            .map(|p| github::fetch_last_commit_at(client, repo, branch, p))
            .collect::<Result<Vec<_>, _>>()
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::commands::scan::build_report;
    use crate::commands::scan::compare::Status;
    use crate::commands::scan::test_helpers::{
        COMMITS_BODY, args_for, stub_blob, stub_commits, stub_tree,
    };
    use crate::shared::gh::MockGhClient;
    use crate::shared::hash::blob_hash;

    #[test]
    fn fetch_commit_dates_parallel_short_circuits_on_empty_input() {
        // No stubs registered; if the function issued any call, MockGhClient
        // would error. Empty input must short-circuit before that happens.
        let mock = MockGhClient::new();
        let result = fetch_commit_dates_parallel(&mock, "o/r", "main", &[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn build_report_identical_skips_commits_api() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
        let local_sha = blob_hash(b"alpha\n");

        let mut mock = MockGhClient::new();
        let trees_body = format!(
            r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_sha}","size":6}}],"truncated":false}}"#
        );
        stub_tree(&mut mock, "o/r", "main", &trees_body);
        // Intentionally no commits stub: if `build_report` calls the Commits
        // API on an identical entry, MockGhClient falls back to Http err which
        // surfaces as a propagated error here.

        let args = args_for(dir.path(), Some("o/r"));
        let (report, failed) = build_report(&args, &mock).unwrap();

        assert_eq!(failed, 0);
        assert_eq!(report.summary.identical, 1);
        assert_eq!(report.summary.drift, 0);
        let entries = report.files.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, Status::Identical);
        assert_eq!(entries[0].local_sha.as_deref(), Some(local_sha.as_str()));
        assert_eq!(entries[0].remote_sha.as_deref(), Some(local_sha.as_str()));
        assert!(entries[0].remote_last_commit_at.is_none());
    }

    #[test]
    fn build_report_local_only_does_not_call_commits() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("only_here.md"), "x\n").unwrap();

        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            r#"{"sha":"x","tree":[],"truncated":false}"#,
        );

        let args = args_for(dir.path(), Some("o/r"));
        let (report, _) = build_report(&args, &mock).unwrap();
        assert_eq!(report.summary.local_only_changed, 1);
        let entries = report.files.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, Status::LocalOnlyChanged);
        assert!(entries[0].remote_sha.is_none());
        assert!(entries[0].remote_last_commit_at.is_none());
    }

    #[test]
    fn build_report_remote_only_does_not_call_commits() {
        let dir = TempDir::new().unwrap();
        let mut mock = MockGhClient::new();
        let trees_body = r#"{"sha":"x","tree":[{"path":"only_remote.md","mode":"100644","type":"blob","sha":"r1","size":1}],"truncated":false}"#;
        stub_tree(&mut mock, "o/r", "main", trees_body);

        let args = args_for(dir.path(), Some("o/r"));
        let (report, _) = build_report(&args, &mock).unwrap();
        assert_eq!(report.summary.remote_only_changed, 1);
        let entries = report.files.unwrap();
        assert_eq!(entries[0].status, Status::RemoteOnlyChanged);
        assert!(entries[0].local_sha.is_none());
        assert!(entries[0].remote_last_commit_at.is_none());
    }

    #[test]
    fn build_report_drift_calls_commits_api() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("d.md"), "local\n").unwrap();

        let mut mock = MockGhClient::new();
        let trees_body = r#"{"sha":"x","tree":[{"path":"d.md","mode":"100644","type":"blob","sha":"sha-remote","size":6}],"truncated":false}"#;
        stub_tree(&mut mock, "o/r", "main", trees_body);
        // Phase 8 task I: sha differ → normalize_pass fetches the remote blob.
        stub_blob(&mut mock, "o/r", "sha-remote", b"remote\n");
        stub_commits(&mut mock, "o/r", "main", "d.md", COMMITS_BODY);

        let args = args_for(dir.path(), Some("o/r"));
        let (report, _) = build_report(&args, &mock).unwrap();
        let entries = report.files.unwrap();
        assert_eq!(entries.len(), 1);
        assert!(matches!(
            entries[0].status,
            Status::Drift | Status::LocalOnlyChanged | Status::RemoteOnlyChanged
        ));
        assert!(entries[0].remote_last_commit_at.is_some());
    }

    #[test]
    fn build_report_drift_multiple_paths_invokes_commits_api_per_path() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
        fs::write(dir.path().join("b.md"), "beta\n").unwrap();
        fs::write(dir.path().join("c.md"), "gamma\n").unwrap();

        let mut mock = MockGhClient::new();
        let trees_body = r#"{"sha":"x","tree":[
            {"path":"a.md","mode":"100644","type":"blob","sha":"remote-a","size":6},
            {"path":"b.md","mode":"100644","type":"blob","sha":"remote-b","size":5},
            {"path":"c.md","mode":"100644","type":"blob","sha":"remote-c","size":6}
        ],"truncated":false}"#;
        stub_tree(&mut mock, "o/r", "main", trees_body);
        // Phase 8 task I: sha differ → normalize_pass fetches remote blobs.
        stub_blob(&mut mock, "o/r", "remote-a", b"remote-alpha\n");
        stub_blob(&mut mock, "o/r", "remote-b", b"remote-beta\n");
        stub_blob(&mut mock, "o/r", "remote-c", b"remote-gamma\n");
        stub_commits(&mut mock, "o/r", "main", "a.md", COMMITS_BODY);
        stub_commits(&mut mock, "o/r", "main", "b.md", COMMITS_BODY);
        stub_commits(&mut mock, "o/r", "main", "c.md", COMMITS_BODY);

        let args = args_for(dir.path(), Some("o/r"));
        let (report, failed) = build_report(&args, &mock).unwrap();
        assert_eq!(failed, 0);

        let entries = report.files.unwrap();
        assert_eq!(entries.len(), 3);
        let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["a.md", "b.md", "c.md"]);
        for e in &entries {
            assert!(
                e.remote_last_commit_at.is_some(),
                "drift entry {} should have commit timestamp",
                e.path
            );
        }
    }
}
