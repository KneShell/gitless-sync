//! Path normalization for cross-platform comparison keys.
//!
//! macOS APFS/HFS+ may store filenames in NFD (decomposed) form while
//! GitHub Trees API returns paths exactly as stored in the repo. Without
//! normalization, the same logical path encoded differently would produce
//! distinct comparison keys and surface as false drift. We canonicalize to
//! NFC at both boundaries (walker output, Trees response) so the comparison
//! map keys align regardless of the encoding origin.
//!
//! See `spec-domain-pitfalls.md` § Path 정규화 and `spec-classification.md`.

use unicode_normalization::UnicodeNormalization;

/// Convert `path` to Unicode NFC form for use as a comparison key.
///
/// Pure ASCII inputs are returned unchanged because NFC is a no-op on them.
/// Decomposed sequences (e.g. `\u{1100}\u{1161}` for the Korean syllable
/// `가`) collapse to their composed form (`\u{AC00}`).
#[must_use]
pub(crate) fn to_nfc(path: &str) -> String {
    path.nfc().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_passes_through_unchanged() {
        assert_eq!(to_nfc("README.md"), "README.md");
        assert_eq!(to_nfc("src/main.rs"), "src/main.rs");
        assert_eq!(to_nfc(""), "");
    }

    #[test]
    fn nfd_korean_syllable_composes_to_nfc() {
        let nfd = "\u{1100}\u{1161}.txt";
        let nfc = to_nfc(nfd);
        assert_eq!(nfc, "\u{AC00}.txt");
        assert_eq!(nfc, "가.txt");
    }

    #[test]
    fn nfc_input_is_idempotent() {
        let nfc = "가.txt";
        assert_eq!(to_nfc(nfc), nfc);
        assert_eq!(to_nfc(&to_nfc(nfc)), nfc);
    }

    #[test]
    fn nfd_latin_e_acute_composes_to_nfc() {
        let nfd = "caf\u{0065}\u{0301}.md";
        let nfc = to_nfc(nfd);
        assert_eq!(nfc, "caf\u{00E9}.md");
        assert_eq!(nfc, "café.md");
    }

    #[test]
    fn nfd_in_directory_segment_composes() {
        let nfd = "docs/\u{1100}\u{1161}/file.md";
        assert_eq!(to_nfc(nfd), "docs/가/file.md");
    }

    #[test]
    fn nfc_and_nfd_inputs_collapse_to_same_key() {
        let nfc = "가.txt";
        let nfd = "\u{1100}\u{1161}.txt";
        assert_ne!(nfc, nfd);
        assert_eq!(to_nfc(nfc), to_nfc(nfd));
    }
}
