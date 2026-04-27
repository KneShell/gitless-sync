use std::path::Path;

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
    // TODO(ralph): hold an `ignore::gitignore::Gitignore` plus custom patterns
    _placeholder: (),
}

impl IgnoreMatcher {
    pub fn new(root: &Path, custom_patterns: &[String]) -> Result<Self, GitlessError> {
        let _ = (root, custom_patterns);
        todo!("build matcher: builtin defaults + .gitignore + custom_patterns (union)")
    }

    pub fn is_ignored(&self, path: &Path) -> bool {
        let _ = path;
        todo!("match against union of patterns")
    }
}
