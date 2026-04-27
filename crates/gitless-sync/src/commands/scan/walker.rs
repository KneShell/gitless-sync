use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use crate::shared::error::GitlessError;
use crate::shared::ignore::IgnoreMatcher;

#[derive(Debug, Clone)]
pub struct LocalFile {
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub mtime: DateTime<Utc>,
}

pub fn walk(root: &Path, matcher: &IgnoreMatcher) -> Result<Vec<LocalFile>, GitlessError> {
    let _ = (root, matcher);
    todo!("walkdir + IgnoreMatcher; relative_path uses forward slash")
}
