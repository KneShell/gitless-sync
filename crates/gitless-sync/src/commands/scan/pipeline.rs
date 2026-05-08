//! Three-pass classification pipeline — domain logic.
//!
//! 1. `build_pre_entries`: hash local files, capture per-path state.
//! 2. `fetch_commit_map` (delegated to [`super::commits`]): fetch commit dates
//!    only for paths whose SHA differs on both sides.
//! 3. `finalize_entries`: classify and emit `FileEntry` rows.
//!
//! [`assemble_entries`] is the orchestrator-facing entry point.

use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Utc};

use super::args::Backend;
use super::commits;
use super::compare::{FileEntry, Status, classify};
use super::hash_local::try_hash_local;
use super::output::Summary;
use super::walker::LocalFile;
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::github::RemoteFile;

/// GitHub call context — bundles client, repo coordinates, and backend choice
/// for `assemble_entries` and `fetch_commit_map` callers.
pub(super) struct GitHubContext<'a, C: GhClient + Sync> {
    pub(super) client: &'a C,
    pub(super) repo: &'a str,
    pub(super) branch: &'a str,
    pub(super) backend: Backend,
}

/// Compare matched local/remote files and produce per-entry report rows.
///
/// Calls a Commits API lookup only for paths whose SHA differs on both sides.
/// Backend choice (`Backend::Rest` / `Backend::Graphql`) decides between rayon
/// 8c REST per-path calls (ADR 0003) and a single GraphQL alias-batched
/// request (ADR 0005). Hash failures are recorded as [`Status::Failed`]
/// without aborting.
pub(super) fn assemble_entries<C: GhClient + Sync>(
    local_files: &[LocalFile],
    remote_files: &[RemoteFile],
    ctx: &GitHubContext<'_, C>,
    keep_bom: bool,
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

    let pending = build_pre_entries(&all_paths, &local_map, &remote_map, keep_bom);
    let commit_paths = extract_commit_paths(&pending);
    let commit_map =
        commits::fetch_commit_map(&commit_paths, ctx.client, ctx.repo, ctx.branch, ctx.backend)?;
    Ok(finalize_entries(pending, &commit_map))
}

/// Pass 1: hash local files and capture per-path state without calling the
/// Commits API.
///
/// Hash failures are recorded as [`PreState::Failed`].
fn build_pre_entries(
    all_paths: &BTreeSet<&str>,
    local_map: &HashMap<&str, &LocalFile>,
    remote_map: &HashMap<&str, &RemoteFile>,
    keep_bom: bool,
) -> Vec<PreEntry> {
    let mut pending: Vec<PreEntry> = Vec::with_capacity(all_paths.len());
    for path in all_paths {
        let local = local_map.get(path).copied();
        let remote = remote_map.get(path).copied();
        let remote_sha = remote.map(|r| r.sha.clone());

        let state = match local {
            Some(lf) => match try_hash_local(&lf.absolute_path, keep_bom) {
                Ok((sha, is_binary)) => PreState::Hashed {
                    local_sha: Some(sha),
                    remote_sha,
                    local_mtime: Some(lf.mtime),
                    is_binary,
                },
                Err(err) => {
                    eprintln!("warning: failed to hash {path}: {err}");
                    PreState::Failed {
                        remote_sha,
                        local_mtime: Some(lf.mtime),
                    }
                }
            },
            None => PreState::Hashed {
                local_sha: None,
                remote_sha,
                local_mtime: None,
                is_binary: false,
            },
        };

        pending.push(PreEntry {
            path: (*path).to_string(),
            state,
        });
    }
    pending
}

/// Pass 2 input: keep only paths whose SHA differs on both sides.
fn extract_commit_paths(pending: &[PreEntry]) -> Vec<String> {
    pending
        .iter()
        .filter_map(|p| match &p.state {
            PreState::Hashed {
                local_sha: Some(l),
                remote_sha: Some(r),
                ..
            } if l != r => Some(p.path.clone()),
            _ => None,
        })
        .collect()
}

/// Pass 3: classify each pending entry and emit `FileEntry` rows in input
/// (`BTreeSet`) order.
fn finalize_entries(
    pending: Vec<PreEntry>,
    commit_map: &HashMap<String, DateTime<Utc>>,
) -> (Vec<FileEntry>, Summary, usize) {
    let mut entries: Vec<FileEntry> = Vec::with_capacity(pending.len());
    let mut summary = Summary::default();
    let mut failed_count = 0usize;

    for pre in pending {
        let entry = match pre.state {
            PreState::Failed {
                remote_sha,
                local_mtime,
            } => {
                summary.failed += 1;
                failed_count += 1;
                FileEntry {
                    path: pre.path,
                    status: Status::Failed,
                    local_sha: None,
                    remote_sha,
                    local_mtime,
                    remote_last_commit_at: None,
                    is_binary: false,
                }
            }
            PreState::Hashed {
                local_sha,
                remote_sha,
                local_mtime,
                is_binary,
            } => {
                let remote_last_commit_at = commit_map.get(pre.path.as_str()).copied();
                let status = classify(
                    local_sha.as_deref(),
                    remote_sha.as_deref(),
                    local_mtime,
                    remote_last_commit_at,
                );
                match status {
                    Status::Identical => summary.identical += 1,
                    Status::LocalOnlyChanged => summary.local_only_changed += 1,
                    Status::RemoteOnlyChanged => summary.remote_only_changed += 1,
                    Status::Drift => summary.drift += 1,
                    Status::Failed => summary.failed += 1,
                }
                FileEntry {
                    path: pre.path,
                    status,
                    local_sha,
                    remote_sha,
                    local_mtime,
                    remote_last_commit_at,
                    is_binary,
                }
            }
        };
        entries.push(entry);
    }

    (entries, summary, failed_count)
}

/// Hash result + remote SHA carried between pass 1 (hashing) and pass 3
/// (classification) of [`assemble_entries`].
enum PreState {
    Failed {
        remote_sha: Option<String>,
        local_mtime: Option<DateTime<Utc>>,
    },
    Hashed {
        local_sha: Option<String>,
        remote_sha: Option<String>,
        local_mtime: Option<DateTime<Utc>>,
        is_binary: bool,
    },
}

struct PreEntry {
    path: String,
    state: PreState,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::scan::test_helpers::{COMMITS_BODY, mtime, stub_commits};
    use crate::shared::gh::MockGhClient;
    use crate::shared::hash::blob_hash;
    use std::fs;
    use tempfile::TempDir;

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
        let (entries, summary, failed) =
            assemble_entries(&[bogus], &[remote], &ctx, false).unwrap();

        assert_eq!(failed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, Status::Failed);
        assert!(entries[0].local_sha.is_none());
        assert_eq!(entries[0].remote_sha.as_deref(), Some("remote-sha"));
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
        let (entries, summary, failed) =
            assemble_entries(&[local], &[remote], &ctx, false).unwrap();

        assert_eq!(failed, 0);
        assert_eq!(summary.identical, 1);
        assert_eq!(entries[0].status, Status::Identical);
    }
}
