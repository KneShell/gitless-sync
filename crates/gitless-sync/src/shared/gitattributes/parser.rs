//! `.gitattributes` line-level tokenizer.
//!
//! Splits each non-empty, non-comment line into a `(pattern, attributes)`
//! pair, builds a single-pattern `gitignore::Gitignore` matcher per line, and
//! decodes each whitespace-separated attribute token into [`RawAttribute`].
//! `classify::classify_raw_attributes` consumes these raw tokens; tying the
//! parser to `AttributeMatch` directly would lose the literal name needed to
//! surface unsupported attributes.
//!
//! Divergences from `.gitattributes(5)`: pattern lines starting with `!` are
//! silently skipped (forbidden negation); whitespace-escaped tokens
//! (`foo\ bar`) are not handled.

use std::path::Path;

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::shared::error::GitlessError;

/// Raw attribute token before whitelist mapping (`classify` turns this into
/// `AttributeMatch`). Names preserve the form the user wrote so unsupported
/// attributes can surface their literal name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawAttribute {
    /// Bare attribute: `text`, `binary`, ...
    Set(String),
    /// Negated attribute: `-text`, `-diff`, ...
    Unset(String),
    /// Key-value attribute: `eol=lf`, `filter=lfs`, ...
    KeyValue { name: String, value: String },
    /// Default-restoring attribute: `!text`.
    Unspecified(String),
}

#[derive(Debug)]
pub(super) struct LineRule {
    pub(super) matcher: Gitignore,
    pub(super) attributes: Vec<RawAttribute>,
}

pub(super) fn parse_lines(
    content: &str,
    builder_root: &Path,
) -> Result<Vec<LineRule>, GitlessError> {
    let mut rules = Vec::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rule) = parse_one_line(line, builder_root)? {
            rules.push(rule);
        }
    }
    Ok(rules)
}

fn parse_one_line(line: &str, builder_root: &Path) -> Result<Option<LineRule>, GitlessError> {
    let mut tokens = line.split_whitespace();
    let Some(pattern) = tokens.next() else {
        return Ok(None);
    };
    if pattern.starts_with('!') {
        return Ok(None);
    }
    let attributes: Vec<RawAttribute> = tokens.map(parse_attribute).collect();
    if attributes.is_empty() {
        return Ok(None);
    }
    let mut builder = GitignoreBuilder::new(builder_root);
    builder
        .add_line(None, pattern)
        .map_err(|e| GitlessError::Config(format!(".gitattributes pattern: {e}")))?;
    let matcher = builder
        .build()
        .map_err(|e| GitlessError::Config(format!(".gitattributes build: {e}")))?;
    Ok(Some(LineRule {
        matcher,
        attributes,
    }))
}

fn parse_attribute(token: &str) -> RawAttribute {
    if let Some(rest) = token.strip_prefix('!') {
        return RawAttribute::Unspecified(rest.to_string());
    }
    if let Some(rest) = token.strip_prefix('-') {
        return RawAttribute::Unset(rest.to_string());
    }
    if let Some((name, value)) = token.split_once('=') {
        return RawAttribute::KeyValue {
            name: name.to_string(),
            value: value.to_string(),
        };
    }
    RawAttribute::Set(token.to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/")
    }

    #[test]
    fn parse_attribute_returns_set_for_bare_name() {
        assert_eq!(
            parse_attribute("binary"),
            RawAttribute::Set("binary".into())
        );
    }

    #[test]
    fn parse_attribute_returns_unset_for_dash_prefix() {
        assert_eq!(parse_attribute("-text"), RawAttribute::Unset("text".into()));
    }

    #[test]
    fn parse_attribute_returns_keyvalue_for_equals() {
        assert_eq!(
            parse_attribute("eol=lf"),
            RawAttribute::KeyValue {
                name: "eol".into(),
                value: "lf".into(),
            }
        );
    }

    #[test]
    fn parse_attribute_returns_unspecified_for_bang_prefix() {
        assert_eq!(
            parse_attribute("!text"),
            RawAttribute::Unspecified("text".into())
        );
    }

    #[test]
    fn parse_attribute_dash_outranks_equals_when_both_prefix() {
        assert_eq!(
            parse_attribute("-eol=lf"),
            RawAttribute::Unset("eol=lf".into())
        );
    }

    #[test]
    fn parse_one_line_skips_negation_pattern() {
        let result = parse_one_line("!keep.log binary", &root()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn parse_one_line_skips_pattern_without_attributes() {
        let result = parse_one_line("*.txt", &root()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn parse_lines_skips_blank_and_comment_lines() {
        let body = "# header\n\n*.txt text=auto\n# trailing\n";
        let rules = parse_lines(body, &root()).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(
            rules[0].attributes,
            vec![RawAttribute::KeyValue {
                name: "text".into(),
                value: "auto".into(),
            }]
        );
    }
}
