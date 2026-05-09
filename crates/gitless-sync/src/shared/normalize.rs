//! Text normalization + hash-input preparation.
//!
//! `prepare_for_hash` is the K2 entry point that routes raw bytes through
//! one of seven `.gitattributes`-driven branches into a `(prepared,
//! is_binary)` tuple consumed by `shared::hash::blob_hash`. The five named
//! helpers (`apply_text_auto` / `apply_binary` / `apply_eol_lf` /
//! `apply_eol_crlf` / `apply_unspecified`) keep each branch within the
//! Phase 6 cognitive-complexity budget. See
//! `docs/specs/spec-hash-and-normalize.md` § Normalize 규칙 + § Lifetime 계약.

use std::sync::Arc;

use crate::shared::gitattributes::{AttributeMatch, GitAttributes};

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

#[must_use]
pub fn is_binary(content: &[u8]) -> bool {
    let probe_len = content.len().min(8000);
    content[..probe_len].contains(&0)
}

#[must_use]
pub fn normalize_text(content: &[u8], keep_bom: bool) -> Vec<u8> {
    let body = strip_utf8_bom(content, keep_bom);
    let mut out = Vec::with_capacity(body.len());
    let mut i = 0;
    while i < body.len() {
        if body[i] == b'\r' && body.get(i + 1) == Some(&b'\n') {
            out.push(b'\n');
            i += 2;
        } else {
            out.push(body[i]);
            i += 1;
        }
    }
    out
}

fn strip_utf8_bom(content: &[u8], keep_bom: bool) -> &[u8] {
    if !keep_bom && content.starts_with(UTF8_BOM) {
        &content[UTF8_BOM.len()..]
    } else {
        content
    }
}

/// Route `raw` through the `.gitattributes`-classified hash-mode branch and
/// return `(prepared_bytes, is_binary)` ready for `shared::hash::blob_hash`.
/// Branch table + `Arc<GitAttributes>` lifetime contract:
/// `spec-hash-and-normalize.md` § Normalize 규칙 + § Lifetime 계약.
/// `LfsPointer` / `Unsupported` defensively fall through to v0.1 default —
/// pipeline short-circuits both into `Status::Failed` before this is called.
#[must_use]
pub(crate) fn prepare_for_hash(
    raw: &[u8],
    keep_bom: bool,
    gitattr: &Arc<GitAttributes>,
    path: &str,
) -> (Vec<u8>, bool) {
    match gitattr.classify_path(path) {
        AttributeMatch::TextAuto => apply_text_auto(raw, keep_bom),
        AttributeMatch::Binary => apply_binary(raw),
        AttributeMatch::EolLf => apply_eol_lf(raw, keep_bom),
        AttributeMatch::EolCrlf => apply_eol_crlf(raw, keep_bom),
        AttributeMatch::LfsPointer
        | AttributeMatch::Unsupported { .. }
        | AttributeMatch::Unspecified => apply_unspecified(raw, keep_bom),
    }
}

// 5 helpers below — split per K2 acceptance to keep prepare_for_hash within
// the Phase 6 cognitive-complexity budget. apply_text_auto and apply_eol_lf
// share a body intentionally (spec table distinguishes them).

fn apply_text_auto(raw: &[u8], keep_bom: bool) -> (Vec<u8>, bool) {
    (normalize_text(raw, keep_bom), false)
}

fn apply_binary(raw: &[u8]) -> (Vec<u8>, bool) {
    (raw.to_vec(), true)
}

fn apply_eol_lf(raw: &[u8], keep_bom: bool) -> (Vec<u8>, bool) {
    (normalize_text(raw, keep_bom), false)
}

/// `eol=crlf`: BOM strip only — line endings preserved (local LF vs remote
/// CRLF diverges, matching the spec acceptance scenario).
fn apply_eol_crlf(raw: &[u8], keep_bom: bool) -> (Vec<u8>, bool) {
    (strip_utf8_bom(raw, keep_bom).to_vec(), false)
}

