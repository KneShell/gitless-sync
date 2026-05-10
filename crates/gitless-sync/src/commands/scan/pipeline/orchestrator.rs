//! Three-pass orchestration of the scan slice.
//!
//! 1. [`build_pre_entries`] — hash local + short-circuit failures.
//! 2. [`extract_commit_paths`] + `commits::fetch_commit_map` — Commits API.
//! 3. [`finalize_entries`] — classify into 4 statuses + assemble `FileEntry`.
//!
//! Concurrency: REST=rayon (ADR 0003), GraphQL=alias batching (ADR 0005)
//! — both gated by `commits::fetch_commit_map` based on `Backend`.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use super::finalize::{extract_commit_paths, finalize_entries};
use super::hash_pass::build_pre_entries;
use super::short_circuit::ClassifyContext;
use crate::commands::scan::args::Backend;
use crate::commands::scan::case_collision;
use crate::commands::scan::commits;
use crate::commands::scan::compare::FileEntry;
use crate::commands::scan::nfd_collision;
use crate::commands::scan::output::Summary;
use crate::commands::scan::walker::LocalFile;
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::gitattributes::GitAttributes;
use crate::shared::github::RemoteFile;

/// GitHub call context — client + repo + backend. `pub(crate)` so the
/// `pipeline/mod.rs` `pub(crate) use` re-export reaches `commands/scan/mod.rs`
/// (parent of `pipeline`). The `mod orchestrator;` declaration in
/// `pipeline/mod.rs` is private, so this isn't accessible via the
/// `orchestrator::` path from outside the slice — only via the re-export.
pub(crate) struct GitHubContext<'a, C: GhClient + Sync> {
    pub(crate) client: &'a C,
    pub(crate) repo: &'a str,
    pub(crate) branch: &'a str,
    pub(crate) backend: Backend,
}

/// Compare files → entries. REST=rayon (ADR 0003), GraphQL=alias batching
/// (ADR 0005).
///
/// # Errors
/// Propagates Commits API errors via [`commits::fetch_commit_map`].
pub(crate) fn assemble_entries<C: GhClient + Sync>(
    local_files: &[LocalFile],
    remote_files: &[RemoteFile],
    ctx: &GitHubContext<'_, C>,
    keep_bom: bool,
    gitattr: &Arc<GitAttributes>,
) -> Result<(Vec<FileEntry>, Summary, usize), GitlessError> {
    let local_map: HashMap<&str, &LocalFile> = local_files
        .iter()
        .map(|f| (f.relative_path.as_str(), f))
        .collect();
    let remote_map: HashMap<&str, &RemoteFile> =
        remote_files.iter().map(|f| (f.path.as_str(), f)).collect();

    let mut all_paths: BTreeSet<&str> = BTreeSet::new();
    all_paths.extend(local_map.keys().copied());
    all_paths.extend(remote_map.keys().copied());

    let case_collisions = case_collision::detect(&all_paths, &local_map, &remote_map);
    let nfd_collisions = nfd_collision::detect(local_files);
    let cctx = ClassifyContext {
        case_collisions: &case_collisions,
        nfd_collisions: &nfd_collisions,
        gitattr,
    };
    let pending = build_pre_entries(&all_paths, &local_map, &remote_map, keep_bom, &cctx);
    let commit_paths = extract_commit_paths(&pending);
    let commit_map =
        commits::fetch_commit_map(&commit_paths, ctx.client, ctx.repo, ctx.branch, ctx.backend)?;
    Ok(finalize_entries(pending, &commit_map))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use tempfile::TempDir;

    use super::*;
    use crate::commands::scan::compare::{FailedReason, Status};
    use crate::commands::scan::test_helpers::mtime;
    use crate::shared::gh::MockGhClient;
    use crate::shared::hash::blob_hash;

    #[test]
    fn assemble_entries_skips_commits_for_identical_path_end_to_end() {
        // Integration check that the orchestrator stitches pass 1 → 2 → 3
        // correctly: identical SHA → extract_commit_paths returns empty →
        // Commits API never called (no stub registered). If the pass chain
        // regresses, MockGhClient surfaces the unexpected call as Http err.
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
            sha,
            mode: "100644".to_string(),
            size: None,
        };

        let mock = MockGhClient::new();
        let ctx = GitHubContext {
            client: &mock,
            repo: "o/r",
            branch: "main",
            backend: Backend::Rest,
        };
        let attrs = Arc::new(GitAttributes::default());
        let (entries, summary, failed) =
            assemble_entries(&[local], &[remote], &ctx, false, &attrs).unwrap();

        assert_eq!(failed, 0);
        assert_eq!(summary.identical, 1);
        assert_eq!(entries[0].status, Status::Identical);
    }

    #[test]
    fn assemble_entries_promotes_lfs_filter_match_with_pointer_placeholder_end_to_end() {
        // Integration check that short_circuit (LfsPointer cascade) flows
        // through finalize (lfs_pointer placeholder propagation). Pure unit
        // tests of either side don't catch a regression in the pass chain.
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join(".gitattributes"),
            "*.psd filter=lfs diff=lfs merge=lfs -text\n",
        )
        .unwrap();
        fs::write(dir.path().join("cover.psd"), b"pointer-bytes").unwrap();
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
            size: None,
        };

        let mock = MockGhClient::new();
        let ctx = GitHubContext {
            client: &mock,
            repo: "o/r",
            branch: "main",
            backend: Backend::Rest,
        };
        let attrs = Arc::new(GitAttributes::load(dir.path()).unwrap());
        let (entries, summary, failed) =
            assemble_entries(&[local], &[remote], &ctx, false, &attrs).unwrap();

        assert_eq!(failed, 1);
        assert_eq!(summary.failed, 1);
        let entry = &entries[0];
        assert_eq!(entry.status, Status::Failed);
        assert_eq!(entry.failed_reason, Some(FailedReason::LfsPointer));
        let pointer = entry
            .lfs_pointer
            .as_ref()
            .expect("lfs reason → placeholder");
        assert_eq!(pointer.oid, "?");
        assert_eq!(pointer.size, 0);
    }

    #[test]
    fn assemble_entries_marks_unreadable_local_as_failed_without_reason_end_to_end() {
        // hash_io error path — orchestrator must propagate
        // PreState::Failed from hash_pass (where eprintln! fires) into a
        // FileEntry with `failed_reason: None`. Validates the v1.0
        // backward-compat surface: hash IO errors don't get an enum reason.
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
            size: None,
        };

        // No commits stub — hash_io path reaches finalize as Failed, which
        // skips the Commits API entirely.
        let mock = MockGhClient::new();
        let ctx = GitHubContext {
            client: &mock,
            repo: "o/r",
            branch: "main",
            backend: Backend::Rest,
        };
        let attrs = Arc::new(GitAttributes::default());
        let (entries, summary, failed) =
            assemble_entries(&[bogus], &[remote], &ctx, false, &attrs).unwrap();

        assert_eq!(failed, 1);
        assert_eq!(summary.failed, 1);
        let entry = &entries[0];
        assert_eq!(entry.status, Status::Failed);
        assert!(entry.failed_reason.is_none());
        assert!(entry.lfs_pointer.is_none());
        assert_eq!(entry.remote_sha.as_deref(), Some("remote-sha"));
    }
}
