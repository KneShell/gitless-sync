//! IO leaf for the `diff` slice — local file read with `NotFound` treated as
//! `None` (one-sided diff case). Called from `compute.rs`.

use std::fs;
use std::path::Path;

use crate::shared::error::GitlessError;

pub(super) fn read_local_optional(path: &Path) -> Result<Option<Vec<u8>>, GitlessError> {
    match fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(GitlessError::Io(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn read_local_optional_returns_none_for_missing_file() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("nope.md");
        let result = read_local_optional(&p).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn read_local_optional_returns_bytes_for_existing_file() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("exists.md");
        fs::write(&p, b"abc").unwrap();
        let result = read_local_optional(&p).unwrap().unwrap();
        assert_eq!(result, b"abc");
    }
}
