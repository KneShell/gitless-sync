//! Test sibling for `pipeline.rs`. Loaded via
//! `#[cfg(test)] #[path = "pipeline_tests.rs"] mod tests;` so the test
//! LOC stays out of the 300-LOC implementation-file gate.

use std::fs;

use tempfile::TempDir;

use super::*;
use crate::commands::scan::test_helpers::{COMMITS_BODY, mtime, stub_commits};
use crate::shared::gh::MockGhClient;
use crate::shared::hash::blob_hash;

#[test]
fn assemble_entries_marks_unreadable_local_as_failed() {
    let dir = TempDir::new().unwrap();
    let bogus = LocalFile {
        relative_path: "ghost.md".to_string(),
        absolute_path: dir.path().join("ghost-not-here.md"),
        mtime: mtime(1_700_000_000),
    };
    let remote = RemoteFile {
        path: "ghost.md".to_string(),
        sha: "remote-sha".to_string(),
    };

    let mut mock = MockGhClient::new();
    stub_commits(&mut mock, "o/r", "main", "ghost.md", COMMITS_BODY);

    let ctx = GitHubContext {
        client: &mock,
        repo: "o/r",
        branch: "main",
        backend: Backend::Rest,
    };
    let (entries, summary, failed) = assemble_entries(&[bogus], &[remote], &ctx, false).unwrap();

    assert_eq!(failed, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].status, Status::Failed);
    assert!(entries[0].local_sha.is_none());
    assert_eq!(entries[0].remote_sha.as_deref(), Some("remote-sha"));
    // hash_io fail keeps `failed_reason` omitted (v1.0 baseline).
    assert!(entries[0].failed_reason.is_none());
}

#[test]
fn assemble_entries_skips_commits_for_identical() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("ok.md"), "hi\n").unwrap();
    let sha = blob_hash(b"hi\n");

    let local = LocalFile {
        relative_path: "ok.md".to_string(),
        absolute_path: dir.path().join("ok.md"),
        mtime: mtime(1_700_000_000),
    };
    let remote = RemoteFile {
        path: "ok.md".to_string(),
        sha: sha.clone(),
    };

    // No commits stub; if assemble_entries hits the Commits API anyway, it
    // would surface as an Http error (MockGhClient default).
    let mock = MockGhClient::new();
    let ctx = GitHubContext {
        client: &mock,
        repo: "o/r",
        branch: "main",
        backend: Backend::Rest,
    };
    let (entries, summary, failed) = assemble_entries(&[local], &[remote], &ctx, false).unwrap();

    assert_eq!(failed, 0);
    assert_eq!(summary.identical, 1);
    assert_eq!(entries[0].status, Status::Identical);
}

#[test]
fn assemble_entries_promotes_case_collision_to_failed_with_reason() {
    // Local volume swallowed `Foo.txt`; only lowercase remains. Remote has
    // both case variants. Pipeline should promote the unmatched remote-side
    // path to Status::Failed + failed_reason: case_collision and skip the
    // Commits API for that path entirely (no stub needed).
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("foo.txt"), "x").unwrap();
    let local = LocalFile {
        relative_path: "foo.txt".to_string(),
        absolute_path: dir.path().join("foo.txt"),
        mtime: mtime(1_700_000_000),
    };
    let lower = RemoteFile {
        path: "foo.txt".to_string(),
        sha: blob_hash(b"x"),
    };
    let upper = RemoteFile {
        path: "Foo.txt".to_string(),
        sha: "remote-upper-sha".to_string(),
    };

    let mock = MockGhClient::new();
    let ctx = GitHubContext {
        client: &mock,
        repo: "o/r",
        branch: "main",
        backend: Backend::Rest,
    };
    let (entries, summary, failed) =
        assemble_entries(&[local], &[lower, upper], &ctx, false).unwrap();

    assert_eq!(failed, 1, "case_collision must increment failed_count");
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.identical, 1);

    let upper_entry = entries.iter().find(|e| e.path == "Foo.txt").unwrap();
    assert_eq!(upper_entry.status, Status::Failed);
    assert_eq!(upper_entry.failed_reason, Some(FailedReason::CaseCollision));
    assert_eq!(
        upper_entry.remote_sha.as_deref(),
        Some("remote-upper-sha"),
        "remote sha is preserved for case_collision entry"
    );

    let lower_entry = entries.iter().find(|e| e.path == "foo.txt").unwrap();
    assert_eq!(lower_entry.status, Status::Identical);
    assert!(lower_entry.failed_reason.is_none());
}
