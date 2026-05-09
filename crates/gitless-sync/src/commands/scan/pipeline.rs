//! Three-pass classification pipeline. [`assemble_entries`] is the entry
//! point: hash local files (pass 1), fetch commit dates for differing
//! paths only (pass 2 via [`super::commits`]), classify and emit
//! [`FileEntry`] rows (pass 3).

use std::collections::{BTreeSet, HashMap, HashSet};

use chrono::{DateTime, Utc};

use super::args::Backend;
use super::case_collision;
use super::commits;
use super::compare::{FailedReason, FileEntry, Status, classify};
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
/// `Backend::Rest` uses rayon 8c per-path calls (ADR 0003); `Backend::Graphql`
/// uses a single alias-batched request (ADR 0005). Hash failures and case
/// collisions are recorded as [`Status::Failed`] without aborting.
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

    let collisions = case_collision::detect(&all_paths, &local_map, &remote_map);
    let pending = build_pre_entries(&all_paths, &local_map, &remote_map, keep_bom, &collisions);
    let commit_paths = extract_commit_paths(&pending);
    let commit_map =
        commits::fetch_commit_map(&commit_paths, ctx.client, ctx.repo, ctx.branch, ctx.backend)?;
    Ok(finalize_entries(pending, &commit_map))
}

/// Pass 1: hash local files, no Commits API call. Hash failures map to
/// [`PreState::Failed`] with `failed_reason: None` (v1.0 `hash_io` default).
/// Paths in `collisions` short-circuit to [`FailedReason::CaseCollision`]
/// without invoking [`try_hash_local`].
fn build_pre_entries(
    all_paths: &BTreeSet<&str>,
    local_map: &HashMap<&str, &LocalFile>,
    remote_map: &HashMap<&str, &RemoteFile>,
    keep_bom: bool,
    collisions: &HashSet<String>,
) -> Vec<PreEntry> {
    let mut pending: Vec<PreEntry> = Vec::with_capacity(all_paths.len());
    for path in all_paths {
        let local = local_map.get(path).copied();
        let remote = remote_map.get(path).copied();
        let remote_sha = remote.map(|r| r.sha.clone());

        if collisions.contains(*path) {
            pending.push(PreEntry {
                path: (*path).to_string(),
                state: PreState::Failed {
                    remote_sha,
                    local_mtime: local.map(|lf| lf.mtime),
                    failed_reason: Some(FailedReason::CaseCollision),
                },
            });
            continue;
        }

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
                        failed_reason: None,
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
                failed_reason,
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
                    failed_reason,
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
                    failed_reason: None,
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
        failed_reason: Option<FailedReason>,
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
#[path = "pipeline_tests.rs"]
mod tests;
