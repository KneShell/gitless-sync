const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const UTF16_LE_BOM: &[u8] = &[0xFF, 0xFE];
const UTF16_BE_BOM: &[u8] = &[0xFE, 0xFF];

const TEXT_DECODE_SHORTLIST: &[&encoding_rs::Encoding] = &[
    encoding_rs::SHIFT_JIS,
    encoding_rs::EUC_KR,
    encoding_rs::GBK,
    encoding_rs::WINDOWS_1252,
];

/// Outcome of a non-mutating decode attempt for non-UTF-8 inputs.
///
/// Hash input remains the original raw bytes regardless of variant
/// (`spec-domain-pitfalls.md` § Encoding (b) policy). Detection is purely
/// informational — callers surface `failed_reason: "encoding"` on
/// [`TextDecodeResult::Unknown`] **or** [`TextDecodeResult::Utf16Bom`]
/// (UTF-16 is out of scope for v0.2; see `spec-hash-and-normalize.md` § BOM).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDecodeResult {
    Utf8,
    Detected {
        encoding: &'static str,
    },
    /// UTF-16 BOM detected. Surfaced as `failed_reason: "encoding"` by the
    /// caller — UTF-16 conversion is out of scope for v0.2.
    Utf16Bom {
        little_endian: bool,
    },
    Unknown,
}

/// Attempt to identify the text encoding of `raw` without modifying it.
///
/// Order: UTF-16 BOM (`FF FE` LE / `FE FF` BE) first; then UTF-8; on UTF-8
/// failure, walk a curated shortlist (`Shift_JIS`, `EUC-KR`, `GBK`,
/// `Windows-1252`) and return the first encoding that decodes without
/// replacement characters. Returns [`TextDecodeResult::Unknown`] when every
/// shortlist entry reports errors.
///
/// UTF-8 BOM (`EF BB BF`) is valid UTF-8 (encodes U+FEFF) and is reported as
/// [`TextDecodeResult::Utf8`]; BOM stripping is `normalize_text`'s job.
///
/// Note: WHATWG `Windows-1252` maps every byte 0x00–0xFF to a code point,
/// so it never reports `had_errors`. With the current shortlist Unknown is
/// effectively unreachable for non-empty input — kept as a forward-compatible
/// signal for future shortlist refinements.
#[must_use]
pub fn try_decode_text(raw: &[u8]) -> TextDecodeResult {
    if raw.starts_with(UTF16_LE_BOM) {
        return TextDecodeResult::Utf16Bom {
            little_endian: true,
        };
    }
    if raw.starts_with(UTF16_BE_BOM) {
        return TextDecodeResult::Utf16Bom {
            little_endian: false,
        };
    }
    if std::str::from_utf8(raw).is_ok() {
        return TextDecodeResult::Utf8;
    }
    for enc in TEXT_DECODE_SHORTLIST {
        let (_, _, had_errors) = enc.decode(raw);
        if !had_errors {
            return TextDecodeResult::Detected {
                encoding: enc.name(),
            };
        }
    }
    TextDecodeResult::Unknown
}

#[must_use]
pub fn is_binary(content: &[u8]) -> bool {
    let probe_len = content.len().min(8000);
    content[..probe_len].contains(&0)
}

