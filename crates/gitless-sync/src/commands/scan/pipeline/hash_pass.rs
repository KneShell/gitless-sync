//! Pass 1 of `assemble_entries` — for each path produce a [`PreEntry`]:
//! either short-circuited [`PreState::Failed`] (no hash, no Commits API)
//! or fully hashed [`PreState::Hashed`] (sha + `is_binary`). No Commits API
//! call lives here; that comes in `super::orchestrator::assemble_entries`.

use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Utc};

use super::short_circuit::{ClassifyContext, try_short_circuit_failed};
use crate::commands::scan::compare::FailedReason;
use crate::commands::scan::hash_local::try_hash_local;
use crate::commands::scan::walker::LocalFile;
use crate::shared::github::RemoteFile;

/// Per-path intermediate state — either short-circuited (no hash, fail
/// reason already locked in) or hashed (caller will classify against
/// remote sha + commit timestamp in [`super::finalize`]).
///
/// `is_binary` semantics (spec-output-schema.md § null 정책 + EE):
/// reflects the local-bytes NUL heuristic when a local read occurred. For
/// `Failed`, that means only the `Encoding` arm — every other Failed reason
/// short-circuits before any local read, so `is_binary: false` is the only
/// honest value (no measurement). `Hashed` always has a measured value;
/// the `local: None` arm (remote-only path) carries `false` for the same
/// "no measurement" reason.
pub(super) enum PreState {
    Failed {
        remote_sha: Option<String>,
        local_mtime: Option<DateTime<Utc>>,
        failed_reason: Option<FailedReason>,
        is_binary: bool,
        size_bytes: Option<u64>,
    },
    Hashed {
        local_sha: Option<String>,
        remote_sha: Option<String>,
        local_mtime: Option<DateTime<Utc>>,
        is_binary: bool,
    },
}

pub(super) struct PreEntry {
    pub(super) path: String,
    pub(super) mode: String,
    pub(super) state: PreState,
}

/// Pass 1: hash local files. No Commits API.
pub(super) fn build_pre_entries(
    all_paths: &BTreeSet<&str>,
    local_map: &HashMap<&str, &LocalFile>,
    remote_map: &HashMap<&str, &RemoteFile>,
    keep_bom: bool,
    cctx: &ClassifyContext<'_>,
) -> Vec<PreEntry> {
    all_paths
        .iter()
        .map(|path| {
            let local = local_map.get(path).copied();
            let remote = remote_map.get(path).copied();
            build_one_pre_entry(path, local, remote, keep_bom, cctx)
        })
        .collect()
}

