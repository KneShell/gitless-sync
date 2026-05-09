//! NFD/NFC collision detection for the comparison stage.
//!
//! `walker::relative_path` normalizes every path to NFC, so two filesystem
//! entries whose raw bytes differ only in Unicode normalization form (NFD vs
//! NFC) produce the same comparison key. The `local_map: HashMap` then drops
//! one of the duplicates — collision information is lost. Detection runs on
//! the pre-dedup [`LocalFile`] slice so both copies are visible.
//!
//! Trigger condition: macOS with `core.precomposeunicode = false`, or any
//! filesystem (NTFS, ext4, APFS) where two files differ only in NFD/NFC.
//! See `spec-domain-pitfalls.md` § Path 정규화 § NFD edge and
//! `spec-classification.md` § Path 정규화 § edge case.

use std::collections::{HashMap, HashSet};

use super::walker::LocalFile;

/// Identify NFC keys that appear two or more times in `local_files`.
///
/// Output paths are flagged with `Status::Failed` + `failed_reason:
/// "nfd_collision"`. Remote-only collisions are impossible (GitHub Trees
/// returns each path once), so this only inspects the local side.
pub(super) fn detect(local_files: &[LocalFile]) -> HashSet<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for f in local_files {
        *counts.entry(f.relative_path.as_str()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .filter(|&(_, c)| c >= 2)
        .map(|(k, _)| k.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::{TimeZone, Utc};

    use super::*;

    fn lf(name: &str) -> LocalFile {
        LocalFile {
            relative_path: name.to_string(),
            absolute_path: PathBuf::from(name),
            mtime: Utc.timestamp_opt(0, 0).unwrap(),
            is_symlink: false,
        }
    }

    #[test]
    fn empty_input_yields_no_collisions() {
        assert!(detect(&[]).is_empty());
    }

    #[test]
    fn single_entry_per_key_yields_no_collisions() {
        let files = [lf("a.txt"), lf("sub/b.md")];
        assert!(detect(&files).is_empty());
    }

    #[test]
    fn duplicate_nfc_key_is_flagged() {
        // Two LocalFile entries whose NFC-normalized relative_path collides
        // (the synthetic NFD/NFC pair `\u{1100}\u{1161}.txt` ≡ `\u{AC00}.txt`
        // both normalize to `가.txt` via walker::relative_path).
        let files = [lf("가.txt"), lf("가.txt")];
        let collisions = detect(&files);
        assert_eq!(collisions.len(), 1);
        assert!(collisions.contains("가.txt"));
    }

    #[test]
    fn three_copies_of_same_key_flag_once() {
        let files = [lf("dup.md"), lf("dup.md"), lf("dup.md")];
        let collisions = detect(&files);
        assert_eq!(collisions.len(), 1);
        assert!(collisions.contains("dup.md"));
    }

    #[test]
    fn distinct_keys_with_one_collision_pair_only_flag_dup_key() {
        let files = [lf("alpha.md"), lf("dup.md"), lf("dup.md"), lf("beta.md")];
        let collisions = detect(&files);
        assert_eq!(collisions.len(), 1);
        assert!(collisions.contains("dup.md"));
        assert!(!collisions.contains("alpha.md"));
        assert!(!collisions.contains("beta.md"));
    }
}
