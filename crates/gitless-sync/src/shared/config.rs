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

/// Resolve the `--token` argument into the actual secret value.
///
/// - `env:NAME` reads the named environment variable.
/// - `literal:VALUE` returns `VALUE` verbatim.
/// - Anything else is returned as-is (covers tokens injected by clap's `env = "GITHUB_TOKEN"`).
///
/// # Errors
/// Returns [`GitlessError::AuthFailed`] when an `env:` reference points to an unset variable.
pub fn resolve_token(spec: &str) -> Result<String, GitlessError> {
    resolve_token_with(spec, |name| std::env::var(name).ok())
}

fn resolve_token_with<F>(spec: &str, env_lookup: F) -> Result<String, GitlessError>
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(name) = spec.strip_prefix("env:") {
        env_lookup(name).ok_or(GitlessError::AuthFailed)
    } else if let Some(value) = spec.strip_prefix("literal:") {
        Ok(value.to_string())
    } else {
        Ok(spec.to_string())
    }
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

    #[test]
    fn token_env_prefix_reads_named_variable() {
        let result = resolve_token_with("env:MY_TOKEN", |name| {
            assert_eq!(name, "MY_TOKEN");
            Some("ghp_test_value".to_string())
        });
        assert_eq!(result.unwrap(), "ghp_test_value");
    }

    #[test]
    fn token_env_prefix_returns_auth_failed_when_unset() {
        let err = resolve_token_with("env:MISSING_VAR", |_| None)
            .expect_err("missing env var should error");
        assert!(matches!(err, GitlessError::AuthFailed));
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn token_literal_prefix_returns_value_verbatim() {
        let result = resolve_token_with("literal:ghp_xxx", |_| {
            panic!("literal prefix must not consult env lookup")
        });
        assert_eq!(result.unwrap(), "ghp_xxx");
    }

    #[test]
    fn token_without_prefix_returned_as_is() {
        let result = resolve_token_with("ghp_raw_token", |_| {
            panic!("raw token must not consult env lookup")
        });
        assert_eq!(result.unwrap(), "ghp_raw_token");
    }

    #[test]
    fn token_literal_prefix_allows_empty_value() {
        let result = resolve_token_with("literal:", |_| {
            panic!("literal prefix must not consult env lookup")
        });
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn resolve_token_uses_real_env_when_var_missing() {
        let err = resolve_token("env:GITLESS_SYNC_DEFINITELY_NOT_SET_ZZZ_4242")
            .expect_err("var should not exist");
        assert!(matches!(err, GitlessError::AuthFailed));
    }
}