fn build_one_pre_entry(
    path: &str,
    local: Option<&LocalFile>,
    remote: Option<&RemoteFile>,
    keep_bom: bool,
    cctx: &ClassifyContext<'_>,
) -> PreEntry {
    let remote_sha = remote.map(|r| r.sha.clone());
    let local_mtime = local.map(|lf| lf.mtime);

    if let Some((mode, reason)) = try_short_circuit_failed(path, local, remote, cctx) {
        // Short-circuited Failed reasons (submodule/symlink/long_path/
        // case_collision/nfd_collision/lfs_pointer/gitattributes_unsupported)
        // bail before any local read → no NUL measurement → `false`.
        let state = PreState::Failed {
            remote_sha,
            local_mtime,
            failed_reason: Some(reason),
            is_binary: false,
            size_bytes: None,
        };
        return PreEntry {
            path: path.to_string(),
            mode,
            state,
        };
    }

    let mode = remote.map_or_else(|| "100644".to_string(), |r| r.mode.clone());
    let state = match local {
        Some(lf) => match try_hash_local(&lf.absolute_path, keep_bom, cctx.gitattr, path) {
            // Failure arm — `is_binary` holds the NUL probe when a body
            // read occurred (encoding/EE: UTF-16 BOM → `true`); size-gate
            // arms skip the body so the contract returns `false`.
            // `size_bytes` is `Some(n)` for `FileTooLarge`/`MemoryExceeded`
            // (Phase 7.2 task K), `None` otherwise.
            Ok((_, is_binary, Some(reason), size_bytes)) => PreState::Failed {
                remote_sha,
                local_mtime,
                failed_reason: Some(reason),
                is_binary,
                size_bytes,
            },
            Ok((sha, is_binary, None, _)) => PreState::Hashed {
                local_sha: Some(sha),
                remote_sha,
                local_mtime,
                is_binary,
            },
            Err(err) => {
                // hash IO error → no successful read → no measurement.
                eprintln!("warning: failed to hash {path}: {err}");
                PreState::Failed {
                    remote_sha,
                    local_mtime,
                    failed_reason: None,
                    is_binary: false,
                    size_bytes: None,
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;
    use std::sync::Arc;

    use tempfile::TempDir;

    use super::*;
    use crate::commands::scan::test_helpers::mtime;
    use crate::shared::gitattributes::GitAttributes;
    use crate::shared::hash::blob_hash;

    fn ctx<'a>(
        case: &'a HashSet<String>,
        nfd: &'a HashSet<String>,
        attrs: &'a Arc<GitAttributes>,
    ) -> ClassifyContext<'a> {
        ClassifyContext {
            case_collisions: case,
            nfd_collisions: nfd,
            gitattr: attrs,
        }
    }

    #[test]
    fn build_one_pre_entry_marks_unreadable_local_as_failed_without_reason() {
        // hash io error path — `try_hash_local` returns Err, the `eprintln!`
        // branch fires, and PreState::Failed lands with `failed_reason: None`
        // (v1.0 baseline behavior, not an enum-tracked reason).
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
        let case = HashSet::new();
        let nfd = HashSet::new();
        let attrs = Arc::new(GitAttributes::default());
        let cctx = ctx(&case, &nfd, &attrs);

        let pre = build_one_pre_entry("ghost.md", Some(&bogus), Some(&remote), false, &cctx);
        assert_eq!(pre.path, "ghost.md");
        assert_eq!(pre.mode, "100644");
        match pre.state {
            PreState::Failed {
                remote_sha,
                failed_reason,
                ..
            } => {
                assert_eq!(remote_sha.as_deref(), Some("remote-sha"));
                assert!(failed_reason.is_none());
            }
            PreState::Hashed { .. } => panic!("expected PreState::Failed, got Hashed"),
        }
    }

    #[test]
    fn build_one_pre_entry_carries_default_mode_for_local_only_path() {
        // No remote tree entry → default mode "100644" (v1.1 schema invariant
        // "every file row carries mode" — spec-output-schema.md § v1.1).
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("local.md"), "x").unwrap();
        let local = LocalFile {
            relative_path: "local.md".to_string(),
            absolute_path: dir.path().join("local.md"),
            mtime: mtime(1_700_000_000),
            is_symlink: false,
        };
        let case = HashSet::new();
        let nfd = HashSet::new();
        let attrs = Arc::new(GitAttributes::default());
        let cctx = ctx(&case, &nfd, &attrs);

        let pre = build_one_pre_entry("local.md", Some(&local), None, false, &cctx);
        assert_eq!(pre.mode, "100644");
        assert!(matches!(pre.state, PreState::Hashed { .. }));
    }

    #[test]
    fn build_one_pre_entry_surfaces_encoding_failure_from_try_hash_local() {
        // UTF-16 BOM payload — `try_hash_local` returns
        // `Ok((sha, _, Some(FailedReason::Encoding)))` per AA plumbing, and
        // build_one_pre_entry must map that into PreState::Failed with the
        // reason preserved (not silently turned into Hashed).
        let dir = TempDir::new().unwrap();
        let mut bom = vec![0xFF, 0xFE];
        bom.extend_from_slice(b"\x68\x00\x69\x00"); // "hi" UTF-16 LE
        fs::write(dir.path().join("u16.txt"), &bom).unwrap();
        let local = LocalFile {
            relative_path: "u16.txt".to_string(),
            absolute_path: dir.path().join("u16.txt"),
            mtime: mtime(1_700_000_000),
            is_symlink: false,
        };
        let case = HashSet::new();
        let nfd = HashSet::new();
        let attrs = Arc::new(GitAttributes::default());
        let cctx = ctx(&case, &nfd, &attrs);

        let pre = build_one_pre_entry("u16.txt", Some(&local), None, false, &cctx);
        match pre.state {
            PreState::Failed {
                failed_reason,
                is_binary,
                ..
            } => {
                assert_eq!(failed_reason, Some(FailedReason::Encoding));
                // EE: encoding-failure arm preserves the NUL heuristic from
                // `try_hash_local`. UTF-16 BOM input has embedded NULs so the
                // measurement is `true` (no information lost on the way to
                // wire JSON).
                assert!(is_binary, "encoding failure must preserve is_binary=true");
            }
            PreState::Hashed { .. } => panic!("expected PreState::Failed with Encoding reason"),
        }
    }

    #[test]
    fn build_one_pre_entry_returns_hashed_for_normal_text_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("hi.md"), "hi\n").unwrap();
        let local = LocalFile {
            relative_path: "hi.md".to_string(),
            absolute_path: dir.path().join("hi.md"),
            mtime: mtime(1_700_000_000),
            is_symlink: false,
        };
        let remote = RemoteFile {
            path: "hi.md".to_string(),
            sha: blob_hash(b"hi\n"),
            mode: "100644".to_string(),
        };
        let case = HashSet::new();
        let nfd = HashSet::new();
        let attrs = Arc::new(GitAttributes::default());
        let cctx = ctx(&case, &nfd, &attrs);

        let pre = build_one_pre_entry("hi.md", Some(&local), Some(&remote), false, &cctx);
        match pre.state {
            PreState::Hashed {
                local_sha,
                is_binary,
                ..
            } => {
                assert_eq!(local_sha.as_deref(), Some(blob_hash(b"hi\n").as_str()));
                assert!(!is_binary);
            }
            PreState::Failed { .. } => panic!("expected PreState::Hashed"),
        }
    }
}
