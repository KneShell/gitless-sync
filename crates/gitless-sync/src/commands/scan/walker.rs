use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use walkdir::WalkDir;

use crate::shared::error::GitlessError;
use crate::shared::ignore::IgnoreMatcher;
use crate::shared::path::to_nfc;

#[derive(Debug, Clone)]
pub struct LocalFile {
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub mtime: DateTime<Utc>,
    /// `true` when this entry is a symbolic link (lstat-detected, target not
    /// followed). Phase 5 task H surfaces these so `pipeline.rs` can promote
    /// the path to `Status::Failed` + `failed_reason: "symlink"`
    /// (`spec-domain-pitfalls.md` § Symlink).
    pub is_symlink: bool,
}

/// Walk `root` and return every file that survives the ignore matcher.
///
/// `relative_path` is normalized to forward slashes (G-004) and then to
/// Unicode NFC so the comparison key matches GitHub's tree/blob paths even
/// when the local filesystem stores names in NFD (macOS APFS/HFS+). See
/// `spec-domain-pitfalls.md` § Path 정규화. Directories and ignored entries
/// are excluded; ignored directories are pruned via a probe path so we
/// don't descend into them (so `node_modules/` does not get walked).
/// Symlinks are surfaced with `is_symlink: true` so `pipeline.rs` can mark
/// them `Status::Failed` + `failed_reason: "symlink"` (Phase 5 task H —
/// `WalkDir::follow_links(false)` keeps lstat semantics, no link target
/// resolution).
///
/// # Errors
/// Returns [`GitlessError::Io`] when the underlying walk reports an I/O failure
/// or when a file's mtime cannot be read.
pub fn walk(root: &Path, matcher: &IgnoreMatcher) -> Result<Vec<LocalFile>, GitlessError> {
    let mut files = Vec::new();

    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            if entry.file_type().is_dir() {
                let Some(rel) = relative_path(root, entry.path()) else {
                    return true;
                };
                let probe = format!("{rel}/.probe");
                return !matcher.is_ignored(Path::new(&probe));
            }
            true
        });

    for entry in walker {
        let entry = entry.map_err(|e| walkdir_to_io(&e))?;

        let file_type = entry.file_type();
        let is_symlink = file_type.is_symlink();
        if !file_type.is_file() && !is_symlink {
            continue;
        }

        let Some(rel) = relative_path(root, entry.path()) else {
            continue;
        };

        if matcher.is_ignored(Path::new(&rel)) {
            continue;
        }

        let metadata = entry.metadata().map_err(|e| walkdir_to_io(&e))?;
        let modified = metadata.modified()?;
        let mtime: DateTime<Utc> = modified.into();

        files.push(LocalFile {
            relative_path: rel,
            absolute_path: entry.path().to_path_buf(),
            mtime,
            is_symlink,
        });
    }

    Ok(files)
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    Some(to_nfc(&rel.to_string_lossy().replace('\\', "/")))
}

