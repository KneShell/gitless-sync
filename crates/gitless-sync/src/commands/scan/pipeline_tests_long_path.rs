//! Test sibling for `pipeline.rs` covering Phase 5 task R1 acceptance —
//! Windows long-path / DOS reserved-name short-circuit promotion to
//! `Status::Failed` + `failed_reason: long_path`. Loaded via
//! `#[cfg(test)] #[path = "pipeline_tests_long_path.rs"] mod
//! tests_long_path;` so the implementation gate stays under 300 LOC.

use tempfile::TempDir;

use super::*;
use crate::shared::gh::MockGhClient;

#[test]
fn assemble_entries_promotes_remote_reserved_name_to_failed_with_long_path_reason() {
    // Remote tree carries `docs/CON.md` — Windows reserved DOS device name in
    // the file stem. Pipeline (Phase 5 task R1) short-circuits the path to
    // Status::Failed + failed_reason: long_path, no Commits API call, and
    // preserves the remote tree mode. The local side has no entry because
    // Windows refuses the create.
    let dir = TempDir::new().unwrap();
    let _ = dir; // tempdir kept alive for symmetry with sibling tests
    let remote = RemoteFile {
        path: "docs/CON.md".to_string(),
        sha: "remote-con-sha".to_string(),
        mode: "100644".to_string(),
    };

    // No commits stub — long_path short-circuit must not invoke Commits API.
    let mock = MockGhClient::new();
    let ctx = GitHubContext {
        client: &mock,
        repo: "o/r",
        branch: "main",
        backend: Backend::Rest,
    };
    let (entries, summary, failed) = assemble_entries(&[], &[remote], &ctx, false).unwrap();

    assert_eq!(failed, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.path, "docs/CON.md");
    assert_eq!(entry.status, Status::Failed);
    assert_eq!(entry.failed_reason, Some(FailedReason::LongPath));
    assert_eq!(entry.mode, "100644", "long_path inherits remote tree mode");
    assert_eq!(entry.remote_sha.as_deref(), Some("remote-con-sha"));
    assert!(entry.local_sha.is_none());
}

#[test]
fn assemble_entries_promotes_oversized_remote_path_to_failed_with_long_path_reason() {
    // Remote tree carries a path 260 bytes long — past the legacy Win32
    // MAX_PATH limit. Pipeline short-circuits without hashing or Commits API.
    let path = "a".repeat(260);
    let remote = RemoteFile {
        path: path.clone(),
        sha: "remote-long-sha".to_string(),
        mode: "100644".to_string(),
    };

    let mock = MockGhClient::new();
    let ctx = GitHubContext {
        client: &mock,
        repo: "o/r",
        branch: "main",
        backend: Backend::Rest,
    };
    let (entries, summary, failed) = assemble_entries(&[], &[remote], &ctx, false).unwrap();

    assert_eq!(failed, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.path, path);
    assert_eq!(entry.status, Status::Failed);
    assert_eq!(entry.failed_reason, Some(FailedReason::LongPath));
    assert_eq!(entry.remote_sha.as_deref(), Some("remote-long-sha"));
    assert!(entry.local_sha.is_none());
}
