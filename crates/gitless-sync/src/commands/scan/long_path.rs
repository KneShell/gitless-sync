//! Detect paths Windows cannot represent without `\\?\` prefix and DOS
//! reserved device names. See `spec-domain-pitfalls.md` § Windows long path
//! and `spec-error-contracts.md` § `long_path`. Pipeline (`pipeline.rs`)
//! consumes [`is_invalid`] to short-circuit such paths into
//! [`super::compare::FailedReason::LongPath`].
//!
//! The check operates on the comparison key (relative, NFC-normalized,
//! forward-slash). That key is symmetric for local and remote, so a remote
//! path that Windows cannot land locally is flagged the same way as a
//! local-side overflow. Under-detection only happens when the working-dir
//! prefix alone pushes a representable relative path over the OS limit; the
//! resulting walker IO error is a separate signal we leave to the IO layer.

/// Windows `MAX_PATH` (with terminating NUL). Paths whose length is **>=**
/// this limit fail the legacy Win32 path API without a `\\?\` prefix.
/// Source: <https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation>.
const WINDOWS_MAX_PATH: usize = 260;

/// DOS device names reserved on Windows. The match is case-insensitive and
/// applies to the file-name **stem** (segment up to the first `.`), so
/// `CON.txt`, `foo/NUL.log`, `dir/com1.bin` are all flagged.
/// `COM10` / `LPT0` are intentionally absent — only `COM1`-`COM9` and
/// `LPT1`-`LPT9` are reserved.
/// Source: <https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#naming-conventions>.
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Returns `true` when `path` cannot be represented on Windows without a
/// `\\?\` long-path prefix or contains a DOS reserved device name in any
/// segment. `path` is the comparison key — relative, forward-slash,
/// NFC-normalized.
#[must_use]
pub fn is_invalid(path: &str) -> bool {
    path.len() >= WINDOWS_MAX_PATH || path.split('/').any(segment_is_reserved)
}

fn segment_is_reserved(segment: &str) -> bool {
    let stem = segment.split('.').next().unwrap_or(segment);
    if stem.is_empty() {
        return false;
    }
    RESERVED_NAMES
        .iter()
        .any(|name| stem.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_ascii_path_is_valid() {
        assert!(!is_invalid("README.md"));
        assert!(!is_invalid("docs/specs/spec-domain-pitfalls.md"));
    }

    #[test]
    fn path_at_259_bytes_is_valid() {
        let path = "a".repeat(259);
        assert_eq!(path.len(), 259);
        assert!(!is_invalid(&path));
    }

    #[test]
    fn path_at_260_bytes_is_long_path() {
        let path = "a".repeat(260);
        assert_eq!(path.len(), 260);
        assert!(is_invalid(&path));
    }

    #[test]
    fn path_well_over_max_path_is_long_path() {
        let path = "x".repeat(512);
        assert!(is_invalid(&path));
    }

    #[test]
    fn bare_reserved_name_con_is_flagged() {
        assert!(is_invalid("CON"));
        assert!(is_invalid("PRN"));
        assert!(is_invalid("AUX"));
        assert!(is_invalid("NUL"));
    }

    #[test]
    fn reserved_name_with_extension_is_flagged() {
        assert!(is_invalid("CON.txt"));
        assert!(is_invalid("NUL.log"));
        assert!(is_invalid("AUX.bin"));
    }

    #[test]
    fn reserved_name_in_subdirectory_is_flagged() {
        assert!(is_invalid("docs/CON.md"));
        assert!(is_invalid("a/b/c/PRN"));
    }

    #[test]
    fn reserved_name_match_is_case_insensitive() {
        assert!(is_invalid("con"));
        assert!(is_invalid("Con.txt"));
        assert!(is_invalid("CoN.LOG"));
        assert!(is_invalid("nul"));
    }

    #[test]
    fn com_and_lpt_numbered_devices_are_flagged() {
        for n in 1..=9 {
            assert!(is_invalid(&format!("COM{n}")));
            assert!(is_invalid(&format!("LPT{n}.txt")));
        }
    }

    #[test]
    fn com10_and_lpt0_are_not_reserved() {
        assert!(!is_invalid("COM10"));
        assert!(!is_invalid("LPT0"));
        assert!(!is_invalid("COM0"));
        assert!(!is_invalid("LPT10.txt"));
    }

    #[test]
    fn reserved_substring_in_longer_name_is_not_flagged() {
        assert!(!is_invalid("console.txt"));
        assert!(!is_invalid("CONFIG.md"));
        assert!(!is_invalid("docs/PRNT-tools.md"));
        assert!(!is_invalid("AUXiliary.rs"));
    }

    #[test]
    fn directory_segment_named_reserved_is_flagged() {
        // `CON` as a directory component is also unrepresentable on Windows.
        assert!(is_invalid("CON/notes.md"));
        assert!(is_invalid("foo/NUL/bar.txt"));
    }

    #[test]
    fn empty_segments_do_not_panic_and_are_not_flagged() {
        // Doubled slash produces an empty segment; the helper must skip it.
        assert!(!is_invalid("foo//bar.txt"));
        assert!(!is_invalid(""));
    }
}
