//! Text encoding detection — non-mutating sniff used by callers to surface
//! `failed_reason: "encoding"` (`spec-domain-pitfalls.md` § Encoding (b) policy
//! and `spec-hash-and-normalize.md` § BOM). The hash input is always the
//! original raw bytes — detection is informational only.

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

#[cfg(test)]
mod tests {
    use super::*;

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
        // the hash. Three encodings exercise distinct encoding_rs decoder
        // paths (EUC-KR/CP949 stateful · Shift_JIS lead+trail multi-byte ·
        // Windows-1252 single-byte table) — input diversity ≈ codepath
        // diversity, not tautology.
        use crate::shared::hash::blob_hash;
        let cases: &[&[u8]] = &[
            &[0xC7, 0xD1, 0xB1, 0xDB], // EUC-KR "한글"
            &[0x82, 0xA0],             // Shift_JIS "あ"
            &[0xA9, 0xAE, 0xE9],       // Latin-1 © ® é
        ];
        for raw in cases {
            let local = raw.to_vec();
            let remote = raw.to_vec();
            let hash_before = blob_hash(&local);
            let _ = try_decode_text(&local);
            let hash_after = blob_hash(&local);
            assert_eq!(hash_before, hash_after, "decode mutated bytes for {raw:?}");
            assert_eq!(
                blob_hash(&local),
                blob_hash(&remote),
                "identical raw bytes diverged on hash for {raw:?}"
            );
        }
    }

    #[test]
    fn prepare_for_hash_preserves_non_utf8_raw_bytes_via_pipeline() {
        // 변환 시나리오 — decode + normalize boundary. Non-UTF-8 raw bytes
        // pass through `prepare_for_hash` (default policy, no .gitattributes)
        // unchanged for hashing. NUL-free non-UTF-8 inputs go through the
        // text-normalize branch — no `\r\n` so output == input == hash input.
        // Local + remote with identical raw bytes → identical blob_hash via
        // the same chain a real scan traverses.
        use crate::shared::gitattributes::GitAttributes;
        use crate::shared::hash::blob_hash;
        use crate::shared::normalize::prepare_for_hash;
        use std::sync::Arc;

        let attrs = Arc::new(GitAttributes::default());
        let cases: &[(&[u8], &str)] = &[
            (&[0xC7, 0xD1, 0xB1, 0xDB], "ko/notes.txt"), // EUC-KR
            (&[0x82, 0xA0], "ja/letter.txt"),            // Shift_JIS
            (&[0xA9, 0xAE, 0xE9], "eu/symbols.txt"),     // Latin-1
        ];
        for (raw, path) in cases {
            let (local_prep, local_bin) = prepare_for_hash(raw, false, &attrs, path);
            let (remote_prep, remote_bin) = prepare_for_hash(raw, false, &attrs, path);
            assert_eq!(local_prep, remote_prep, "prepare diverged for {path}");
            assert_eq!(local_bin, remote_bin, "is_binary diverged for {path}");
            assert_eq!(
                local_prep, *raw,
                "prepare mutated NUL-free non-UTF-8 raw bytes for {path}"
            );
            assert_eq!(
                blob_hash(&local_prep),
                blob_hash(&remote_prep),
                "blob_hash diverged on identical raw bytes for {path}"
            );
        }
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
}
