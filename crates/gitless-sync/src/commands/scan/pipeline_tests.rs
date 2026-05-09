//! Test sibling for `pipeline.rs`. Loaded via
//! `#[cfg(test)] #[path = "pipeline_tests.rs"] mod tests;` so the test
//! LOC stays out of the 300-LOC implementation-file gate.

use std::fs;

use tempfile::TempDir;

use super::*;
use crate::commands::scan::test_helpers::{COMMITS_BODY, mtime, stub_commits};
use crate::shared::gh::MockGhClient;
use crate::shared::gitattributes::GitAttributes;
use crate::shared::hash::blob_hash;

#[test]
fn assemble_entries_marks_unreadable_local_as_failed() {
    let dir = TempDir::new().unwrap();
    let bogus = LocalFile {
        relative_path: "ghost.md".to_string(),
        absolute_path: dir.path().join("ghost-not-here.md"),
        mtime: mtime(1_700_000_000),
        is_symlink: false,
    };
    let remote = RemoteFile {
        path: "ghost.md".to_string(),
        sha: "remote-sha".to_string(),
        mode: "100644".to_string(),
    };

    let mut mock = MockGhClient::new();
    stub_commits(&mut mock, "o/r", "main", "ghost.md", COMMITS_BODY);

    let ctx = GitHubContext {
        client: &mock,
        repo: "o/r",
        branch: "main",
        backend: Backend::Rest,
    };
    let (entries, summary, failed) =
        assemble_entries(&[bogus], &[remote], &ctx, false, &GitAttributes::default()).unwrap();

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
        is_symlink: false,
    };
    let remote = RemoteFile {
        path: "ok.md".to_string(),
        sha: sha.clone(),
        mode: "100644".to_string(),
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
    let (entries, summary, failed) =
        assemble_entries(&[local], &[remote], &ctx, false, &GitAttributes::default()).unwrap();

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
        is_symlink: false,
    };
    let lower = RemoteFile {
        path: "foo.txt".to_string(),
        sha: blob_hash(b"x"),
        mode: "100644".to_string(),
    };
    let upper = RemoteFile {
        path: "Foo.txt".to_string(),
        sha: "remote-upper-sha".to_string(),
        mode: "100644".to_string(),
    };

    let mock = MockGhClient::new();
    let ctx = GitHubContext {
        client: &mock,
        repo: "o/r",
        branch: "main",
        backend: Backend::Rest,
    };
    let gitattr = GitAttributes::default();
    let (entries, summary, failed) =
        assemble_entries(&[local], &[lower, upper], &ctx, false, &gitattr).unwrap();

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

#[test]
fn assemble_entries_promotes_remote_submodule_to_failed_with_reason() {
    // Remote tree carries a submodule (`mode: "160000"`, type=commit). Phase
    // 5 task G short-circuits this BEFORE try_hash_local — even if a local
    // file shadows the same path, hashing it against a commit-pointer SHA is
    // meaningless. Result: Status::Failed + failed_reason: submodule, mode
    // bit "160000" carried into the v1.1 JSON, no Commits API call needed.
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("vendor-lib"), "shadow content").unwrap();
    let local = LocalFile {
        relative_path: "vendor/lib".to_string(),
        absolute_path: dir.path().join("vendor-lib"),
        mtime: mtime(1_700_000_000),
        is_symlink: false,
    };
    let remote = RemoteFile {
        path: "vendor/lib".to_string(),
        sha: "deadbeefcafe".to_string(),
        mode: "160000".to_string(),
    };

    // No commits stub — submodule short-circuit must not invoke Commits API.
    let mock = MockGhClient::new();
    let ctx = GitHubContext {
        client: &mock,
        repo: "o/r",
        branch: "main",
        backend: Backend::Rest,
    };
    let (entries, summary, failed) =
        assemble_entries(&[local], &[remote], &ctx, false, &GitAttributes::default()).unwrap();

    assert_eq!(failed, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.path, "vendor/lib");
    assert_eq!(entry.status, Status::Failed);
    assert_eq!(entry.failed_reason, Some(FailedReason::Submodule));
    assert_eq!(entry.mode, "160000");
    assert_eq!(entry.remote_sha.as_deref(), Some("deadbeefcafe"));
    assert!(entry.local_sha.is_none());
}

