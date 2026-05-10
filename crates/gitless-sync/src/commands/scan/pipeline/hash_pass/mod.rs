//! Pass 1 of `assemble_entries` — for each path produce a [`PreEntry`]:
//! either short-circuited [`PreState::Failed`] (no hash, no Commits API)
//! or fully hashed [`PreState::Hashed`] (sha + `is_binary`). No Commits API
//! call lives here; that comes in `super::orchestrator::assemble_entries`.
//!
//! Phase 7.2 task N split — `types.rs` (data definitions) + `local.rs`
//! (`build_local_state` + local-arm tests). `mod.rs` and `local.rs` both
//! depend on `types.rs` only, keeping the cycle graph acyclic
//! (`spec-architecture.md` § Module 폴더 단위 정책).

mod local;
mod types;

use std::collections::{BTreeSet, HashMap};

pub(super) use self::types::{PreEntry, PreState};

use self::local::{LocalArmInputs, build_local_state};
use super::short_circuit::{ClassifyContext, try_short_circuit_failed};
use crate::commands::scan::compare::Presence;
use crate::commands::scan::hash_remote::try_remote_size_gate;
use crate::commands::scan::walker::LocalFile;
use crate::shared::github::RemoteFile;

fn presence_of(local: Option<&LocalFile>, remote: Option<&RemoteFile>) -> Presence {
    match (local.is_some(), remote.is_some()) {
        (true, true) => Presence::Both,
        (true, false) => Presence::LocalOnly,
        (false, true) => Presence::RemoteOnly,
        (false, false) => unreachable!("union path must have at least one side"),
    }
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
    let presence = presence_of(local, remote);

    if let Some((mode, reason)) = try_short_circuit_failed(path, local, remote, cctx) {
        // Short-circuited reasons bail before any local read → no NUL
        // measurement → `is_binary: false`.
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
            presence,
            state,
        };
    }

    let mode = remote.map_or_else(|| "100644".to_string(), |r| r.mode.clone());

    // Remote-side size pre-flight (Phase 7.2 task N): cheaper than the
    // local arm (no syscall), so it wins when both sides are oversize.
    if let Some((reason, n)) = try_remote_size_gate(remote) {
        let state = PreState::Failed {
            remote_sha,
            local_mtime,
            failed_reason: Some(reason),
            is_binary: false,
            size_bytes: Some(n),
        };
        return PreEntry {
            path: path.to_string(),
            mode,
            presence,
            state,
        };
    }

    let state = match local {
        Some(lf) => build_local_state(LocalArmInputs {
            local: lf,
            keep_bom,
            gitattr: cctx.gitattr,
            path,
            remote_sha,
            local_mtime,
        }),
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
        presence,
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
    use crate::commands::scan::compare::FailedReason;
    use crate::commands::scan::test_helpers::mtime;
    use crate::shared::gitattributes::GitAttributes;

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

    fn empty_cctx_inputs() -> (HashSet<String>, HashSet<String>, Arc<GitAttributes>) {
        (
            HashSet::new(),
            HashSet::new(),
            Arc::new(GitAttributes::default()),
        )
    }

    fn remote_with_size(path: &str, size: u64) -> RemoteFile {
        RemoteFile {
            path: path.to_string(),
            sha: "remote-sha".to_string(),
            mode: "100644".to_string(),
            size: Some(size),
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
        let (case, nfd, attrs) = empty_cctx_inputs();
        let cctx = ctx(&case, &nfd, &attrs);

        let pre = build_one_pre_entry("local.md", Some(&local), None, false, &cctx);
        assert_eq!(pre.mode, "100644");
        assert!(matches!(pre.state, PreState::Hashed { .. }));
    }

    #[test]
    fn build_one_pre_entry_promotes_memory_exceeded_from_remote_size_pre_flight() {
        // Phase 7.2 task N — Trees response size 50 MB+1 short-circuits to
        // `memory_exceeded` before any local read or blob fetch. Local
        // omitted on purpose: the remote pre-flight wins even when there
        // is no local arm to compete with. `scan` never calls `fetch_blob`
        // either, so the "0 invocations" contract holds trivially.
        let n = 50 * 1024 * 1024 + 1;
        let r = remote_with_size("big.bin", n);
        let (case, nfd, attrs) = empty_cctx_inputs();
        let cctx = ctx(&case, &nfd, &attrs);

        let pre = build_one_pre_entry("big.bin", None, Some(&r), false, &cctx);
        match pre.state {
            PreState::Failed {
                failed_reason,
                size_bytes,
                ..
            } => {
                assert_eq!(failed_reason, Some(FailedReason::MemoryExceeded));
                assert_eq!(size_bytes, Some(n));
            }
            PreState::Hashed { .. } => panic!("expected PreState::Failed"),
        }
    }

    #[test]
    fn build_one_pre_entry_promotes_file_too_large_from_remote_size_pre_flight() {
        let n = 100 * 1024 * 1024 + 1;
        let r = remote_with_size("huge.bin", n);
        let (case, nfd, attrs) = empty_cctx_inputs();
        let cctx = ctx(&case, &nfd, &attrs);

        let pre = build_one_pre_entry("huge.bin", None, Some(&r), false, &cctx);
        match pre.state {
            PreState::Failed {
                failed_reason,
                size_bytes,
                ..
            } => {
                assert_eq!(failed_reason, Some(FailedReason::FileTooLarge));
                assert_eq!(size_bytes, Some(n));
            }
            PreState::Hashed { .. } => panic!("expected PreState::Failed"),
        }
    }

    #[test]
    fn remote_size_gate_wins_over_local_arm_when_remote_is_oversize() {
        // Edge case (advisor flag): both sides "oversize" — remote pre-flight
        // is cheaper (no syscall) so it fires first per dispatch order. The
        // resulting `size_bytes` carries the remote-derived count, proving
        // the local arm never ran.
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("dual.bin"), b"small").unwrap();
        let local = LocalFile {
            relative_path: "dual.bin".to_string(),
            absolute_path: dir.path().join("dual.bin"),
            mtime: mtime(1_700_000_000),
            is_symlink: false,
        };
        let n = 60 * 1024 * 1024;
        let r = remote_with_size("dual.bin", n);
        let (case, nfd, attrs) = empty_cctx_inputs();
        let cctx = ctx(&case, &nfd, &attrs);

        let pre = build_one_pre_entry("dual.bin", Some(&local), Some(&r), false, &cctx);
        match pre.state {
            PreState::Failed {
                failed_reason,
                size_bytes,
                ..
            } => {
                assert_eq!(failed_reason, Some(FailedReason::MemoryExceeded));
                assert_eq!(
                    size_bytes,
                    Some(n),
                    "size_bytes carries remote-derived count, proving local arm did not run"
                );
            }
            PreState::Hashed { .. } => panic!("expected PreState::Failed"),
        }
    }
}
