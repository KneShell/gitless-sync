//! Pre-hash failure-cascade dispatch — short-circuits paths into
//! `Status::Failed` before `try_hash_local`. Priority (highest wins):
//!
//! 1. `nfd_collision` (Phase 5.13 task AA).
//! 2. `case_collision` (Phase 5.2 task D).
//! 3. `long_path` (Phase 5.4 task R1).
//! 4. submodule (`mode == "160000"`, Phase 5.4 task G).
//! 5. symlink (`mode == "120000"` or `is_symlink`, task H).
//! 6. `.gitattributes` LFS (`AttributeMatch::LfsPointer`, task G1).
//! 7. `.gitattributes` Unsupported (task K1.5 + AA).
//! 8. `file_too_large` / `memory_exceeded` — post-read in
//!    `super::hash_local::try_size_gate` (Phase 7.2 task K).
//! 9. `Encoding` — post-read in `try_hash_local` (Phase 5.13.1 FF).
//!
//! Items 8–9 only run when this dispatch returns `None`, so a 1–7
//! match locks them out. Specs: `spec-domain-pitfalls.md` +
//! `spec-hash-and-normalize.md` § 우선순위.

use std::collections::HashSet;
use std::sync::Arc;

use crate::commands::scan::compare::FailedReason;
use crate::commands::scan::long_path;
use crate::commands::scan::walker::LocalFile;
use crate::shared::gitattributes::{AttributeMatch, GitAttributes};
use crate::shared::github::RemoteFile;

/// Inputs the short-circuit cascade and `build_one_pre_entry` need —
/// pre-computed collision sets + a shared `Arc<GitAttributes>` (K2 lifetime
/// contract: parsed once, shared by reference).
pub(super) struct ClassifyContext<'a> {
    pub(super) case_collisions: &'a HashSet<String>,
    pub(super) nfd_collisions: &'a HashSet<String>,
    pub(super) gitattr: &'a Arc<GitAttributes>,
}

