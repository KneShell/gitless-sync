//! Local file hashing — IO leaf module.
//!
//! Reads bytes from disk, normalizes per `shared::normalize`, and runs
//! `shared::hash::blob_hash`. Pure side-effect: no GitHub, no classification.
//!
//! K2 added `gitattr` + `relative_path` so `prepare_for_hash` can route
//! through the `.gitattributes`-classified branch.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::shared::gitattributes::GitAttributes;
use crate::shared::hash::blob_hash;
use crate::shared::normalize::prepare_for_hash;

pub(super) fn try_hash_local(
    path: &Path,
    keep_bom: bool,
    gitattr: &Arc<GitAttributes>,
    relative_path: &str,
) -> Result<(String, bool), std::io::Error> {
    let raw = fs::read(path)?;
    let (prepared, is_binary) = prepare_for_hash(&raw, keep_bom, gitattr, relative_path);
    Ok((blob_hash(&prepared), is_binary))
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
        let (sha, is_bin) = try_hash_local(&p, false, &attrs, "hello.md").unwrap();
        assert!(!is_bin);
        assert_eq!(sha, blob_hash(b"hello\n"));
    }

    #[test]
    fn try_hash_local_normalizes_crlf() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("crlf.md");
        fs::write(&p, b"hello\r\n").unwrap();
        let attrs = empty_attrs();
        let (sha, _) = try_hash_local(&p, false, &attrs, "crlf.md").unwrap();
        assert_eq!(sha, blob_hash(b"hello\n"));
    }

    #[test]
    fn try_hash_local_marks_binary() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("bin");
        fs::write(&p, [0u8, 1, 2, 3]).unwrap();
        let attrs = empty_attrs();
        let (_, is_bin) = try_hash_local(&p, false, &attrs, "bin").unwrap();
        assert!(is_bin);
    }
}
