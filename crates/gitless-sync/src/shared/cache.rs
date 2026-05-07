//! Local SHA mtime cache (Phase 4, ADR 0009).
//!
//! Stores `<path> -> (mtime, self-hash)` per repo+branch under the OS user
//! cache directory (`dirs::cache_dir()/gitless-sync/`). Read-only contract is
//! preserved — this is internal metadata, not user data (ADR 0009 § Decision).
//!
//! Failure modes are all graceful: missing / corrupt / version-mismatched
//! cache files are reset to default and the scan proceeds with the same
//! timing as a cold run.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::shared::error::GitlessError;

/// Schema version. Bump on any breaking layout change — old cache files are
/// then reset to default on load.
const CACHE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Cache {
    pub(crate) version: u32,
    pub(crate) entries: HashMap<String, CacheEntry>,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION,
            entries: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CacheEntry {
    pub(crate) mtime: DateTime<Utc>,
    pub(crate) sha: String,
    #[serde(default)]
    pub(crate) is_binary: bool,
}

impl Cache {
    pub(crate) fn lookup(&self, path: &str, mtime: DateTime<Utc>) -> Option<(&str, bool)> {
        self.entries.get(path).and_then(|e| {
            if e.mtime == mtime {
                Some((e.sha.as_str(), e.is_binary))
            } else {
                None
            }
        })
    }

    pub(crate) fn insert(
        &mut self,
        path: String,
        mtime: DateTime<Utc>,
        sha: String,
        is_binary: bool,
    ) {
        self.entries.insert(
            path,
            CacheEntry {
                mtime,
                sha,
                is_binary,
            },
        );
    }

    /// Atomic write: serialize JSON to `<path>.tmp`, then rename onto `path`.
    /// Creates the parent directory if missing.
    pub(crate) fn save(&self, path: &Path) -> Result<(), GitlessError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| GitlessError::Config(format!("cache serialize: {e}")))?;
        let tmp = tmp_sibling(path);
        fs::write(&tmp, &bytes)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}

/// Read + parse `path`. Any failure (missing, IO error, corrupt JSON, version
/// mismatch) returns `Cache::default()` so the caller can keep going. Non-
/// missing failures emit a single stderr warning so an operator can notice.
pub(crate) fn load(path: &Path) -> Cache {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == ErrorKind::NotFound => return Cache::default(),
        Err(e) => {
            eprintln!("warning: cache reset: read failed: {e}");
            return Cache::default();
        }
    };
    let parsed: Cache = match serde_json::from_slice(&bytes) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warning: cache reset: parse failed: {e}");
            return Cache::default();
        }
    };
    if parsed.version != CACHE_VERSION {
        eprintln!(
            "warning: cache reset: version mismatch (expected {CACHE_VERSION}, got {})",
            parsed.version
        );
        return Cache::default();
    }
    parsed
}

/// Resolve the cache file path for `repo` (`owner/name`) + `branch`.
///
/// Layout: `<dirs::cache_dir()>/gitless-sync/<sanitized_repo>__<sanitized_branch>.json`.
/// Sanitization replaces `/` with `__` and Windows-reserved characters with
/// `_` so the result is a valid filename on every supported OS.
///
/// # Errors
/// Returns [`GitlessError::Config`] when the OS user-cache directory is
/// unavailable (extremely rare; e.g. a stripped-down environment with no
/// `HOME`/`LOCALAPPDATA`). Caller should fall back to running without cache.
pub(crate) fn cache_path(repo: &str, branch: &str) -> Result<PathBuf, GitlessError> {
    let base = dirs::cache_dir()
        .ok_or_else(|| GitlessError::Config("user cache directory unavailable".to_string()))?;
    let filename = format!(
        "{}__{}.json",
        sanitize_component(repo),
        sanitize_component(branch)
    );
    Ok(base.join("gitless-sync").join(filename))
}

fn sanitize_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '/' {
            out.push_str("__");
        } else if matches!(c, '<' | '>' | ':' | '"' | '\\' | '|' | '?' | '*') {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    out
}

