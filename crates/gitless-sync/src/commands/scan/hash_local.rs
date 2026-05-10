//! Local file hashing — IO leaf module.
//!
//! Reads bytes from disk, normalizes per `shared::normalize`, and runs
//! `shared::hash::blob_hash`. Pure side-effect: no GitHub, no classification.
//!
//! K2 added `gitattr` + `relative_path` so `prepare_for_hash` can route
//! through the `.gitattributes`-classified branch. AA extends the return
//! tuple to surface non-UTF-8 / UTF-16 BOM as `FailedReason::Encoding`
//! using the same byte read — the hash input is always raw bytes regardless
//! (`spec-domain-pitfalls.md` § Encoding (b) policy). Phase 7.2 task K
//! prepends an `fs::metadata().len()` size pre-flight to short-circuit
//! oversize files before any read (`spec-hash-and-normalize.md` § Phase 7
//! 검출 알고리즘) — 100 MB → `FileTooLarge`, 50 MB → `MemoryExceeded`.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::compare::FailedReason;
use crate::shared::decode::{TextDecodeResult, try_decode_text};
use crate::shared::gitattributes::GitAttributes;
use crate::shared::hash::blob_hash;
use crate::shared::normalize::{is_binary, prepare_for_hash};

/// 100 MB — GitHub Blobs API hard limit (fact check 2026-05-10).
pub(super) const FILE_TOO_LARGE_BYTES: u64 = 100 * 1024 * 1024;
/// 50 MB — tool memory safety threshold (raw + base64 + SHA-1 buffer worst case).
pub(super) const MEMORY_EXCEEDED_BYTES: u64 = 50 * 1024 * 1024;

/// Pure size-gate check. `Some((reason, size))` when the byte count exceeds
/// either threshold (priority: `FileTooLarge` > `MemoryExceeded`); `None`
/// when it falls within the safe range. Boundaries are strict (`>`): a
/// 50 MB-exact file passes, 50 MB+1 fails.
pub(super) fn try_size_gate(size: u64) -> Option<(FailedReason, u64)> {
    if size > FILE_TOO_LARGE_BYTES {
        Some((FailedReason::FileTooLarge, size))
    } else if size > MEMORY_EXCEEDED_BYTES {
        Some((FailedReason::MemoryExceeded, size))
    } else {
        None
    }
}

