//! Whitelist mapping that reduces a path's accumulated [`RawAttribute`]
//! tokens to a single [`AttributeMatch`] hash-mode bucket consumed by
//! `prepare_for_hash`. Five whitelist variants (text=auto / binary / eol=lf /
//! eol=crlf / filter=lfs) plus `Unspecified` (v0.1 default) and `Unsupported`
//! (out-of-whitelist passthrough preserving the literal attribute name).
//!
//! K1.5 precedence (pinned in `spec-domain-pitfalls.md` § `.gitattributes`
//! 화이트리스트):
//! 1. any `filter=lfs` → [`AttributeMatch::LfsPointer`] (canonical git-lfs
//!    marker is authoritative even when co-attributes look unsupported);
//! 2. any non-whitelist token → [`AttributeMatch::Unsupported`] with the
//!    first such literal name (fires even when a whitelist token also
//!    matches);
//! 3. otherwise last-wins among the four hash-mode variants;
//! 4. empty match → [`AttributeMatch::Unspecified`].

use super::parser::RawAttribute;

/// Whitelisted hash-mode classification, derived from `.gitattributes`
/// matches via `GitAttributes::classify_path`. K2 routes each variant to a
/// `prepare_for_hash` branch (text=auto / binary / eol=lf / eol=crlf / LFS
/// pointer / v0.1 default / failed-with-reason).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeMatch {
    TextAuto,
    Binary,
    EolLf,
    EolCrlf,
    LfsPointer,
    Unspecified,
    Unsupported { attribute_name: String },
}

#[must_use]
pub(super) fn classify_raw_attributes(raws: &[RawAttribute]) -> AttributeMatch {
    let mut last_whitelist: Option<AttributeMatch> = None;
    let mut first_unsupported: Option<String> = None;
    for raw in raws {
        match whitelist_match(raw) {
            Some(AttributeMatch::LfsPointer) => return AttributeMatch::LfsPointer,
            Some(matched) => last_whitelist = Some(matched),
            None => {
                if first_unsupported.is_none() {
                    first_unsupported = Some(unsupported_name(raw));
                }
            }
        }
    }
    if let Some(name) = first_unsupported {
        return AttributeMatch::Unsupported {
            attribute_name: name,
        };
    }
    last_whitelist.unwrap_or(AttributeMatch::Unspecified)
}

fn whitelist_match(raw: &RawAttribute) -> Option<AttributeMatch> {
    match raw {
        RawAttribute::Set(name) if name == "binary" => Some(AttributeMatch::Binary),
        RawAttribute::KeyValue { name, value } => match (name.as_str(), value.as_str()) {
            ("text", "auto") => Some(AttributeMatch::TextAuto),
            ("eol", "lf") => Some(AttributeMatch::EolLf),
            ("eol", "crlf") => Some(AttributeMatch::EolCrlf),
            ("filter", "lfs") => Some(AttributeMatch::LfsPointer),
            _ => None,
        },
        _ => None,
    }
}

