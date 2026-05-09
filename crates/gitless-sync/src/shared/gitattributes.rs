//! `.gitattributes` parser — working tree only.
//!
//! Loads project root + sub-directory `.gitattributes` files in a single pass
//! and exposes raw per-line attributes via [`GitAttributes::match_path`]. The
//! K1 task scope: parse + raw match. Whitelist mapping (`AttributeMatch` enum)
//! and last-wins-per-attribute reduction live in K1.5/K2.
//!
//! See `docs/specs/spec-hash-and-normalize.md` § `.gitattributes` 파서 and
//! `docs/specs/spec-config.md` § 위치 정책.
//!
//! Divergence from `.gitattributes(5)`:
//! - Lines whose pattern token starts with `!` (forbidden negation) are
//!   silently skipped. Attribute *tokens* prefixed with `!` (e.g. `!text`)
//!   are still parsed correctly as [`RawAttribute::Unspecified`].
//! - Trailing-slash directory patterns (`docs/`) match recursively because
//!   we reuse `ignore::gitignore::GitignoreBuilder`. K1 acceptance pins
//!   "gitignore-style glob"; revisit in K3/K4 if vault dogfooding regresses.
//! - Whitespace-escaped tokens (`foo\ bar text=auto`) are not handled —
//!   `split_whitespace` treats the backslash as a literal.
#![allow(dead_code)]

use std::fs;
use std::path::Path;

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use walkdir::WalkDir;

use crate::shared::error::GitlessError;

/// Raw attribute token before whitelist mapping (K1.5 turns this into
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
struct LineRule {
    matcher: Gitignore,
    attributes: Vec<RawAttribute>,
}

#[derive(Debug)]
struct AttributesFile {
    /// Working-tree-relative directory containing this `.gitattributes`,
    /// using forward slashes. Empty for root.
    source_dir: String,
    rules: Vec<LineRule>,
    depth: usize,
}

/// All `.gitattributes` discovered under one working tree root, sorted
/// shallowest-first so flat accumulation in [`Self::match_path`] yields the
/// deepest matching line at the tail (K1.5 reduces with last-wins).
#[derive(Debug, Default)]
pub(crate) struct GitAttributes {
    files: Vec<AttributesFile>,
}

impl GitAttributes {
    /// Walk `root`, parsing every `.gitattributes` file along the way.
    /// `.git/` is skipped for efficiency (out of scope per `spec-config.md`).
    ///
    /// # Errors
    /// Returns [`GitlessError::Io`] / [`GitlessError::Config`] on filesystem
    /// or pattern build failures.
    pub fn load(root: &Path) -> Result<Self, GitlessError> {
        let mut files = Vec::new();
        let walker = WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !is_dot_git_dir(e));
        for entry in walker {
            let entry = entry.map_err(walk_err_to_gitless)?;
            if !entry.file_type().is_file() || entry.file_name() != ".gitattributes" {
                continue;
            }
            let source_dir = relative_dir_forward_slash(entry.path(), root);
            let depth = if source_dir.is_empty() {
                0
            } else {
                source_dir.matches('/').count() + 1
            };
            let content = fs::read_to_string(entry.path())?;
            let dir_for_matcher = entry.path().parent().unwrap_or(root);
            let rules = parse_lines(&content, dir_for_matcher)?;
            files.push(AttributesFile {
                source_dir,
                rules,
                depth,
            });
        }
        files.sort_by_key(|f| f.depth);
        Ok(Self { files })
    }

    /// Every attribute matching `path` (working-tree-relative, forward
    /// slash). Order: shallowest file first, lines top-to-bottom — K1.5
    /// iterates and applies last-wins-per-attribute.
    #[must_use]
    pub fn match_path(&self, path: &str) -> Vec<RawAttribute> {
        let mut acc = Vec::new();
        for file in &self.files {
            let Some(relative) = strip_source_dir(path, &file.source_dir) else {
                continue;
            };
            for rule in &file.rules {
                if rule
                    .matcher
                    .matched_path_or_any_parents(Path::new(relative), false)
                    .is_ignore()
                {
                    acc.extend_from_slice(&rule.attributes);
                }
            }
        }
        acc
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

fn is_dot_git_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir() && entry.file_name() == ".git"
}

fn walk_err_to_gitless(err: walkdir::Error) -> GitlessError {
    err.into_io_error().map_or_else(
        || GitlessError::Config(".gitattributes walk error".into()),
        GitlessError::Io,
    )
}

fn relative_dir_forward_slash(file_path: &Path, root: &Path) -> String {
    file_path
        .parent()
        .and_then(|p| p.strip_prefix(root).ok())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn strip_source_dir<'a>(path: &'a str, source_dir: &str) -> Option<&'a str> {
    if source_dir.is_empty() {
        return Some(path);
    }
    if !path.starts_with(source_dir) {
        return None;
    }
    let after = &path[source_dir.len()..];
    after.strip_prefix('/')
}

fn parse_lines(content: &str, builder_root: &Path) -> Result<Vec<LineRule>, GitlessError> {
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
#[path = "gitattributes_tests.rs"]
mod tests;
