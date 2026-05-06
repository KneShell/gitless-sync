use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::shared::error::GitlessError;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub repo: Option<String>,
    pub branch: Option<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
}

/// Load a `gitless-sync.toml` config file.
///
/// Returns [`Config::default`] when `path` is `None` or refers to a non-existent file.
/// Returns [`GitlessError::Config`] when the file exists but cannot be read or parsed.
///
/// # Errors
/// Returns [`GitlessError::Config`] for I/O failures on an existing file or for TOML parse errors.
pub fn load(path: Option<&Path>) -> Result<Config, GitlessError> {
    let Some(path) = path else {
        return Ok(Config::default());
    };
    if !path.is_file() {
        return Ok(Config::default());
    }
    let text = fs::read_to_string(path)
        .map_err(|e| GitlessError::Config(format!("read {}: {e}", path.display())))?;
    toml::from_str::<Config>(&text)
        .map_err(|e| GitlessError::Config(format!("parse {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn load_returns_default_when_path_is_none() {
        let cfg = load(None).expect("None should yield default");
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn load_returns_default_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nope.toml");
        let cfg = load(Some(&missing)).expect("missing file should yield default");
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn load_parses_valid_toml() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("gitless-sync.toml");
        fs::write(
            &path,
            "repo = \"owner/name\"\nbranch = \"dev\"\nignore = [\"dist/\", \"*.tmp\"]\n",
        )
        .unwrap();
        let cfg = load(Some(&path)).expect("valid toml should parse");
        assert_eq!(cfg.repo.as_deref(), Some("owner/name"));
        assert_eq!(cfg.branch.as_deref(), Some("dev"));
        assert_eq!(cfg.ignore, vec!["dist/".to_string(), "*.tmp".to_string()]);
    }

    #[test]
    fn load_treats_empty_toml_as_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("gitless-sync.toml");
        fs::write(&path, "").unwrap();
        let cfg = load(Some(&path)).expect("empty toml should parse");
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn load_omits_optional_fields() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("gitless-sync.toml");
        fs::write(&path, "branch = \"dev\"\n").unwrap();
        let cfg = load(Some(&path)).expect("partial toml should parse");
        assert_eq!(cfg.repo, None);
        assert_eq!(cfg.branch.as_deref(), Some("dev"));
        assert!(cfg.ignore.is_empty());
    }

    #[test]
    fn load_returns_config_error_on_invalid_toml() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("gitless-sync.toml");
        fs::write(&path, "this is = not valid = toml ===\n").unwrap();
        let err = load(Some(&path)).expect_err("invalid toml should error");
        assert!(matches!(err, GitlessError::Config(_)));
        assert_eq!(err.exit_code(), 1);
    }
}