/// v0.1 default policy. Reused defensively for `LfsPointer` / `Unsupported`.
fn apply_unspecified(raw: &[u8], keep_bom: bool) -> (Vec<u8>, bool) {
    if is_binary(raw) {
        (raw.to_vec(), true)
    } else {
        (normalize_text(raw, keep_bom), false)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::shared::hash::blob_hash;

    fn empty_attrs() -> Arc<GitAttributes> {
        Arc::new(GitAttributes::default())
    }

    fn attrs_with(body: &str) -> (TempDir, Arc<GitAttributes>) {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitattributes"), body).unwrap();
        let attrs = Arc::new(GitAttributes::load(dir.path()).unwrap());
        (dir, attrs)
    }

    // --- normalize_text + is_binary -------------------------------------

    #[test]
    fn strips_bom_when_keep_bom_false() {
        let input = [0xEF, 0xBB, 0xBF, b'a'];
        assert_eq!(normalize_text(&input, false), b"a");
    }

    #[test]
    fn keeps_bom_when_keep_bom_true() {
        let input = [0xEF, 0xBB, 0xBF, b'a'];
        assert_eq!(normalize_text(&input, true), input);
    }

    #[test]
    fn crlf_normalized_to_lf() {
        assert_eq!(
            normalize_text(b"hello\r\nworld\r\n", false),
            b"hello\nworld\n"
        );
    }

    #[test]
    fn lone_cr_is_preserved() {
        assert_eq!(normalize_text(b"a\rb", false), b"a\rb");
    }

    #[test]
    fn detects_binary_with_nul_byte() {
        assert!(is_binary(&[0u8, 1, 2]));
        assert!(!is_binary(b"plain text"));
    }

    #[test]
    fn binary_probe_is_capped_at_first_8000_bytes() {
        let mut content = vec![b'a'; 9000];
        content.push(0);
        assert!(!is_binary(&content));
    }

    // --- prepare_for_hash: 7 branches -----------------------------------

    #[test]
    fn unspecified_branch_matches_v0_1_default_for_text_and_binary() {
        let attrs = empty_attrs();
        let (text_out, text_flag) = prepare_for_hash(b"hello\r\n", false, &attrs, "any.txt");
        assert_eq!(text_out, b"hello\n");
        assert!(!text_flag);

        let binary = [0u8, 1, 2, 3];
        let (bin_out, bin_flag) = prepare_for_hash(&binary, false, &attrs, "any.bin");
        assert_eq!(bin_out, binary);
        assert!(bin_flag);
    }

    #[test]
    fn unspecified_branch_keeps_bom_when_requested() {
        let input = [0xEF, 0xBB, 0xBF, b'a'];
        let attrs = empty_attrs();
        let (out, is_bin) = prepare_for_hash(&input, true, &attrs, "any.txt");
        assert_eq!(out, input);
        assert!(!is_bin);
    }

    #[test]
    fn text_auto_branch_forces_text_even_with_nul_bytes() {
        // NUL-bearing bytes that v0.1 default would mark binary; text=auto
        // overrides → LF normalize, is_binary=false.
        let (_dir, attrs) = attrs_with("*.txt text=auto\n");
        let raw = b"a\x00b\r\nc";
        let (out, is_bin) = prepare_for_hash(raw, false, &attrs, "any.txt");
        assert!(!is_bin, "text=auto must override NUL heuristic");
        assert_eq!(out, b"a\x00b\nc");
    }

    #[test]
    fn binary_branch_skips_normalize_for_zero_nul_input() {
        // No NUL bytes means v0.1 default would treat as text + LF normalize;
        // explicit `binary` keeps raw bytes + marks binary.
        let (_dir, attrs) = attrs_with("*.bin binary\n");
        let raw = b"hello\r\nworld\r\n";
        let (out, is_bin) = prepare_for_hash(raw, false, &attrs, "data.bin");
        assert!(is_bin);
        assert_eq!(out, raw);
    }

    #[test]
    fn eol_lf_branch_normalizes_crlf() {
        let (_dir, attrs) = attrs_with("*.sh eol=lf\n");
        let (out, is_bin) = prepare_for_hash(b"line\r\n", false, &attrs, "run.sh");
        assert!(!is_bin);
        assert_eq!(out, b"line\n");
    }

    #[test]
    fn eol_crlf_branch_preserves_crlf_diverging_from_lf() {
        // Spec acceptance: local LF + remote CRLF must yield different SHA
        // when `*.txt eol=crlf` is set. apply_eol_crlf hashes raw bytes
        // (BOM strip only) so local-LF and remote-CRLF inputs diverge.
        let (_dir, attrs) = attrs_with("*.txt eol=crlf\n");
        let (lf_out, _) = prepare_for_hash(b"hello\n", false, &attrs, "notes.txt");
        let (crlf_out, _) = prepare_for_hash(b"hello\r\n", false, &attrs, "notes.txt");
        assert_ne!(blob_hash(&lf_out), blob_hash(&crlf_out));
        // BOM strip still applies.
        let (with_bom, _) = prepare_for_hash(b"\xEF\xBB\xBFhi\r\n", false, &attrs, "notes.txt");
        assert_eq!(with_bom, b"hi\r\n");
    }

    #[test]
    fn lfs_pointer_branch_falls_through_to_unspecified_default() {
        // Pipeline short-circuits LFS-tracked paths before hashing; if a
        // caller bypasses that, prepare_for_hash defensively returns the
        // v0.1 default output rather than panicking.
        let (_dir, attrs) = attrs_with("*.psd filter=lfs\n");
        let raw = b"hello\r\n";
        let (out, is_bin) = prepare_for_hash(raw, false, &attrs, "art/cover.psd");
        assert!(!is_bin);
        assert_eq!(out, b"hello\n");
    }

    #[test]
    fn unsupported_branch_falls_through_to_unspecified_default() {
        // Whitelist-miss attribute (`working-tree-encoding=...`) reaches
        // prepare_for_hash if the caller hasn't promoted to Failed. The
        // function returns v0.1 output defensively.
        let (_dir, attrs) = attrs_with("*.foo working-tree-encoding=UTF-16\n");
        let raw = b"hello\r\n";
        let (out, _) = prepare_for_hash(raw, false, &attrs, "weird.foo");
        assert_eq!(out, b"hello\n");
    }

    // --- lifetime contract ----------------------------------------------

    #[test]
    fn lifetime_contract_one_load_n_calls_no_clone_leak() {
        // Single vault scan → one Arc::new(GitAttributes::load) → N calls
        // share the same instance with no extra Arc clones leaked. Type
        // signature (`&Arc<GitAttributes>`) prevents reparse inside
        // prepare_for_hash; this test verifies the caller-side invariant.
        let (_dir, attrs) = attrs_with("*.sh eol=lf\n");
        for _ in 0..5 {
            let (out, _) = prepare_for_hash(b"x\r\n", false, &attrs, "run.sh");
            assert_eq!(out, b"x\n");
        }
        assert_eq!(
            Arc::strong_count(&attrs),
            1,
            "no Arc clones should leak across N calls"
        );
    }

    // --- cross-module invariant (decode + normalize) --------------------

    #[test]
    fn utf16_bom_passes_through_unchanged_for_hashing_and_normalize() {
        // (b) policy: detection does not perturb hash bytes. normalize_text
        // also leaves UTF-16 BOM alone (only UTF-8 BOM is stripped).
        use crate::shared::decode::try_decode_text;
        let utf16_le = [0xFFu8, 0xFE, 0x41, 0x00];
        let hash_before = blob_hash(&utf16_le);
        let _ = try_decode_text(&utf16_le);
        assert_eq!(blob_hash(&utf16_le), hash_before);
        assert_eq!(normalize_text(&utf16_le, false), utf16_le);
        // UTF-8 BOM by contrast is stripped (v0.1 behavior preserved).
        assert_eq!(normalize_text(&[0xEFu8, 0xBB, 0xBF, b'a'], false), b"a");
    }
}