fn tmp_sibling(path: &Path) -> PathBuf {
    let mut s: OsString = path.as_os_str().to_owned();
    s.push(".tmp");
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::TimeZone;
    use tempfile::TempDir;

    use super::*;

    fn ts(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    #[test]
    fn lookup_hits_when_mtime_matches() {
        let mut c = Cache::default();
        c.insert("a.md".to_string(), ts(100), "sha-a".to_string(), false);
        let hit = c.lookup("a.md", ts(100));
        assert_eq!(hit, Some(("sha-a", false)));
    }

    #[test]
    fn lookup_misses_on_unknown_path() {
        let c = Cache::default();
        assert!(c.lookup("ghost.md", ts(100)).is_none());
    }

    #[test]
    fn lookup_invalidates_on_mtime_change() {
        let mut c = Cache::default();
        c.insert("a.md".to_string(), ts(100), "sha-a".to_string(), false);
        assert!(c.lookup("a.md", ts(200)).is_none());
    }

    #[test]
    fn insert_preserves_is_binary_flag() {
        let mut c = Cache::default();
        c.insert("bin".to_string(), ts(50), "sha-bin".to_string(), true);
        assert_eq!(c.lookup("bin", ts(50)), Some(("sha-bin", true)));
    }

    #[test]
    fn load_missing_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("absent.json");
        let c = load(&path);
        assert_eq!(c.version, CACHE_VERSION);
        assert!(c.entries.is_empty());
    }

    #[test]
    fn load_corrupt_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("corrupt.json");
        fs::write(&path, b"not json at all").unwrap();
        let c = load(&path);
        assert_eq!(c.version, CACHE_VERSION);
        assert!(c.entries.is_empty());
    }

    #[test]
    fn load_version_mismatch_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("future.json");
        fs::write(&path, br#"{"version":999,"entries":{}}"#).unwrap();
        let c = load(&path);
        assert_eq!(c.version, CACHE_VERSION);
        assert!(c.entries.is_empty());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("c.json");

        let mut c = Cache::default();
        c.insert("a.md".to_string(), ts(100), "sha-a".to_string(), false);
        c.insert("b.md".to_string(), ts(200), "sha-b".to_string(), true);
        c.save(&path).unwrap();

        let loaded = load(&path);
        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.lookup("a.md", ts(100)), Some(("sha-a", false)));
        assert_eq!(loaded.lookup("b.md", ts(200)), Some(("sha-b", true)));
    }

    #[test]
    fn save_creates_missing_parent_directory() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("nested").join("dir").join("c.json");
        assert!(!nested.parent().unwrap().exists());

        let c = Cache::default();
        c.save(&nested).unwrap();
        assert!(nested.exists());
    }

    #[test]
    fn save_atomic_does_not_leave_tmp_artifact() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("c.json");
        let c = Cache::default();
        c.save(&path).unwrap();

        let tmp = tmp_sibling(&path);
        assert!(!tmp.exists(), "tmp file should be renamed away");
        assert!(path.exists());
    }

    #[test]
    fn save_replaces_existing_file_without_corruption() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("c.json");

        let mut first = Cache::default();
        first.insert("a.md".to_string(), ts(1), "sha-1".to_string(), false);
        first.save(&path).unwrap();

        let mut second = Cache::default();
        second.insert("b.md".to_string(), ts(2), "sha-2".to_string(), false);
        second.save(&path).unwrap();

        let loaded = load(&path);
        assert!(loaded.entries.contains_key("b.md"));
        assert!(!loaded.entries.contains_key("a.md"));
    }

    #[test]
    fn cache_path_sanitizes_owner_repo_branch() {
        let p = cache_path("KneShell/gitless-sync", "main").unwrap();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(name, "KneShell__gitless-sync__main.json");
        // Parent must be inside an OS user-cache directory, never the repo or
        // working directory (ADR 0009 § Decision §1).
        let parent = p.parent().unwrap();
        assert_eq!(parent.file_name().unwrap(), "gitless-sync");
    }

    #[test]
    fn cache_path_sanitizes_windows_reserved_chars() {
        let p = cache_path("o/r", "feature:colon").unwrap();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(name, "o__r__feature_colon.json");
    }

    #[test]
    fn sanitize_component_handles_mixed_specials() {
        assert_eq!(sanitize_component("a/b"), "a__b");
        // 8 reserved chars → 8 underscores.
        assert_eq!(sanitize_component(r#"<>:"\|?*"#), "_".repeat(8));
        assert_eq!(sanitize_component("plain"), "plain");
        // Non-ASCII (e.g. Korean) is preserved.
        assert_eq!(sanitize_component("한글"), "한글");
    }
}
