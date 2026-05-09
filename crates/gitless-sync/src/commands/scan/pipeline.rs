//! Three-pass `assemble_entries`: hash local (pass 1), fetch commit dates
//! for differing paths via [`super::commits`] (pass 2), classify into
//! [`FileEntry`] rows (pass 3).

use std::collections::{BTreeSet, HashMap, HashSet};

use chrono::{DateTime, Utc};

use super::args::Backend;
use super::case_collision;
use super::commits;
use super::compare::{FailedReason, FileEntry, Status, classify};
use super::hash_local::try_hash_local;
use super::long_path;
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

/// Pass 1: hash local files, no Commits API call. Delegates per-path
/// composition to [`build_one_pre_entry`] so the loop body stays small and
/// the case-collision / submodule short-circuits remain isolated.
fn build_pre_entries(
    all_paths: &BTreeSet<&str>,
    local_map: &HashMap<&str, &LocalFile>,
    remote_map: &HashMap<&str, &RemoteFile>,
    keep_bom: bool,
    collisions: &HashSet<String>,
) -> Vec<PreEntry> {
    all_paths
        .iter()
        .map(|path| {
            let local = local_map.get(path).copied();
            let remote = remote_map.get(path).copied();
            build_one_pre_entry(path, local, remote, keep_bom, collisions)
        })
        .collect()
}

/// Compose one [`PreEntry`]. Defers the failed-short-circuit cascade to
/// [`try_short_circuit_failed`] so the cascade stays in one place and this
/// function fits the 60-line clippy gate. After short-circuit: hash ok →
/// `Hashed`; hash err → `Failed { failed_reason: None }` (v1.0 `hash_io`
/// default); local absent → remote-only `Hashed`.
fn build_one_pre_entry(
    path: &str,
    local: Option<&LocalFile>,
    remote: Option<&RemoteFile>,
    keep_bom: bool,
    collisions: &HashSet<String>,
) -> PreEntry {
    let remote_sha = remote.map(|r| r.sha.clone());
    let local_mtime = local.map(|lf| lf.mtime);

    if let Some((mode, reason)) = try_short_circuit_failed(path, local, remote, collisions) {
        return PreEntry {
            path: path.to_string(),
            mode,
            state: PreState::Failed {
                remote_sha,
                local_mtime,
                failed_reason: Some(reason),
            },
        };
    }

    let mode = remote.map_or_else(|| "100644".to_string(), |r| r.mode.clone());

    let state = match local {
        Some(lf) => match try_hash_local(&lf.absolute_path, keep_bom) {
            Ok((sha, is_binary)) => PreState::Hashed {
                local_sha: Some(sha),
                remote_sha,
                local_mtime,
                is_binary,
            },
            Err(err) => {
                eprintln!("warning: failed to hash {path}: {err}");
                PreState::Failed {
                    remote_sha,
                    local_mtime,
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

    PreEntry {
        path: path.to_string(),
        mode,
        state,
    }
}

/// Cascade of failed short-circuits. Priority: case collision → `long_path` /
/// reserved name → submodule (`160000`) → symlink (`120000` or
/// `local.is_symlink`). Submodule/symlink force their canonical mode bit so
/// local-only symlinks still report `"120000"` per `spec-output-schema.md`
/// § v1.1; `long_path` inherits the remote tree mode when available.
fn try_short_circuit_failed(
    path: &str,
    local: Option<&LocalFile>,
    remote: Option<&RemoteFile>,
    collisions: &HashSet<String>,
) -> Option<(String, FailedReason)> {
    let mode = || remote.map_or_else(|| "100644".to_string(), |r| r.mode.clone());
    if collisions.contains(path) {
        Some((mode(), FailedReason::CaseCollision))
    } else if long_path::is_invalid(path) {
        Some((mode(), FailedReason::LongPath))
    } else if remote.is_some_and(|r| r.mode == "160000") {
        Some(("160000".to_string(), FailedReason::Submodule))
    } else if remote.is_some_and(|r| r.mode == "120000") || local.is_some_and(|lf| lf.is_symlink) {
        Some(("120000".to_string(), FailedReason::Symlink))
    } else {
        None
    }
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
/// (`BTreeSet`) order. Per-entry composition lives in [`pre_entry_to_file`].
fn finalize_entries(
    pending: Vec<PreEntry>,
    commit_map: &HashMap<String, DateTime<Utc>>,
) -> (Vec<FileEntry>, Summary, usize) {
    let mut entries: Vec<FileEntry> = Vec::with_capacity(pending.len());
    let mut summary = Summary::default();
    let mut failed_count = 0usize;

    for pre in pending {
        let entry = pre_entry_to_file(pre, commit_map, &mut summary, &mut failed_count);
        entries.push(entry);
    }

    (entries, summary, failed_count)
}

/// Convert one [`PreEntry`] into a [`FileEntry`], updating the shared
/// `summary` / `failed_count` accumulators. Failed pre-entries skip the
/// Commits API lookup and carry through `failed_reason` (e.g. submodule,
/// case collision); hashed entries run [`classify`] against the matching
/// commit date.
fn pre_entry_to_file(
    pre: PreEntry,
    commit_map: &HashMap<String, DateTime<Utc>>,
    summary: &mut Summary,
    failed_count: &mut usize,
) -> FileEntry {
    let PreEntry { path, mode, state } = pre;
    match state {
        PreState::Failed {
            remote_sha,
            local_mtime,
            failed_reason,
        } => {
            summary.failed += 1;
            *failed_count += 1;
            FileEntry {
                path,
                status: Status::Failed,
                local_sha: None,
                remote_sha,
                local_mtime,
                remote_last_commit_at: None,
                is_binary: false,
                mode,
                failed_reason,
            }
        }
        PreState::Hashed {
            local_sha,
            remote_sha,
            local_mtime,
            is_binary,
        } => {
            let remote_last_commit_at = commit_map.get(path.as_str()).copied();
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
                path,
                status,
                local_sha,
                remote_sha,
                local_mtime,
                remote_last_commit_at,
                is_binary,
                mode,
                failed_reason: None,
            }
        }
    }
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
    /// Tree mode bit; defaults to `"100644"` for local-only paths and
    /// flows into `FileEntry::mode` (v1.1 schema).
    mode: String,
    state: PreState,
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "pipeline_tests_modes.rs"]
mod tests_modes;

#[cfg(test)]
#[path = "pipeline_tests_long_path.rs"]
mod tests_long_path;
