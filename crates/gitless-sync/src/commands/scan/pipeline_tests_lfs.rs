//! Test sibling for `pipeline.rs` covering Phase 5 task G1 acceptance —
//! `.gitattributes filter=lfs` short-circuits paths into `Status::Failed` +
//! `failed_reason: "lfs_pointer"` + placeholder `{oid: "?", size: 0}` without
//! a blob fetch (`spec-domain-pitfalls.md` § LFS pointer). Loaded via
//! `#[cfg(test)] #[path = "pipeline_tests_lfs.rs"] mod tests_lfs;`.

use std::fs;

use tempfile::TempDir;

use super::*;
use crate::commands::scan::test_helpers::mtime;
use crate::shared::gh::MockGhClient;
use crate::shared::gitattributes::GitAttributes;
use crate::shared::hash::blob_hash;

/// Canonical git-lfs pointer text (`spec/v1`). Scan does not parse this —
/// only `.gitattributes filter=lfs` matters for detection — but we use real
/// pointer bytes so the fixture matches what an LFS-tracked file holds.
const LFS_POINTER_BYTES: &[u8] =
    b"version https://git-lfs.github.com/spec/v1\noid sha256:4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393\nsize 12345\n";

#[test]
fn assemble_entries_promotes_lfs_filter_path_to_failed_with_pointer_placeholder() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join(".gitattributes"),
        "*.psd filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    fs::write(dir.path().join("cover.psd"), LFS_POINTER_BYTES).unwrap();
    let local = LocalFile {
        relative_path: "cover.psd".to_string(),
        absolute_path: dir.path().join("cover.psd"),
        mtime: mtime(1_700_000_000),
        is_symlink: false,
    };
    let remote = RemoteFile {
        path: "cover.psd".to_string(),
        sha: "remote-pointer-blob-sha".to_string(),
        mode: "100644".to_string(),
    };

    // No commits stub — LFS short-circuit must not invoke Commits API.
    let mock = MockGhClient::new();
    let ctx = GitHubContext {
        client: &mock,
        repo: "o/r",
        branch: "main",
        backend: Backend::Rest,
    };
    let gitattr = GitAttributes::load(dir.path()).unwrap();
    let (entries, summary, failed) =
        assemble_entries(&[local], &[remote], &ctx, false, &gitattr).unwrap();

    assert_eq!(failed, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.path, "cover.psd");
    assert_eq!(entry.status, Status::Failed);
    assert_eq!(entry.failed_reason, Some(FailedReason::LfsPointer));
    assert_eq!(
        entry.mode, "100644",
        "lfs_pointer inherits remote tree mode"
    );
    let pointer = entry
        .lfs_pointer
        .as_ref()
        .expect("Status::Failed + lfs_pointer reason must populate lfs_pointer field");
    assert_eq!(
        pointer.oid, "?",
        "scan does not fetch blob; oid is placeholder"
    );
    assert_eq!(
        pointer.size, 0,
        "scan does not fetch blob; size is placeholder"
    );
    assert!(
        entry.local_sha.is_none(),
        "lfs short-circuits before hashing"
    );
}

#[test]
fn assemble_entries_omits_lfs_pointer_for_unrelated_failed_reasons() {
    // case_collision-promoted entry must NOT carry an `lfs_pointer` field —
    // the placeholder is reserved for `failed_reason: lfs_pointer` only.
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("foo.txt"), "x").unwrap();
    let local = LocalFile {
        relative_path: "foo.txt".to_string(),
        absolute_path: dir.path().join("foo.txt"),
        mtime: mtime(1_700_000_000),
        is_symlink: false,
    };
    let remote_lower = RemoteFile {
        path: "foo.txt".to_string(),
        sha: blob_hash(b"x"),
        mode: "100644".to_string(),
    };
    let remote_upper = RemoteFile {
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
    let (entries, _, _) = assemble_entries(
        &[local],
        &[remote_lower, remote_upper],
        &ctx,
        false,
        &gitattr,
    )
    .unwrap();

    let upper = entries.iter().find(|e| e.path == "Foo.txt").unwrap();
    assert_eq!(upper.failed_reason, Some(FailedReason::CaseCollision));
    assert!(
        upper.lfs_pointer.is_none(),
        "non-LFS Failed entries must not carry lfs_pointer"
    );
}

#[test]
fn assemble_entries_does_not_promote_path_without_filter_lfs_match() {
    // `.gitattributes` lists `text=auto` for *.txt — no `filter=lfs` for
    // anything — so the path must NOT be promoted to lfs_pointer. Sanity
    // check that an unrelated `.gitattributes` file doesn't accidentally
    // trigger LFS detection.
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".gitattributes"), "*.txt text=auto\n").unwrap();
    fs::write(dir.path().join("notes.txt"), "hi\n").unwrap();
    let local = LocalFile {
        relative_path: "notes.txt".to_string(),
        absolute_path: dir.path().join("notes.txt"),
        mtime: mtime(1_700_000_000),
        is_symlink: false,
    };
    let remote = RemoteFile {
        path: "notes.txt".to_string(),
        sha: crate::shared::hash::blob_hash(b"hi\n"),
        mode: "100644".to_string(),
    };

    let mock = MockGhClient::new();
    let ctx = GitHubContext {
        client: &mock,
        repo: "o/r",
        branch: "main",
        backend: Backend::Rest,
    };
    let gitattr = GitAttributes::load(dir.path()).unwrap();
    let (entries, summary, _) =
        assemble_entries(&[local], &[remote], &ctx, false, &gitattr).unwrap();

    assert_eq!(summary.identical, 1);
    assert_eq!(summary.failed, 0);
    let entry = &entries[0];
    assert!(entry.lfs_pointer.is_none());
    assert!(entry.failed_reason.is_none());
}
