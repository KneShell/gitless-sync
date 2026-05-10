//! Local-arm extraction from `super::build_one_pre_entry` — runs the
//! `try_hash_local` outcome through the 3 [`PreState`] arms (size/encoding
//! failure, hashed text, hash IO error). Phase 7.2 task N split keeps the
//! parent `mod.rs` under the LOC gate while preserving full test coverage
//! of the local-side dispatch.

use std::sync::Arc;

use chrono::{DateTime, Utc};

use super::types::PreState;
use crate::commands::scan::hash_local::try_hash_local;
use crate::commands::scan::walker::LocalFile;
use crate::shared::gitattributes::GitAttributes;

/// Inputs for [`build_local_state`] — wraps the per-path hash arguments
/// the parent's `build_one_pre_entry` already has computed. Keeps the
/// function signature under the 5-arg `clippy::too_many_arguments` gate.
pub(super) struct LocalArmInputs<'a> {
    pub(super) local: &'a LocalFile,
    pub(super) keep_bom: bool,
    pub(super) gitattr: &'a Arc<GitAttributes>,
    pub(super) path: &'a str,
    pub(super) remote_sha: Option<String>,
    pub(super) local_mtime: Option<DateTime<Utc>>,
}

pub(super) fn build_local_state(args: LocalArmInputs<'_>) -> PreState {
    let LocalArmInputs {
        local,
        keep_bom,
        gitattr,
        path,
        remote_sha,
        local_mtime,
    } = args;
    match try_hash_local(&local.absolute_path, keep_bom, gitattr, path) {
        // size-gate arms (`FileTooLarge`/`MemoryExceeded`, task K) skip the
        // read so `is_binary` returns `false`; encoding (FF) preserves the
        // NUL probe (UTF-16 BOM → `true`).
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
            eprintln!("warning: failed to hash {path}: {err}");
            PreState::Failed {
                remote_sha,
                local_mtime,
                failed_reason: None,
                is_binary: false,
                size_bytes: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::commands::scan::compare::FailedReason;
    use crate::commands::scan::test_helpers::mtime;
    use crate::shared::hash::blob_hash;

    fn local_at(dir: &TempDir, name: &str) -> LocalFile {
        LocalFile {
            relative_path: name.to_string(),
            absolute_path: dir.path().join(name),
            mtime: mtime(1_700_000_000),
            is_symlink: false,
        }
    }

    fn empty_attrs() -> Arc<GitAttributes> {
        Arc::new(GitAttributes::default())
    }

    #[test]
    fn build_local_state_marks_unreadable_local_as_failed_without_reason() {
        // hash io error path — `try_hash_local` returns Err, the `eprintln!`
        // branch fires, and PreState::Failed lands with `failed_reason: None`
        // (v1.0 backward-compat: hash IO errors don't get an enum reason).
        let dir = TempDir::new().unwrap();
        let bogus = LocalFile {
            relative_path: "ghost.md".to_string(),
            absolute_path: dir.path().join("ghost-not-here.md"),
            mtime: mtime(1_700_000_000),
            is_symlink: false,
        };
        let attrs = empty_attrs();

        let state = build_local_state(LocalArmInputs {
            local: &bogus,
            keep_bom: false,
            gitattr: &attrs,
            path: "ghost.md",
            remote_sha: Some("remote-sha".to_string()),
            local_mtime: Some(mtime(1_700_000_000)),
        });
        match state {
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
    fn build_local_state_surfaces_encoding_failure_from_try_hash_local() {
        // UTF-16 BOM payload — `try_hash_local` returns
        // `Ok((sha, _, Some(FailedReason::Encoding)))` per AA plumbing, and
        // build_local_state must map that into PreState::Failed with the
        // reason preserved (not silently turned into Hashed). UTF-16 BOM
        // input has embedded NULs so is_binary measurement is `true` (no
        // information lost on the way to wire JSON).
        let dir = TempDir::new().unwrap();
        let mut bom = vec![0xFF, 0xFE];
        bom.extend_from_slice(b"\x68\x00\x69\x00"); // "hi" UTF-16 LE
        fs::write(dir.path().join("u16.txt"), &bom).unwrap();
        let local = local_at(&dir, "u16.txt");
        let attrs = empty_attrs();

        let state = build_local_state(LocalArmInputs {
            local: &local,
            keep_bom: false,
            gitattr: &attrs,
            path: "u16.txt",
            remote_sha: None,
            local_mtime: None,
        });
        match state {
            PreState::Failed {
                failed_reason,
                is_binary,
                ..
            } => {
                assert_eq!(failed_reason, Some(FailedReason::Encoding));
                assert!(is_binary, "encoding failure must preserve is_binary=true");
            }
            PreState::Hashed { .. } => panic!("expected PreState::Failed with Encoding reason"),
        }
    }

    #[test]
    fn build_local_state_returns_hashed_for_normal_text_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("hi.md"), "hi\n").unwrap();
        let local = local_at(&dir, "hi.md");
        let attrs = empty_attrs();

        let state = build_local_state(LocalArmInputs {
            local: &local,
            keep_bom: false,
            gitattr: &attrs,
            path: "hi.md",
            remote_sha: Some(blob_hash(b"hi\n")),
            local_mtime: Some(mtime(1_700_000_000)),
        });
        match state {
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
