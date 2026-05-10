//! Pass 2 + Pass 3 of `assemble_entries`:
//!
//! - [`extract_commit_paths`] — filter pre-entries that need a Commits API
//!   call (only Hashed entries with `local_sha != remote_sha`). Identical /
//!   one-side-only paths skip the Commits API entirely (G-003 contract).
//! - [`finalize_entries`] — feed the commit timestamp map back into the
//!   pre-entries, run [`classify`], emit the final [`FileEntry`] vec.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::hash_pass::{PreEntry, PreState};
use crate::commands::scan::compare::{FileEntry, Status, classify};
use crate::commands::scan::lfs;
use crate::commands::scan::output::Summary;

/// Pass 2: paths that still need a Commits API call (Hashed + sha differ).
pub(super) fn extract_commit_paths(pending: &[PreEntry]) -> Vec<String> {
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

/// Pass 3: classify each `PreEntry` into a final `FileEntry` + accumulate
/// summary counters and `failed_count`.
pub(super) fn finalize_entries(
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
            is_binary,
            size_bytes,
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
                is_binary,
                mode,
                lfs_pointer: lfs::placeholder_pointer_for(failed_reason),
                failed_reason,
                size_bytes,
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
                lfs_pointer: None,
                size_bytes: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::scan::compare::FailedReason;
    use crate::commands::scan::test_helpers::mtime;

    fn hashed_entry(path: &str, mode: &str, local: &str, remote: &str) -> PreEntry {
        PreEntry {
            path: path.to_string(),
            mode: mode.to_string(),
            state: PreState::Hashed {
                local_sha: Some(local.to_string()),
                remote_sha: Some(remote.to_string()),
                local_mtime: Some(mtime(1_700_000_000)),
                is_binary: false,
            },
        }
    }

    fn failed_entry(path: &str, mode: &str, reason: Option<FailedReason>) -> PreEntry {
        PreEntry {
            path: path.to_string(),
            mode: mode.to_string(),
            state: PreState::Failed {
                remote_sha: Some("remote-sha".to_string()),
                local_mtime: Some(mtime(1_700_000_000)),
                failed_reason: reason,
                is_binary: false,
                size_bytes: None,
            },
        }
    }

    #[test]
    fn extract_commit_paths_returns_only_hashed_entries_with_differing_shas() {
        // Pass 2 contract — identical, local-only, remote-only, and Failed
        // entries all skip the Commits API call. Only the (Some, Some, neq)
        // case needs the timestamp.
        let pending = vec![
            hashed_entry("identical.md", "100644", "abc", "abc"),
            hashed_entry("changed.md", "100644", "local-sha", "remote-sha"),
            PreEntry {
                path: "local-only.md".to_string(),
                mode: "100644".to_string(),
                state: PreState::Hashed {
                    local_sha: Some("local-sha".to_string()),
                    remote_sha: None,
                    local_mtime: Some(mtime(1_700_000_000)),
                    is_binary: false,
                },
            },
            PreEntry {
                path: "remote-only.md".to_string(),
                mode: "100644".to_string(),
                state: PreState::Hashed {
                    local_sha: None,
                    remote_sha: Some("remote-sha".to_string()),
                    local_mtime: None,
                    is_binary: false,
                },
            },
            failed_entry("failed.md", "120000", Some(FailedReason::Symlink)),
        ];
        let paths = extract_commit_paths(&pending);
        assert_eq!(paths, vec!["changed.md".to_string()]);
    }

    #[test]
    fn pre_entry_to_file_propagates_failed_reason_and_lfs_pointer_placeholder() {
        // Failed-with-LfsPointer reason → lfs_pointer field gets the
        // placeholder `{oid: "?", size: 0}` (spec-output-schema.md § v1.1).
        let pre = failed_entry("cover.psd", "100644", Some(FailedReason::LfsPointer));
        let mut summary = Summary::default();
        let mut failed = 0usize;
        let entry = pre_entry_to_file(pre, &HashMap::new(), &mut summary, &mut failed);

        assert_eq!(entry.status, Status::Failed);
        assert_eq!(entry.failed_reason, Some(FailedReason::LfsPointer));
        let pointer = entry.lfs_pointer.expect("lfs_pointer reason → placeholder");
        assert_eq!(pointer.oid, "?");
        assert_eq!(pointer.size, 0);
        assert_eq!(failed, 1);
        assert_eq!(summary.failed, 1);
    }

    #[test]
    fn pre_entry_to_file_omits_lfs_pointer_for_non_lfs_failed_reasons() {
        // Wire-format invariants for short-circuit Failed: `lfs_pointer`
        // is reserved for `failed_reason: lfs_pointer`; `is_binary` is
        // the no-measurement default `false` (EE — short-circuit bails
        // before any local read).
        for reason in [
            FailedReason::CaseCollision,
            FailedReason::Submodule,
            FailedReason::Symlink,
            FailedReason::LongPath,
            FailedReason::Encoding,
            FailedReason::NfdCollision,
            FailedReason::GitattributesUnsupported,
        ] {
            let pre = failed_entry("x", "100644", Some(reason));
            let mut summary = Summary::default();
            let mut failed = 0usize;
            let entry = pre_entry_to_file(pre, &HashMap::new(), &mut summary, &mut failed);
            assert!(
                entry.lfs_pointer.is_none(),
                "{reason:?} carries lfs_pointer"
            );
            assert!(!entry.is_binary, "{reason:?} flips is_binary");
        }
    }

    #[test]
    fn pre_entry_to_file_preserves_is_binary_for_encoding_failure() {
        // EE: encoding-failure preserves `try_hash_local`'s NUL heuristic
        // (UTF-16 BOM has embedded NULs → `is_binary: true` survives).
        let pre = PreEntry {
            path: "u16.txt".to_string(),
            mode: "100644".to_string(),
            state: PreState::Failed {
                remote_sha: None,
                local_mtime: Some(mtime(1_700_000_000)),
                failed_reason: Some(FailedReason::Encoding),
                is_binary: true,
                size_bytes: None,
            },
        };
        let mut summary = Summary::default();
        let mut failed = 0usize;
        let entry = pre_entry_to_file(pre, &HashMap::new(), &mut summary, &mut failed);
        assert_eq!(entry.status, Status::Failed);
        assert_eq!(entry.failed_reason, Some(FailedReason::Encoding));
        assert!(entry.is_binary, "encoding failure must keep is_binary");
    }

    #[test]
    fn pre_entry_to_file_classifies_identical_when_shas_match_and_preserves_mode() {
        // Mode bit ("100755") + matching SHAs → Status::Identical, mode bit
        // flows through to the FileEntry — content equal executables stay
        // Identical (spec-output-schema.md § v1.1 acceptance).
        let pre = hashed_entry("build.sh", "100755", "sha-equal", "sha-equal");
        let mut summary = Summary::default();
        let mut failed = 0usize;
        let entry = pre_entry_to_file(pre, &HashMap::new(), &mut summary, &mut failed);

        assert_eq!(entry.status, Status::Identical);
        assert_eq!(entry.mode, "100755");
        assert!(entry.failed_reason.is_none());
        assert!(entry.lfs_pointer.is_none());
        assert_eq!(summary.identical, 1);
        assert_eq!(failed, 0);
    }

    #[test]
    fn finalize_entries_aggregates_summary_across_mixed_states() {
        // Multi-entry rollup — verifies summary counters sum correctly when
        // each PreEntry resolves through its own match arm.
        let pending = vec![
            hashed_entry("same.md", "100644", "abc", "abc"),
            failed_entry("bad.md", "120000", Some(FailedReason::Symlink)),
            PreEntry {
                path: "local-only.md".to_string(),
                mode: "100644".to_string(),
                state: PreState::Hashed {
                    local_sha: Some("l".to_string()),
                    remote_sha: None,
                    local_mtime: Some(mtime(1_700_000_000)),
                    is_binary: false,
                },
            },
        ];
        let (entries, summary, failed) = finalize_entries(pending, &HashMap::new());
        assert_eq!(entries.len(), 3);
        assert_eq!(summary.identical, 1);
        assert_eq!(summary.local_only_changed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(failed, 1);
    }
}