#[must_use]
pub fn normalize_text(content: &[u8], keep_bom: bool) -> Vec<u8> {
    let body = if !keep_bom && content.starts_with(UTF8_BOM) {
        &content[UTF8_BOM.len()..]
    } else {
        content
    };

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

#[must_use]
pub fn prepare_for_hash(content: &[u8], keep_bom: bool) -> (Vec<u8>, bool) {
    if is_binary(content) {
        (content.to_vec(), true)
    } else {
        (normalize_text(content, keep_bom), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn prepare_for_hash_returns_correct_flag() {
        let (text_out, text_flag) = prepare_for_hash(b"hello\r\n", false);
        assert_eq!(text_out, b"hello\n");
        assert!(!text_flag);

        let binary = [0u8, 1, 2, 3];
        let (bin_out, bin_flag) = prepare_for_hash(&binary, false);
        assert_eq!(bin_out, binary);
        assert!(bin_flag);
    }

    #[test]
    fn prepare_for_hash_keeps_bom_when_requested() {
        let input = [0xEF, 0xBB, 0xBF, b'a'];
        let (out, is_bin) = prepare_for_hash(&input, true);
        assert_eq!(out, input);
        assert!(!is_bin);
    }

    #[test]
    fn try_decode_text_returns_utf8_for_ascii() {
        assert_eq!(try_decode_text(b"hello"), TextDecodeResult::Utf8);
    }

    #[test]
    fn try_decode_text_returns_utf8_for_valid_utf8_multibyte() {
        assert_eq!(try_decode_text("한글".as_bytes()), TextDecodeResult::Utf8);
        assert_eq!(try_decode_text("café".as_bytes()), TextDecodeResult::Utf8);
        assert_eq!(try_decode_text("日本語".as_bytes()), TextDecodeResult::Utf8);
    }

    #[test]
    fn try_decode_text_returns_utf8_for_empty_input() {
        assert_eq!(try_decode_text(&[]), TextDecodeResult::Utf8);
    }

    #[test]
    fn try_decode_text_returns_detected_for_euc_kr_bytes() {
        // "한글" encoded as EUC-KR (CP949). Not valid UTF-8 (every byte is a
        // bare continuation-range byte for UTF-8). The shortlist will pick
        // some encoding — we don't pin the name per (b) policy.
        let euc_kr = [0xC7u8, 0xD1, 0xB1, 0xDB];
        let result = try_decode_text(&euc_kr);
        assert!(
            matches!(result, TextDecodeResult::Detected { .. }),
            "expected Detected, got {result:?}"
        );
    }

    #[test]
    fn try_decode_text_returns_detected_for_shift_jis_bytes() {
        // "あ" (U+3042) encoded as Shift_JIS = [0x82, 0xA0].
        let shift_jis = [0x82u8, 0xA0];
        let result = try_decode_text(&shift_jis);
        assert!(
            matches!(result, TextDecodeResult::Detected { .. }),
            "expected Detected, got {result:?}"
        );
    }

    #[test]
    fn try_decode_text_returns_detected_for_latin1_bytes() {
        // © (0xA9), ® (0xAE), 'é' (0xE9) — valid Windows-1252 / Latin-1,
        // not valid UTF-8 standalone.
        let latin1 = [0xA9u8, 0xAE, 0xE9];
        let result = try_decode_text(&latin1);
        assert!(
            matches!(result, TextDecodeResult::Detected { .. }),
            "expected Detected, got {result:?}"
        );
    }

    #[test]
    fn try_decode_text_preserves_raw_bytes_for_hashing() {
        // (b) policy: detection must not perturb the bytes that flow into
        // the hash. Same raw EUC-KR file on local + remote → same blob hash
        // regardless of detection variant.
        use crate::shared::hash::blob_hash;
        let euc_kr_local = [0xC7u8, 0xD1, 0xB1, 0xDB];
        let euc_kr_remote = [0xC7u8, 0xD1, 0xB1, 0xDB];
        let hash_before = blob_hash(&euc_kr_local);
        let _ = try_decode_text(&euc_kr_local);
        let hash_after = blob_hash(&euc_kr_local);
        assert_eq!(hash_before, hash_after);
        assert_eq!(blob_hash(&euc_kr_local), blob_hash(&euc_kr_remote));
    }

    #[test]
    fn try_decode_text_is_deterministic() {
        let input = [0xC7u8, 0xD1, 0xB1, 0xDB];
        assert_eq!(try_decode_text(&input), try_decode_text(&input));
    }

    #[test]
    fn try_decode_text_detects_utf16_bom_le_be_alone_and_with_payload() {
        let cases = [
            (vec![0xFFu8, 0xFE, 0x41, 0x00], true),  // LE "A"
            (vec![0xFEu8, 0xFF, 0x00, 0x41], false), // BE "A"
            (vec![0xFFu8, 0xFE], true),              // LE BOM-only
            (vec![0xFEu8, 0xFF], false),             // BE BOM-only
        ];
        for (raw, le) in cases {
            assert_eq!(
                try_decode_text(&raw),
                TextDecodeResult::Utf16Bom { little_endian: le }
            );
        }
    }

    #[test]
    fn try_decode_text_separates_utf8_bom_and_short_prefix_from_utf16() {
        // UTF-8 BOM (EF BB BF) encodes U+FEFF — valid UTF-8, not Utf16Bom.
        assert_eq!(
            try_decode_text(&[0xEFu8, 0xBB, 0xBF, b'a']),
            TextDecodeResult::Utf8
        );
        // Single byte cannot match a 2-byte BOM. Falls through to shortlist.
        for short in [[0xFFu8], [0xFEu8]] {
            assert!(matches!(
                try_decode_text(&short),
                TextDecodeResult::Detected { .. }
            ));
        }
    }

    #[test]
    fn utf16_bom_passes_through_unchanged_for_hashing_and_normalize() {
        // (b) policy: detection does not perturb hash bytes. normalize_text
        // also leaves UTF-16 BOM alone (only UTF-8 BOM is stripped).
        use crate::shared::hash::blob_hash;
        let utf16_le = [0xFFu8, 0xFE, 0x41, 0x00];
        let hash_before = blob_hash(&utf16_le);
        let _ = try_decode_text(&utf16_le);
        assert_eq!(blob_hash(&utf16_le), hash_before);
        assert_eq!(normalize_text(&utf16_le, false), utf16_le);
        // UTF-8 BOM by contrast is stripped (v0.1 behavior preserved).
        assert_eq!(normalize_text(&[0xEFu8, 0xBB, 0xBF, b'a'], false), b"a");
    }
}