fn unsupported_name(raw: &RawAttribute) -> String {
    match raw {
        RawAttribute::Set(name)
        | RawAttribute::Unset(name)
        | RawAttribute::Unspecified(name)
        | RawAttribute::KeyValue { name, .. } => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(name: &str, value: &str) -> RawAttribute {
        RawAttribute::KeyValue {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    fn set(name: &str) -> RawAttribute {
        RawAttribute::Set(name.to_string())
    }

    fn unset(name: &str) -> RawAttribute {
        RawAttribute::Unset(name.to_string())
    }

    fn unspecified(name: &str) -> RawAttribute {
        RawAttribute::Unspecified(name.to_string())
    }

    fn unsupported(name: &str) -> AttributeMatch {
        AttributeMatch::Unsupported {
            attribute_name: name.into(),
        }
    }

    // Empty / single whitelist variants ------------------------------------

    #[test]
    fn empty_input_returns_unspecified() {
        assert_eq!(classify_raw_attributes(&[]), AttributeMatch::Unspecified);
    }

    #[test]
    fn text_auto_keyvalue_returns_text_auto() {
        assert_eq!(
            classify_raw_attributes(&[kv("text", "auto")]),
            AttributeMatch::TextAuto
        );
    }

    #[test]
    fn binary_set_returns_binary() {
        assert_eq!(
            classify_raw_attributes(&[set("binary")]),
            AttributeMatch::Binary
        );
    }

    #[test]
    fn eol_lf_keyvalue_returns_eol_lf() {
        assert_eq!(
            classify_raw_attributes(&[kv("eol", "lf")]),
            AttributeMatch::EolLf
        );
    }

    #[test]
    fn eol_crlf_keyvalue_returns_eol_crlf() {
        assert_eq!(
            classify_raw_attributes(&[kv("eol", "crlf")]),
            AttributeMatch::EolCrlf
        );
    }

    #[test]
    fn filter_lfs_returns_lfs_pointer() {
        assert_eq!(
            classify_raw_attributes(&[kv("filter", "lfs")]),
            AttributeMatch::LfsPointer
        );
    }

    // Unsupported scenarios ------------------------------------------------

    #[test]
    fn working_tree_encoding_keyvalue_is_unsupported() {
        assert_eq!(
            classify_raw_attributes(&[kv("working-tree-encoding", "UTF-16")]),
            unsupported("working-tree-encoding")
        );
    }

    #[test]
    fn ident_set_is_unsupported() {
        assert_eq!(
            classify_raw_attributes(&[set("ident")]),
            unsupported("ident")
        );
    }

    #[test]
    fn filter_clean_non_lfs_is_unsupported() {
        assert_eq!(
            classify_raw_attributes(&[kv("filter", "clean")]),
            unsupported("filter")
        );
    }

    #[test]
    fn legacy_crlf_bare_is_unsupported() {
        // Legacy `crlf` (Set form). Only `eol=crlf` (KeyValue) is whitelisted.
        assert_eq!(classify_raw_attributes(&[set("crlf")]), unsupported("crlf"));
    }

    #[test]
    fn eol_native_is_unsupported() {
        // Only `eol=lf` and `eol=crlf` are in the whitelist.
        assert_eq!(
            classify_raw_attributes(&[kv("eol", "native")]),
            unsupported("eol")
        );
    }

    #[test]
    fn bare_text_set_is_unsupported() {
        // Whitelist requires `text=auto` (KeyValue). Bare `text` (Set) is not.
        assert_eq!(classify_raw_attributes(&[set("text")]), unsupported("text"));
    }

    #[test]
    fn unset_text_is_unsupported() {
        // `-text` (Unset) is not in the whitelist.
        assert_eq!(
            classify_raw_attributes(&[unset("text")]),
            unsupported("text")
        );
    }

    #[test]
    fn unspecified_bang_text_is_unsupported() {
        // `!text` (default-restoring) is not whitelisted either.
        assert_eq!(
            classify_raw_attributes(&[unspecified("text")]),
            unsupported("text")
        );
    }

    // Precedence rules -----------------------------------------------------

    #[test]
    fn filter_lfs_with_co_attributes_wins_canonical_marker() {
        // Canonical git-lfs line: filter=lfs is authoritative; the trailing
        // unsupported tokens (-text, diff=lfs, merge=lfs) do not override.
        assert_eq!(
            classify_raw_attributes(&[
                kv("filter", "lfs"),
                kv("diff", "lfs"),
                kv("merge", "lfs"),
                unset("text"),
            ]),
            AttributeMatch::LfsPointer
        );
    }

    #[test]
    fn filter_lfs_wins_over_whitelist_co_attributes() {
        assert_eq!(
            classify_raw_attributes(&[kv("filter", "lfs"), set("binary")]),
            AttributeMatch::LfsPointer
        );
    }

    #[test]
    fn unsupported_wins_over_whitelist_when_no_lfs() {
        // Rule 2 fires even when a whitelist attribute also matches.
        assert_eq!(
            classify_raw_attributes(&[kv("working-tree-encoding", "UTF-16"), kv("eol", "lf")]),
            unsupported("working-tree-encoding")
        );
    }

    #[test]
    fn unsupported_attribute_name_is_first_non_whitelist() {
        // First non-whitelist token wins the attribute_name.
        assert_eq!(
            classify_raw_attributes(&[set("ident"), kv("filter", "clean")]),
            unsupported("ident")
        );
    }

    #[test]
    fn text_auto_then_binary_uses_last_wins() {
        assert_eq!(
            classify_raw_attributes(&[kv("text", "auto"), set("binary")]),
            AttributeMatch::Binary
        );
    }

    #[test]
    fn binary_then_text_auto_uses_last_wins() {
        assert_eq!(
            classify_raw_attributes(&[set("binary"), kv("text", "auto")]),
            AttributeMatch::TextAuto
        );
    }
}
