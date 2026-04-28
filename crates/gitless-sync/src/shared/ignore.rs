use std::path::Path;

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::shared::error::GitlessError;

pub const BUILTIN_IGNORES: &[&str] = &[
    ".git/",
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
    "node_modules/",
    "target/",
];

pub struct IgnoreMatcher {
    gitignore: Gitignore,
}

impl IgnoreMatcher {
    /// Build a matcher from the union of: builtin defaults, the root's
    /// `.gitignore` (if any), and `custom_patterns` (e.g., from CLI/config).
    ///
    /// # Errors
    /// Returns [`GitlessError::Config`] when a pattern fails to parse or the
    /// underlying matcher cannot be built.
    pub fn new(root: &Path, custom_patterns: &[String]) -> Result<Self, GitlessError> {
        let mut builder = GitignoreBuilder::new(root);

        for pattern in BUILTIN_IGNORES {
            builder
                .add_line(None, pattern)
                .map_err(|e| GitlessError::Config(format!("builtin ignore pattern: {e}")))?;
        }

        let gitignore_path = root.join(".gitignore");
        if gitignore_path.is_file()
            && let Some(err) = builder.add(&gitignore_path)
        {
            return Err(GitlessError::Config(format!(".gitignore: {err}")));
        }

        for pattern in custom_patterns {
            builder
                .add_line(None, pattern)
                .map_err(|e| GitlessError::Config(format!("custom ignore pattern: {e}")))?;
        }

        let gitignore = builder
            .build()
            .map_err(|e| GitlessError::Config(format!("ignore matcher build: {e}")))?;

        Ok(Self { gitignore })
    }

    /// Returns true when `path` matches any pattern (and is not whitelisted by
    /// a later negation). Input separators are normalized to forward slash so
    /// Windows backslash paths match the same as POSIX paths (G-004).
    #[must_use]
    pub fn is_ignored(&self, path: &Path) -> bool {
        let normalized = path.to_string_lossy().replace('\\', "/");
        self.gitignore
            .matched_path_or_any_parents(Path::new(&normalized), false)
            .is_ignore()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn matcher_with(custom: &[&str]) -> (TempDir, IgnoreMatcher) {
        let dir = TempDir::new().expect("create tempdir");
        let custom_owned: Vec<String> = custom.iter().map(|s| (*s).to_string()).collect();
        let matcher = IgnoreMatcher::new(dir.path(), &custom_owned).expect("build matcher");
        (dir, matcher)
    }

    #[test]
    fn new_succeeds_without_gitignore() {
        let dir = TempDir::new().unwrap();
        let matcher = IgnoreMatcher::new(dir.path(), &[]).expect("no gitignore should be ok");
        assert!(!matcher.is_ignored(Path::new("readme.md")));
    }

    #[test]
    fn builtin_matches_dot_git_head() {
        let (_dir, m) = matcher_with(&[]);
        assert!(m.is_ignored(Path::new(".git/HEAD")));
    }

    #[test]
    fn builtin_matches_node_modules_subpath() {
        let (_dir, m) = matcher_with(&[]);
        assert!(m.is_ignored(Path::new("node_modules/foo")));
    }

    #[test]
    fn builtin_matches_target_subpath() {
        let (_dir, m) = matcher_with(&[]);
        assert!(m.is_ignored(Path::new("target/debug/build/foo")));
    }

    #[test]
    fn builtin_matches_ds_store_anywhere() {
        let (_dir, m) = matcher_with(&[]);
        assert!(m.is_ignored(Path::new(".DS_Store")));
        assert!(m.is_ignored(Path::new("nested/dir/.DS_Store")));
    }

    #[test]
    fn custom_glob_matches_log_file() {
        let (_dir, m) = matcher_with(&["*.log"]);
        assert!(m.is_ignored(Path::new("debug.log")));
    }

    #[test]
    fn custom_glob_does_not_match_other_extension() {
        let (_dir, m) = matcher_with(&["*.log"]);
        assert!(!m.is_ignored(Path::new("notes.md")));
    }

    #[test]
    fn gitignore_dir_pattern_matches_subpath() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), "dist/\n").unwrap();
        let m = IgnoreMatcher::new(dir.path(), &[]).unwrap();
        assert!(m.is_ignored(Path::new("dist/bundle.js")));
    }

    #[test]
    fn gitignore_and_custom_form_union() {
        // PRD scenario 9: .gitignore + --ignore are unioned together.
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), "dist/\n").unwrap();
        let m = IgnoreMatcher::new(dir.path(), &["*.bak".to_string()]).unwrap();
        assert!(m.is_ignored(Path::new("dist/bundle.js")));
        assert!(m.is_ignored(Path::new("notes.bak")));
        assert!(!m.is_ignored(Path::new("src/main.rs")));
    }

    #[test]
    fn windows_backslash_normalized_to_forward_slash() {
        let (_dir, m) = matcher_with(&[]);
        assert!(m.is_ignored(Path::new(r".git\HEAD")));
        assert!(m.is_ignored(Path::new(r"node_modules\foo\bar")));
    }

    #[test]
    fn negation_pattern_overrides_earlier_match() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), "*.log\n!keep.log\n").unwrap();
        let m = IgnoreMatcher::new(dir.path(), &[]).unwrap();
        assert!(m.is_ignored(Path::new("debug.log")));
        assert!(!m.is_ignored(Path::new("keep.log")));
    }

    #[test]
    fn unmatched_path_returns_false() {
        let (_dir, m) = matcher_with(&[]);
        assert!(!m.is_ignored(Path::new("src/main.rs")));
        assert!(!m.is_ignored(Path::new("README.md")));
    }
}
