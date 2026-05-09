//! `.gitattributes filter=lfs` predicate. Pipeline (`pipeline.rs`) consumes
//! [`is_lfs`] to short-circuit LFS-tracked paths into
//! [`super::compare::FailedReason::LfsPointer`] without a blob fetch.
//!
//! See `spec-domain-pitfalls.md` § LFS pointer and
//! `spec-hash-and-normalize.md` § 화이트리스트 § LFS pointer.

use super::compare::{FailedReason, LfsPointer};
use crate::shared::gitattributes::{AttributeMatch, GitAttributes};

/// Returns `true` when `path` is matched by a `filter=lfs`
/// `.gitattributes` rule (canonical git-lfs marker, e.g.
/// `*.psd filter=lfs diff=lfs merge=lfs -text`). `path` is the comparison
/// key — relative, forward-slash, NFC-normalized.
#[must_use]
pub(crate) fn is_lfs(path: &str, gitattr: &GitAttributes) -> bool {
    matches!(gitattr.classify_path(path), AttributeMatch::LfsPointer)
}

/// Scan-side LFS pointer placeholder. `scan` does not fetch blobs, so oid
/// and size are unknown — emit `{oid: "?", size: 0}` per
/// `spec-output-schema.md` § v1.1. `diff` (later) parses real values.
#[must_use]
pub(crate) fn placeholder_pointer_for(reason: Option<FailedReason>) -> Option<LfsPointer> {
    matches!(reason, Some(FailedReason::LfsPointer)).then(|| LfsPointer {
        oid: "?".to_string(),
        size: 0,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn load(dir: &TempDir) -> GitAttributes {
        GitAttributes::load(dir.path()).expect("load .gitattributes")
    }

    #[test]
    fn returns_false_when_no_gitattributes_present() {
        let dir = TempDir::new().unwrap();
        let gitattr = load(&dir);
        assert!(!is_lfs("vendor/lib.zip", &gitattr));
    }

    #[test]
    fn matches_canonical_git_lfs_marker_line() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join(".gitattributes"),
            "*.psd filter=lfs diff=lfs merge=lfs -text\n",
        )
        .unwrap();
        let gitattr = load(&dir);
        assert!(is_lfs("art/cover.psd", &gitattr));
    }

    #[test]
    fn matches_filter_lfs_only_line() {
        // Spec: presence of `filter=lfs` is authoritative — even without
        // accompanying `diff=lfs merge=lfs -text`, the path is LFS-tracked.
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitattributes"), "*.bin filter=lfs\n").unwrap();
        let gitattr = load(&dir);
        assert!(is_lfs("data/payload.bin", &gitattr));
    }

    #[test]
    fn does_not_match_unrelated_attributes() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitattributes"), "*.txt text=auto\n").unwrap();
        let gitattr = load(&dir);
        assert!(!is_lfs("notes.txt", &gitattr));
    }

    #[test]
    fn does_not_match_unrelated_filter_value() {
        // `filter=cleanup` is not LFS — only `filter=lfs` is the canonical
        // marker. Whitelist mapping in `gitattributes::whitelist_match`
        // promotes this to `Unsupported`, so `is_lfs` is false.
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitattributes"), "*.foo filter=cleanup\n").unwrap();
        let gitattr = load(&dir);
        assert!(!is_lfs("artifact.foo", &gitattr));
    }

    #[test]
    fn matches_path_under_subdirectory_gitattributes() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("art")).unwrap();
        fs::write(
            dir.path().join("art").join(".gitattributes"),
            "*.psd filter=lfs\n",
        )
        .unwrap();
        let gitattr = load(&dir);
        assert!(is_lfs("art/cover.psd", &gitattr));
        assert!(!is_lfs("readme.md", &gitattr));
    }
}
