//! Test sibling for `pipeline.rs` covering mode-bit acceptance criteria
//! (Phase 5 task J onward). Loaded via
//! `#[cfg(test)] #[path = "pipeline_tests_modes.rs"] mod tests_modes;` so
//! each test file stays under the 300-LOC implementation gate while keeping
//! the mode-bit cases collected in one place.

use std::fs;

use tempfile::TempDir;

use super::*;
use crate::commands::scan::test_helpers::mtime;
use crate::shared::gh::MockGhClient;
use crate::shared::hash::blob_hash;

#[test]
fn assemble_entries_keeps_identical_when_only_mode_differs_executable() {
    // Phase 5 task J — spec-output-schema.md § v1.1 acceptance verbatim:
    // `mode == "100755"` + content 동일 → `Status::Identical`. Mode bit
    // alone is not drift; content-equal executables stay Identical and the
    // `mode: "100755"` flows through to v1.1 JSON for the caller. No
    // commits stub — identical SHAs skip the Commits API per the
    // `assemble_entries_skips_commits_for_identical` precedent.
    let dir = TempDir::new().unwrap();
    let content = b"#!/bin/sh\necho hi\n";
    fs::write(dir.path().join("build.sh"), content).unwrap();

    let local = LocalFile {
        relative_path: "build.sh".to_string(),
        absolute_path: dir.path().join("build.sh"),
        mtime: mtime(1_700_000_000),
        is_symlink: false,
    };
    let remote = RemoteFile {
        path: "build.sh".to_string(),
        sha: blob_hash(content),
        mode: "100755".to_string(),
    };

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
    assert_eq!(summary.drift, 0);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "build.sh");
    assert_eq!(entries[0].status, Status::Identical);
    assert_eq!(
        entries[0].mode, "100755",
        "executable mode bit must propagate to FileEntry.mode for v1.1 JSON"
    );
    assert!(entries[0].failed_reason.is_none());
}