#[test]
fn assemble_entries_carries_mode_for_local_only_files() {
    // Local-only paths (no remote tree entry) inherit the default "100644"
    // mode bit, ensuring v1.1 schema invariant "every file row carries mode"
    // (spec-output-schema.md § v1.1 acceptance).
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("local.md"), "x").unwrap();
    let local = LocalFile {
        relative_path: "local.md".to_string(),
        absolute_path: dir.path().join("local.md"),
        mtime: mtime(1_700_000_000),
        is_symlink: false,
    };

    let mock = MockGhClient::new();
    let ctx = GitHubContext {
        client: &mock,
        repo: "o/r",
        branch: "main",
        backend: Backend::Rest,
    };
    let (entries, summary, _) =
        assemble_entries(&[local], &[], &ctx, false, &GitAttributes::default()).unwrap();

    assert_eq!(summary.local_only_changed, 1);
    assert_eq!(entries[0].mode, "100644");
}

#[test]
fn assemble_entries_promotes_remote_symlink_to_failed_with_reason() {
    // Remote tree carries a symlink (`mode: "120000"`, type=blob). Phase 5
    // task H short-circuits this BEFORE try_hash_local — the remote sha
    // points to a blob holding the link target path, not file content, so
    // a content compare is meaningless. Result: Status::Failed +
    // failed_reason: symlink, mode bit "120000" carried into v1.1 JSON,
    // no Commits API call needed.
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("link"), "shadow content").unwrap();
    let local = LocalFile {
        relative_path: "link".to_string(),
        absolute_path: dir.path().join("link"),
        mtime: mtime(1_700_000_000),
        is_symlink: false,
    };
    let remote = RemoteFile {
        path: "link".to_string(),
        sha: "feedface".to_string(),
        mode: "120000".to_string(),
    };

    // No commits stub — symlink short-circuit must not invoke Commits API.
    let mock = MockGhClient::new();
    let ctx = GitHubContext {
        client: &mock,
        repo: "o/r",
        branch: "main",
        backend: Backend::Rest,
    };
    let (entries, summary, failed) =
        assemble_entries(&[local], &[remote], &ctx, false, &GitAttributes::default()).unwrap();

    assert_eq!(failed, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.path, "link");
    assert_eq!(entry.status, Status::Failed);
    assert_eq!(entry.failed_reason, Some(FailedReason::Symlink));
    assert_eq!(entry.mode, "120000");
    assert_eq!(entry.remote_sha.as_deref(), Some("feedface"));
    assert!(entry.local_sha.is_none());
}

#[test]
fn assemble_entries_promotes_local_only_symlink_to_failed_with_mode_120000() {
    // Local symlink with no matching remote entry — the default mode bit
    // for local-only paths is "100644", so the symlink branch MUST
    // override to "120000" to satisfy the v1.1 schema invariant
    // (spec-output-schema.md § v1.1: mode reflects link type).
    let dir = TempDir::new().unwrap();
    let local = LocalFile {
        relative_path: "stale-link".to_string(),
        absolute_path: dir.path().join("stale-link"),
        mtime: mtime(1_700_000_000),
        is_symlink: true,
    };

    // No commits stub — symlink short-circuit must not invoke Commits API.
    let mock = MockGhClient::new();
    let ctx = GitHubContext {
        client: &mock,
        repo: "o/r",
        branch: "main",
        backend: Backend::Rest,
    };
    let (entries, summary, failed) =
        assemble_entries(&[local], &[], &ctx, false, &GitAttributes::default()).unwrap();

    assert_eq!(failed, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.path, "stale-link");
    assert_eq!(entry.status, Status::Failed);
    assert_eq!(entry.failed_reason, Some(FailedReason::Symlink));
    assert_eq!(
        entry.mode, "120000",
        "local-only symlink must override default 100644 to canonical 120000"
    );
    assert!(entry.remote_sha.is_none());
    assert!(entry.local_sha.is_none());
}
