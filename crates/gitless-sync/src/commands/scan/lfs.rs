//! LFS pointer placeholder companion. `pipeline::try_short_circuit_failed`
//! drives the LFS detect via `gitattr.classify_path(path)` directly (AA
//! consolidated the LFS predicate + Unsupported branch into a single match
//! to keep `pipeline.rs` under the 300-LOC gate).
//!
//! See `spec-domain-pitfalls.md` § LFS pointer and
//! `spec-hash-and-normalize.md` § 화이트리스트 § LFS pointer.

use super::compare::{FailedReason, LfsPointer};

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
    use super::*;

    #[test]
    fn placeholder_returns_some_for_lfs_pointer_reason() {
        let p = placeholder_pointer_for(Some(FailedReason::LfsPointer)).unwrap();
        assert_eq!(p.oid, "?");
        assert_eq!(p.size, 0);
    }

    #[test]
    fn placeholder_returns_none_for_other_reasons_and_none() {
        for r in [
            None,
            Some(FailedReason::CaseCollision),
            Some(FailedReason::Submodule),
            Some(FailedReason::Symlink),
            Some(FailedReason::LongPath),
            Some(FailedReason::Encoding),
            Some(FailedReason::NfdCollision),
            Some(FailedReason::GitattributesUnsupported),
        ] {
            assert!(placeholder_pointer_for(r).is_none(), "{r:?}");
        }
    }
}
