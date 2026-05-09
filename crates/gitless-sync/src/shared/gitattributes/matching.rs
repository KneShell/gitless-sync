//! Working-tree `.gitattributes` discovery + per-path attribute lookup.
//! Working tree only — `.git/info/attributes` and `~/.config/git/attributes`
//! are out of scope (`spec-config.md` § 위치 정책).

use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use super::classify::{AttributeMatch, classify_raw_attributes};
use super::parser::{LineRule, RawAttribute, parse_lines};
use crate::shared::error::GitlessError;

#[derive(Debug)]
struct AttributesFile {
    /// Working-tree-relative directory, forward-slash. Empty for root.
    source_dir: String,
    rules: Vec<LineRule>,
    depth: usize,
}

/// `.gitattributes` discovered under one working-tree root, sorted
/// shallowest-first so flat accumulation in [`Self::match_path`] yields the
/// deepest matching line at the tail. `pub` (not `pub(crate)`) so the bench
/// in `benches/gitattributes_match.rs` can build fixtures.
#[doc(hidden)]
#[derive(Debug, Default)]
pub struct GitAttributes {
    files: Vec<AttributesFile>,
}

impl GitAttributes {
    /// Walk `root`, parsing each `.gitattributes` (skipping `.git/`).
    ///
    /// # Errors
    /// [`GitlessError::Io`] / [`GitlessError::Config`] on fs or pattern
    /// build failures.
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

    /// Attributes matching `path` (working-tree-relative, forward slash),
    /// shallowest-first / top-to-bottom for last-wins reduction.
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

    /// Reduce matching attributes to one [`AttributeMatch`] — see
    /// `super::classify` for the 4-rule precedence.
    #[must_use]
    pub(crate) fn classify_path(&self, path: &str) -> AttributeMatch {
        classify_raw_attributes(&self.match_path(path))
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

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn kv(name: &str, value: &str) -> RawAttribute {
        RawAttribute::KeyValue {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    // load + match_path + is_empty ---------------------------------------

    #[test]
    fn empty_root_returns_empty_attributes() {
        let dir = TempDir::new().unwrap();
        let attrs = GitAttributes::load(dir.path()).unwrap();
        assert!(attrs.is_empty());
        assert!(attrs.match_path("any/file.txt").is_empty());
    }

    #[test]
    fn root_gitattributes_matches_simple_glob() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitattributes"), "*.txt text=auto\n").unwrap();
        let attrs = GitAttributes::load(dir.path()).unwrap();
        assert_eq!(attrs.match_path("notes.txt"), vec![kv("text", "auto")]);
        assert!(attrs.match_path("README.md").is_empty());
    }

    // Parser-token tests (comment/blank skip, negation skip, set/unset/kv
    // tokenization, pattern-only skip) live in `super::parser::tests`.

    #[test]
    fn line_level_order_preserves_natural_order() {
        let dir = TempDir::new().unwrap();
        let body = "*.txt text=auto\n*.txt eol=lf\n";
        fs::write(dir.path().join(".gitattributes"), body).unwrap();
        let attrs = GitAttributes::load(dir.path()).unwrap();
        assert_eq!(
            attrs.match_path("a.txt"),
            vec![kv("text", "auto"), kv("eol", "lf")]
        );
    }

    #[test]
    fn sub_dir_attributes_do_not_match_outside_their_dir() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("docs");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join(".gitattributes"), "*.md text=auto\n").unwrap();
        let attrs = GitAttributes::load(dir.path()).unwrap();
        assert!(attrs.match_path("README.md").is_empty());
        assert!(!attrs.match_path("docs/foo.md").is_empty());
    }

    #[test]
    fn dot_git_directory_is_skipped() {
        let dir = TempDir::new().unwrap();
        let dot_git = dir.path().join(".git");
        fs::create_dir(&dot_git).unwrap();
        fs::write(dot_git.join(".gitattributes"), "*.txt text=auto\n").unwrap();
        let attrs = GitAttributes::load(dir.path()).unwrap();
        assert!(attrs.is_empty());
    }

    #[test]
    fn target_directory_is_not_skipped() {
        // BUILTIN_IGNORES is for scan walking, not attribute discovery —
        // working-tree dirs (target/, node_modules/) still load attributes.
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join(".gitattributes"), "*.bin binary\n").unwrap();
        let attrs = GitAttributes::load(dir.path()).unwrap();
        assert_eq!(
            attrs.match_path("target/foo.bin"),
            vec![RawAttribute::Set("binary".into())]
        );
    }

    #[test]
    fn three_level_match_path_preserves_root_to_deepest_order() {
        // K4: root + `a/` + `a/b/` each contribute one rule for the same
        // path; raw accumulation must be shallowest first, deepest last so
        // the K1.5 reducer's last-wins lands on the deepest file.
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitattributes"), "*.txt text=auto\n").unwrap();
        let lvl2 = dir.path().join("a");
        fs::create_dir(&lvl2).unwrap();
        fs::write(lvl2.join(".gitattributes"), "*.txt eol=lf\n").unwrap();
        let lvl3 = lvl2.join("b");
        fs::create_dir(&lvl3).unwrap();
        fs::write(lvl3.join(".gitattributes"), "*.txt eol=crlf\n").unwrap();
        let attrs = GitAttributes::load(dir.path()).unwrap();
        assert_eq!(
            attrs.match_path("a/b/notes.txt"),
            vec![kv("text", "auto"), kv("eol", "lf"), kv("eol", "crlf")]
        );
    }

    // classify_path integration over multi-level fixtures ----------------

    #[test]
    fn classify_path_empty_match_returns_unspecified() {
        let dir = TempDir::new().unwrap();
        let attrs = GitAttributes::load(dir.path()).unwrap();
        assert_eq!(
            attrs.classify_path("anything.txt"),
            AttributeMatch::Unspecified
        );
    }

    #[test]
    fn classify_path_three_level_uses_deepest_winner() {
        // root says text=auto, a/ says eol=lf, a/b/ says eol=crlf; deepest wins.
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitattributes"), "*.txt text=auto\n").unwrap();
        let lvl2 = dir.path().join("a");
        fs::create_dir(&lvl2).unwrap();
        fs::write(lvl2.join(".gitattributes"), "*.txt eol=lf\n").unwrap();
        let lvl3 = lvl2.join("b");
        fs::create_dir(&lvl3).unwrap();
        fs::write(lvl3.join(".gitattributes"), "*.txt eol=crlf\n").unwrap();
        let attrs = GitAttributes::load(dir.path()).unwrap();
        assert_eq!(
            attrs.classify_path("a/b/notes.txt"),
            AttributeMatch::EolCrlf
        );
    }

    #[test]
    fn classify_path_three_level_with_line_level_last_match_wins() {
        // Deepest .gitattributes has two lines matching the same path; the
        // last line wins on top of the depth ordering.
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitattributes"), "*.txt text=auto\n").unwrap();
        let lvl2 = dir.path().join("a");
        fs::create_dir(&lvl2).unwrap();
        fs::write(lvl2.join(".gitattributes"), "*.txt eol=lf\n").unwrap();
        let lvl3 = lvl2.join("b");
        fs::create_dir(&lvl3).unwrap();
        fs::write(
            lvl3.join(".gitattributes"),
            "*.txt eol=lf\n*.txt eol=crlf\n",
        )
        .unwrap();
        let attrs = GitAttributes::load(dir.path()).unwrap();
        assert_eq!(
            attrs.classify_path("a/b/notes.txt"),
            AttributeMatch::EolCrlf
        );
    }
}
