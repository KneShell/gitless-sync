//! Local file hashing — IO leaf module.
//!
//! Reads bytes from disk, normalizes per `shared::normalize`, and runs
//! `shared::hash::blob_hash`. Pure side-effect: no GitHub, no classification.
//!
//! K2 added `gitattr` + `relative_path` so `prepare_for_hash` can route
//! through the `.gitattributes`-classified branch. AA extends the return
//! tuple to surface non-UTF-8 / UTF-16 BOM as `FailedReason::Encoding`
//! using the same byte read — the hash input is always raw bytes regardless
//! (`spec-domain-pitfalls.md` § Encoding (b) policy).

use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::compare::FailedReason;
use crate::shared::decode::{TextDecodeResult, try_decode_text};
use crate::shared::gitattributes::GitAttributes;
use crate::shared::hash::blob_hash;
use crate::shared::normalize::prepare_for_hash;

/// Hash a local file and surface encoding failures using the same byte read.
///
/// The third tuple element is `Some(FailedReason::Encoding)` when the bytes
/// carry a UTF-16 BOM (out of scope for v0.2; see `spec-hash-and-normalize.md`
/// § BOM). `Unknown` is logically unreachable per `decode.rs` (Windows-1252
/// covers all bytes). The caller demotes `Hashed` → `Failed` on `Some(_)`.
/// SHA + `is_binary` are still computed because they remain valid for the
/// raw-bytes hash policy.
pub(super) fn try_hash_local(
    path: &Path,
    keep_bom: bool,
    gitattr: &Arc<GitAttributes>,
    relative_path: &str,
) -> Result<(String, bool, Option<FailedReason>), std::io::Error> {
    let raw = fs::read(path)?;
    let encoding_failure = match try_decode_text(&raw) {
        TextDecodeResult::Utf16Bom { .. } => Some(FailedReason::Encoding),
        TextDecodeResult::Utf8 | TextDecodeResult::Detected { .. } | TextDecodeResult::Unknown => {
            None
        }
    };
    let (prepared, is_binary) = prepare_for_hash(&raw, keep_bom, gitattr, relative_path);
    Ok((blob_hash(&prepared), is_binary, encoding_failure))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn empty_attrs() -> Arc<GitAttributes> {
        Arc::new(GitAttributes::default())
    }

    #[test]
    fn try_hash_local_returns_io_error_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nope.txt");
        let attrs = empty_attrs();
        let err = try_hash_local(&missing, false, &attrs, "nope.txt").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn try_hash_local_hashes_text_file() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("hello.md");
        fs::write(&p, "hello\n").unwrap();
        let attrs = empty_attrs();
        let (sha, is_bin, encoding) = try_hash_local(&p, false, &attrs, "hello.md").unwrap();
        assert!(!is_bin);
        assert_eq!(sha, blob_hash(b"hello\n"));
        assert!(encoding.is_none());
    }

    #[test]
    fn try_hash_local_normalizes_crlf() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("crlf.md");
        fs::write(&p, b"hello\r\n").unwrap();
        let attrs = empty_attrs();
        let (sha, _, encoding) = try_hash_local(&p, false, &attrs, "crlf.md").unwrap();
        assert_eq!(sha, blob_hash(b"hello\n"));
        assert!(encoding.is_none());
    }

    #[test]
    fn try_hash_local_marks_binary() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("bin");
        fs::write(&p, [0u8, 1, 2, 3]).unwrap();
        let attrs = empty_attrs();
        let (_, is_bin, encoding) = try_hash_local(&p, false, &attrs, "bin").unwrap();
        assert!(is_bin);
        // NUL-bearing input is binary, decode shortlist still succeeds via
        // Windows-1252 (covers all bytes) → no encoding failure surfaced.
        assert!(encoding.is_none());
    }

    #[test]
    fn try_hash_local_surfaces_utf16_bom_as_encoding_failure() {
        // UTF-16 LE BOM (FF FE) — `try_decode_text` returns `Utf16Bom`, hash
        // is still computed over raw bytes (b-policy). UTF-16 conversion
        // is out of v0.2 scope; encoding failure is the surface signal.
        // `Unknown` is effectively unreachable per `decode.rs` (Windows-1252
        // covers all bytes) — this case targets the only fireable branch.
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("utf16.txt");
        fs::write(&p, [0xFFu8, 0xFE, b'A', 0]).unwrap();
        let attrs = empty_attrs();
        let (sha, is_bin, encoding) = try_hash_local(&p, false, &attrs, "utf16.txt").unwrap();
        assert!(is_bin, "UTF-16 with embedded NUL must be marked binary");
        assert_eq!(sha, blob_hash(&[0xFFu8, 0xFE, b'A', 0]));
        assert_eq!(encoding, Some(FailedReason::Encoding));
    }
}