/// Hash a local file and surface encoding / size failures using the same
/// metadata + byte read. The 3rd tuple slot is `Some(FailedReason)` for
/// UTF-16 BOM (`Encoding`) or oversize (`FileTooLarge` / `MemoryExceeded`);
/// the 4th slot carries the byte count for size failures, `None` otherwise.
/// Caller demotes `Hashed` → `Failed` on `Some(_)`.
///
/// Size pre-flight runs first via `fs::metadata` so a 100 MB+ file never
/// hits `fs::read` (tool memory + GitHub API budget protection — Phase 7.2
/// task K). On size failure: `is_binary: false` (no read, no NUL probe),
/// `sha: ""`, `size_bytes: Some(n)` — wire JSON omits SHA on Failed
/// entries via `pre_entry_to_file` (`spec-output-schema.md` § null 정책).
///
/// Encoding failure path keeps Phase 5.13 task AA semantics: NUL probe
/// from raw bytes (UTF-16 BOM has embedded NULs → `is_binary: true`),
/// empty SHA placeholder, `size_bytes: None`.
pub(super) fn try_hash_local(
    path: &Path,
    keep_bom: bool,
    gitattr: &Arc<GitAttributes>,
    relative_path: &str,
) -> Result<(String, bool, Option<FailedReason>, Option<u64>), std::io::Error> {
    let size = fs::metadata(path)?.len();
    if let Some((reason, n)) = try_size_gate(size) {
        return Ok((String::new(), false, Some(reason), Some(n)));
    }
    let raw = fs::read(path)?;
    if let TextDecodeResult::Utf16Bom { .. } = try_decode_text(&raw) {
        return Ok((
            String::new(),
            is_binary(&raw),
            Some(FailedReason::Encoding),
            None,
        ));
    }
    let (prepared, bin) = prepare_for_hash(&raw, keep_bom, gitattr, relative_path);
    Ok((blob_hash(&prepared), bin, None, None))
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
        let (sha, is_bin, encoding, size_bytes) =
            try_hash_local(&p, false, &attrs, "hello.md").unwrap();
        assert!(!is_bin);
        assert_eq!(sha, blob_hash(b"hello\n"));
        assert!(encoding.is_none());
        assert!(size_bytes.is_none(), "size_bytes is None on success path");
    }

    #[test]
    fn try_hash_local_normalizes_crlf() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("crlf.md");
        fs::write(&p, b"hello\r\n").unwrap();
        let attrs = empty_attrs();
        let (sha, _, encoding, size_bytes) = try_hash_local(&p, false, &attrs, "crlf.md").unwrap();
        assert_eq!(sha, blob_hash(b"hello\n"));
        assert!(encoding.is_none());
        assert!(size_bytes.is_none());
    }

    #[test]
    fn try_hash_local_marks_binary() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("bin");
        fs::write(&p, [0u8, 1, 2, 3]).unwrap();
        let attrs = empty_attrs();
        let (_, is_bin, encoding, size_bytes) = try_hash_local(&p, false, &attrs, "bin").unwrap();
        assert!(is_bin);
        // NUL-bearing input is binary, decode shortlist still succeeds via
        // Windows-1252 (covers all bytes) → no encoding failure surfaced.
        assert!(encoding.is_none());
        assert!(size_bytes.is_none());
    }

    #[test]
    fn try_hash_local_surfaces_utf16_bom_as_encoding_failure() {
        // UTF-16 LE BOM (FF FE) — `try_decode_text` returns `Utf16Bom`,
        // KK skips `prepare_for_hash` + `blob_hash` (caller discards SHA via
        // `_` in `pipeline/hash_pass.rs`). is_binary still measured from raw
        // bytes (NUL probe). `Unknown` is effectively unreachable per
        // `decode.rs` (Windows-1252 covers all bytes); UTF-16 BOM is the only
        // fireable branch. size_bytes stays `None` — encoding failure is
        // distinct from size failure (Phase 7.2 task K).
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("utf16.txt");
        fs::write(&p, [0xFFu8, 0xFE, b'A', 0]).unwrap();
        let attrs = empty_attrs();
        let (sha, is_bin, encoding, size_bytes) =
            try_hash_local(&p, false, &attrs, "utf16.txt").unwrap();
        assert!(is_bin, "UTF-16 with embedded NUL must be marked binary");
        assert_eq!(
            sha, "",
            "KK: encoding failure short-circuits hash; sha is empty placeholder"
        );
        assert_eq!(encoding, Some(FailedReason::Encoding));
        assert!(size_bytes.is_none(), "encoding failure has no size_bytes");
    }

    #[test]
    fn size_gate_passes_through_at_or_below_50mb_exact() {
        // Boundary is strict `>` — 50 MB exact passes (only over-50 fails).
        assert_eq!(try_size_gate(0), None);
        assert_eq!(try_size_gate(MEMORY_EXCEEDED_BYTES), None);
    }

    #[test]
    fn size_gate_promotes_memory_exceeded_just_over_50mb() {
        let n = MEMORY_EXCEEDED_BYTES + 1;
        assert_eq!(try_size_gate(n), Some((FailedReason::MemoryExceeded, n)));
    }

    #[test]
    fn size_gate_returns_memory_exceeded_at_100mb_exact() {
        // 100 MB exact is `> 50 MB` but `not > 100 MB` per strict `>` —
        // FileTooLarge fires only above 100 MB.
        assert_eq!(
            try_size_gate(FILE_TOO_LARGE_BYTES),
            Some((FailedReason::MemoryExceeded, FILE_TOO_LARGE_BYTES))
        );
    }

    #[test]
    fn size_gate_promotes_file_too_large_just_over_100mb() {
        let n = FILE_TOO_LARGE_BYTES + 1;
        assert_eq!(try_size_gate(n), Some((FailedReason::FileTooLarge, n)));
    }

    #[test]
    fn size_gate_prefers_file_too_large_over_memory_exceeded() {
        // 200 MB is over both thresholds — `FileTooLarge` wins per cascade.
        let n = 200 * 1024 * 1024;
        assert_eq!(try_size_gate(n), Some((FailedReason::FileTooLarge, n)));
    }

    #[test]
    fn try_hash_local_short_circuits_oversize_file_via_metadata_pre_flight() {
        // End-to-end pre-flight check — sparse-file `set_len` makes the
        // metadata report 101 MB without writing 101 MB to disk (NTFS / ext4
        // both honor `set_len` past EOF without zero-fill). Validates
        // `try_hash_local` calls `fs::metadata` BEFORE `fs::read` so the
        // 101 MB body never enters memory.
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("huge.bin");
        let f = fs::File::create(&p).unwrap();
        f.set_len(FILE_TOO_LARGE_BYTES + 1).unwrap();
        drop(f);
        let attrs = empty_attrs();
        let (sha, is_bin, reason, size_bytes) =
            try_hash_local(&p, false, &attrs, "huge.bin").unwrap();
        assert_eq!(sha, "", "size short-circuit returns empty SHA placeholder");
        assert!(
            !is_bin,
            "size short-circuit emits is_binary=false (no read)"
        );
        assert_eq!(reason, Some(FailedReason::FileTooLarge));
        assert_eq!(size_bytes, Some(FILE_TOO_LARGE_BYTES + 1));
    }
}