/// Cascade dispatch — returns `Some((mode, reason))` if any branch fires,
/// `None` if the path proceeds to `try_hash_local`. See module doc for
/// priority ordering.
pub(super) fn try_short_circuit_failed(
    path: &str,
    local: Option<&LocalFile>,
    remote: Option<&RemoteFile>,
    cctx: &ClassifyContext<'_>,
) -> Option<(String, FailedReason)> {
    let mode = || remote.map_or_else(|| "100644".to_string(), |r| r.mode.clone());
    if cctx.nfd_collisions.contains(path) {
        Some((mode(), FailedReason::NfdCollision))
    } else if cctx.case_collisions.contains(path) {
        Some((mode(), FailedReason::CaseCollision))
    } else if long_path::is_invalid(path) {
        Some((mode(), FailedReason::LongPath))
    } else if remote.is_some_and(|r| r.mode == "160000") {
        Some(("160000".to_string(), FailedReason::Submodule))
    } else if remote.is_some_and(|r| r.mode == "120000") || local.is_some_and(|lf| lf.is_symlink) {
        Some(("120000".to_string(), FailedReason::Symlink))
    } else {
        match cctx.gitattr.classify_path(path) {
            AttributeMatch::LfsPointer => Some((mode(), FailedReason::LfsPointer)),
            AttributeMatch::Unsupported { .. } => {
                Some((mode(), FailedReason::GitattributesUnsupported))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;

    use super::*;
    use crate::commands::scan::test_helpers::mtime;

    fn empty_attrs() -> Arc<GitAttributes> {
        Arc::new(GitAttributes::default())
    }

    fn empty_set() -> HashSet<String> {
        HashSet::new()
    }

    fn local_file(path: &str, abs_root: &Path, is_symlink: bool) -> LocalFile {
        LocalFile {
            relative_path: path.to_string(),
            absolute_path: abs_root.join(path),
            mtime: mtime(1_700_000_000),
            is_symlink,
        }
    }

    fn remote_file(path: &str, mode: &str) -> RemoteFile {
        RemoteFile {
            path: path.to_string(),
            sha: "remote-sha".to_string(),
            mode: mode.to_string(),
        }
    }

    fn cctx_with<'a>(
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

    fn assert_promoted(result: Option<&(String, FailedReason)>, mode: &str, reason: FailedReason) {
        assert_eq!(result, Some(&(mode.to_string(), reason)));
    }

    #[test]
    fn nfd_collision_promotes_with_remote_mode() {
        let nfd: HashSet<String> = ["dup.txt".to_string()].into_iter().collect();
        let case = empty_set();
        let attrs = empty_attrs();
        let cctx = cctx_with(&case, &nfd, &attrs);
        let r = remote_file("dup.txt", "100644");
        let result = try_short_circuit_failed("dup.txt", None, Some(&r), &cctx);
        assert_promoted(result.as_ref(), "100644", FailedReason::NfdCollision);
    }

    #[test]
    fn case_collision_promotes_with_remote_mode() {
        let case: HashSet<String> = ["Foo.txt".to_string()].into_iter().collect();
        let nfd = empty_set();
        let attrs = empty_attrs();
        let cctx = cctx_with(&case, &nfd, &attrs);
        let r = remote_file("Foo.txt", "100644");
        let result = try_short_circuit_failed("Foo.txt", None, Some(&r), &cctx);
        assert_promoted(result.as_ref(), "100644", FailedReason::CaseCollision);
    }

    #[test]
    fn long_path_reserved_dos_name_promotes() {
        let case = empty_set();
        let nfd = empty_set();
        let attrs = empty_attrs();
        let cctx = cctx_with(&case, &nfd, &attrs);
        let r = remote_file("docs/CON.md", "100644");
        let result = try_short_circuit_failed("docs/CON.md", None, Some(&r), &cctx);
        assert_eq!(result, Some(("100644".to_string(), FailedReason::LongPath)));
    }

    #[test]
    fn long_path_oversized_260_bytes_promotes() {
        let case = empty_set();
        let nfd = empty_set();
        let attrs = empty_attrs();
        let cctx = cctx_with(&case, &nfd, &attrs);
        let path = "a".repeat(260);
        let r = remote_file(&path, "100644");
        let result = try_short_circuit_failed(&path, None, Some(&r), &cctx);
        assert_eq!(result, Some(("100644".to_string(), FailedReason::LongPath)));
    }

    #[test]
    fn submodule_mode_160000_promotes_with_canonical_mode() {
        let case = empty_set();
        let nfd = empty_set();
        let attrs = empty_attrs();
        let cctx = cctx_with(&case, &nfd, &attrs);
        let r = remote_file("vendor/lib", "160000");
        let result = try_short_circuit_failed("vendor/lib", None, Some(&r), &cctx);
        assert_promoted(result.as_ref(), "160000", FailedReason::Submodule);
    }

    #[test]
    fn symlink_mode_120000_remote_promotes_with_canonical_mode() {
        let case = empty_set();
        let nfd = empty_set();
        let attrs = empty_attrs();
        let cctx = cctx_with(&case, &nfd, &attrs);
        let r = remote_file("link", "120000");
        let result = try_short_circuit_failed("link", None, Some(&r), &cctx);
        assert_eq!(result, Some(("120000".to_string(), FailedReason::Symlink)));
    }

    #[test]
    fn local_only_symlink_overrides_default_mode_to_canonical_120000() {
        let case = empty_set();
        let nfd = empty_set();
        let attrs = empty_attrs();
        let cctx = cctx_with(&case, &nfd, &attrs);
        let dir = TempDir::new().unwrap();
        let l = local_file("stale-link", dir.path(), true);
        let result = try_short_circuit_failed("stale-link", Some(&l), None, &cctx);
        assert_eq!(result, Some(("120000".to_string(), FailedReason::Symlink)));
    }

    #[test]
    fn gitattributes_lfs_filter_promotes_to_lfs_pointer() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join(".gitattributes"),
            "*.psd filter=lfs diff=lfs merge=lfs -text\n",
        )
        .unwrap();
        let attrs = Arc::new(GitAttributes::load(dir.path()).unwrap());
        let case = empty_set();
        let nfd = empty_set();
        let cctx = cctx_with(&case, &nfd, &attrs);
        let result = try_short_circuit_failed("cover.psd", None, None, &cctx);
        assert_promoted(result.as_ref(), "100644", FailedReason::LfsPointer);
    }

    #[test]
    fn gitattributes_unsupported_attribute_promotes_to_failed_reason() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join(".gitattributes"),
            "*.txt working-tree-encoding=UTF-16\n",
        )
        .unwrap();
        let attrs = Arc::new(GitAttributes::load(dir.path()).unwrap());
        let case = empty_set();
        let nfd = empty_set();
        let cctx = cctx_with(&case, &nfd, &attrs);
        let result = try_short_circuit_failed("notes.txt", None, None, &cctx);
        let want = FailedReason::GitattributesUnsupported;
        assert_promoted(result.as_ref(), "100644", want);
    }

    #[test]
    fn no_short_circuit_returns_none_for_plain_path() {
        let case = empty_set();
        let nfd = empty_set();
        let attrs = empty_attrs();
        let cctx = cctx_with(&case, &nfd, &attrs);
        let r = remote_file("plain.txt", "100644");
        let result = try_short_circuit_failed("plain.txt", None, Some(&r), &cctx);
        assert_eq!(result, None);
    }

    #[test]
    fn case_collision_outranks_submodule_when_both_match() {
        // Cascade-priority lock: case_collision (priority 2) wins over
        // submodule (priority 4). Mode falls back to remote tree mode
        // (`mode()` closure path), not the canonical "160000" — proving the
        // dispatch returned BEFORE the submodule arm.
        let case: HashSet<String> = ["Foo.txt".to_string()].into_iter().collect();
        let nfd = empty_set();
        let attrs = empty_attrs();
        let cctx = cctx_with(&case, &nfd, &attrs);
        let r = remote_file("Foo.txt", "160000");
        let result = try_short_circuit_failed("Foo.txt", None, Some(&r), &cctx);
        assert_promoted(result.as_ref(), "160000", FailedReason::CaseCollision);
    }

    #[test]
    fn lfs_match_outranks_post_read_size_gate_and_encoding_priorities() {
        // Cascade-priority lock (Phase 7.2 task L): post-read items 8–9
        // (`file_too_large` / `memory_exceeded`, `super::hash_local::
        // try_size_gate`, task K) and `Encoding` (task FF) only fire
        // when this dispatch returns `None`. A `Some(LfsPointer)` blocks
        // `build_one_pre_entry` from ever reaching `try_hash_local`, so
        // neither size nor encoding can surface for the same path — the
        // cascade is byte/size-blind. See `spec-hash-and-normalize.md`
        // § 우선순위.
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join(".gitattributes"),
            "*.psd filter=lfs diff=lfs merge=lfs -text\n",
        )
        .unwrap();
        let attrs = Arc::new(GitAttributes::load(dir.path()).unwrap());
        let case = empty_set();
        let nfd = empty_set();
        let cctx = cctx_with(&case, &nfd, &attrs);
        let result = try_short_circuit_failed("cover.psd", None, None, &cctx);
        assert_promoted(result.as_ref(), "100644", FailedReason::LfsPointer);
    }

    #[test]
    fn gitattributes_text_auto_without_lfs_filter_returns_none() {
        // Regression guard (Phase 5.13.1 task GG): a `.gitattributes`
        // with `text=auto` only (no `filter=lfs`) must NOT promote — path
        // proceeds to `try_hash_local`. See `spec-domain-pitfalls.md`
        // § `.gitattributes` 화이트리스트.
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitattributes"), "*.txt text=auto\n").unwrap();
        let attrs = Arc::new(GitAttributes::load(dir.path()).unwrap());
        let case = empty_set();
        let nfd = empty_set();
        let cctx = cctx_with(&case, &nfd, &attrs);
        let result = try_short_circuit_failed("notes.txt", None, None, &cctx);
        assert_eq!(result, None);
    }
}