fn walkdir_to_io(err: &walkdir::Error) -> GitlessError {
    GitlessError::Io(std::io::Error::other(err.to_string()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn matcher_for(dir: &TempDir) -> IgnoreMatcher {
        IgnoreMatcher::new(dir.path(), &[]).expect("build matcher")
    }

    fn names(files: &[LocalFile]) -> Vec<String> {
        files.iter().map(|f| f.relative_path.clone()).collect()
    }

    #[test]
    fn walks_files_in_tempdir() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "alpha").unwrap();
        fs::write(dir.path().join("b.md"), "bravo").unwrap();

        let files = walk(dir.path(), &matcher_for(&dir)).unwrap();
        let n = names(&files);

        assert_eq!(n.len(), 2);
        assert!(n.contains(&"a.txt".to_string()));
        assert!(n.contains(&"b.md".to_string()));
    }

    #[test]
    fn excludes_files_matched_by_custom_ignore() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("debug.log"), "x").unwrap();
        fs::write(dir.path().join("notes.md"), "y").unwrap();

        let matcher = IgnoreMatcher::new(dir.path(), &["*.log".to_string()]).unwrap();
        let files = walk(dir.path(), &matcher).unwrap();

        assert_eq!(names(&files), vec!["notes.md".to_string()]);
    }

    #[test]
    fn relative_paths_use_forward_slash() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("sub").join("nested")).unwrap();
        fs::write(dir.path().join("sub").join("nested").join("file.txt"), "hi").unwrap();

        let files = walk(dir.path(), &matcher_for(&dir)).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, "sub/nested/file.txt");
        assert!(!files[0].relative_path.contains('\\'));
    }

    #[test]
    fn skips_directory_excluded_by_builtin() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("node_modules").join("pkg")).unwrap();
        fs::write(
            dir.path().join("node_modules").join("pkg").join("foo.js"),
            "x",
        )
        .unwrap();
        fs::write(dir.path().join("keep.txt"), "y").unwrap();

        let files = walk(dir.path(), &matcher_for(&dir)).unwrap();
        assert_eq!(names(&files), vec!["keep.txt".to_string()]);
    }

    #[test]
    fn empty_directory_not_in_results() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("empty_dir")).unwrap();
        fs::write(dir.path().join("file.txt"), "x").unwrap();

        let files = walk(dir.path(), &matcher_for(&dir)).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, "file.txt");
    }

    #[test]
    fn mtime_is_recent_utc_datetime() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file.txt"), "x").unwrap();

        let files = walk(dir.path(), &matcher_for(&dir)).unwrap();
        assert_eq!(files.len(), 1);

        let delta = Utc::now().signed_duration_since(files[0].mtime);
        assert!(
            delta.num_seconds().abs() < 60,
            "mtime should be within a minute of now, got delta={}s",
            delta.num_seconds()
        );
    }

    #[test]
    fn applies_gitignore_from_root() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();
        fs::create_dir_all(dir.path().join("build")).unwrap();
        fs::write(dir.path().join("build").join("output.bin"), "x").unwrap();
        fs::write(dir.path().join("src.txt"), "y").unwrap();

        let matcher = IgnoreMatcher::new(dir.path(), &[]).unwrap();
        let files = walk(dir.path(), &matcher).unwrap();
        let n = names(&files);

        assert!(n.contains(&"src.txt".to_string()));
        assert!(n.contains(&".gitignore".to_string()));
        assert!(!n.iter().any(|p| p.starts_with("build/")));
    }

    #[test]
    fn absolute_path_is_set_to_real_file() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("here.txt");
        fs::write(&target, "x").unwrap();

        let files = walk(dir.path(), &matcher_for(&dir)).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].absolute_path, target);
    }

    #[test]
    fn nested_ignored_directory_is_pruned() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("src").join("target")).unwrap();
        fs::write(dir.path().join("src").join("main.rs"), "x").unwrap();
        fs::write(dir.path().join("src").join("target").join("artifact"), "y").unwrap();

        let files = walk(dir.path(), &matcher_for(&dir)).unwrap();
        let n = names(&files);
        assert!(n.contains(&"src/main.rs".to_string()));
        assert!(!n.iter().any(|p| p.contains("target/")));
    }

    #[cfg(unix)]
    #[test]
    fn captures_symlinks_on_unix_with_is_symlink_flag() {
        // Phase 5 task H: walker now emits symlink entries with
        // `is_symlink: true` so `pipeline.rs` can promote them to
        // `Status::Failed` + `failed_reason: "symlink"`. The link target is
        // not followed (`WalkDir::follow_links(false)`).
        use std::os::unix::fs::symlink;

        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("real.txt"), "x").unwrap();
        symlink(dir.path().join("real.txt"), dir.path().join("link.txt")).unwrap();

        let files = walk(dir.path(), &matcher_for(&dir)).unwrap();
        let n = names(&files);

        assert_eq!(n.len(), 2);
        assert!(n.contains(&"real.txt".to_string()));
        assert!(n.contains(&"link.txt".to_string()));

        let link = files
            .iter()
            .find(|f| f.relative_path == "link.txt")
            .unwrap();
        assert!(link.is_symlink, "symlink entry must carry is_symlink=true");

        let real = files
            .iter()
            .find(|f| f.relative_path == "real.txt")
            .unwrap();
        assert!(!real.is_symlink, "regular file must carry is_symlink=false");
    }

    #[test]
    fn relative_path_normalizes_nfd_to_nfc() {
        // Direct check on the helper: filesystems on macOS may yield NFD
        // bytes for the same logical name. We can't reliably create an NFD-
        // named file on every test runner, so this asserts the helper
        // produces NFC keys when given an NFD synthetic path.
        let root = Path::new("/root");
        let nfd_path = Path::new("/root/\u{1100}\u{1161}.txt");
        let key = relative_path(root, nfd_path).unwrap();
        assert_eq!(key, "가.txt");
        assert_eq!(key, "\u{AC00}.txt");
    }

    #[test]
    fn relative_path_nfc_input_is_preserved() {
        let root = Path::new("/root");
        let nfc_path = Path::new("/root/가.txt");
        let key = relative_path(root, nfc_path).unwrap();
        assert_eq!(key, "가.txt");
    }
}
