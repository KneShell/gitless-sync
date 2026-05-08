//! Local file hashing — IO leaf module.
//!
//! Reads bytes from disk, normalizes per `shared::normalize`, and runs
//! `shared::hash::blob_hash`. Pure side-effect: no GitHub, no classification.

use std::fs;
use std::path::Path;

use crate::shared::hash::blob_hash;
use crate::shared::normalize::prepare_for_hash;

pub(super) fn try_hash_local(
    path: &Path,
    keep_bom: bool,
) -> Result<(String, bool), std::io::Error> {
    let raw = fs::read(path)?;
    let (prepared, is_binary) = prepare_for_hash(&raw, keep_bom);
    Ok((blob_hash(&prepared), is_binary))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn try_hash_local_returns_io_error_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nope.txt");
        let err = try_hash_local(&missing, false).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn try_hash_local_hashes_text_file() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("hello.md");
        fs::write(&p, "hello\n").unwrap();
        let (sha, is_bin) = try_hash_local(&p, false).unwrap();
        assert!(!is_bin);
        assert_eq!(sha, blob_hash(b"hello\n"));
    }

    #[test]
    fn try_hash_local_normalizes_crlf() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("crlf.md");
        fs::write(&p, b"hello\r\n").unwrap();
        let (sha, _) = try_hash_local(&p, false).unwrap();
        assert_eq!(sha, blob_hash(b"hello\n"));
    }

    #[test]
    fn try_hash_local_marks_binary() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("bin");
        fs::write(&p, [0u8, 1, 2, 3]).unwrap();
        let (_, is_bin) = try_hash_local(&p, false).unwrap();
        assert!(is_bin);
    }
}
