//! `gitless-sync init` — emit a `gitless-sync.toml` body to a writer.
//!
//! The tool never creates files (ADR 0001 / ADR 0004 read-only). Callers
//! redirect stdout: `gitless-sync init --repo a/b > gitless-sync.toml`.

use std::io::Write;

use crate::shared::error::GitlessError;

/// Arguments parsed from the `init` subcommand.
#[derive(Debug)]
pub struct InitArgs {
    pub repo: String,
    pub branch: Option<String>,
    pub ignore: Vec<String>,
}

/// Emit a `gitless-sync.toml` body to `writer`.
///
/// Stable order: `repo` then `branch` then `ignore`. Optional fields are
/// emitted only when set / non-empty (`spec-cli-interface.md` § init).
///
/// # Errors
/// Propagates any [`std::io::Error`] from the writer as [`GitlessError::Io`].
/// Empty-`repo` validation is handled by P4, not here.
pub fn run<W: Write>(args: &InitArgs, writer: &mut W) -> Result<(), GitlessError> {
    writeln!(writer, "repo = \"{}\"", args.repo)?;
    if let Some(branch) = args.branch.as_deref() {
        writeln!(writer, "branch = \"{branch}\"")?;
    }
    if !args.ignore.is_empty() {
        let joined = args
            .ignore
            .iter()
            .map(|p| format!("\"{p}\""))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(writer, "ignore = [{joined}]")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::config::Config;

    fn emit(args: &InitArgs) -> String {
        let mut buf = Vec::new();
        run(args, &mut buf).expect("run should succeed for valid args");
        String::from_utf8(buf).expect("emitted bytes are utf-8")
    }

    #[test]
    fn emits_repo_only_when_branch_and_ignore_unset() {
        let out = emit(&InitArgs {
            repo: "a/b".to_string(),
            branch: None,
            ignore: vec![],
        });
        assert_eq!(out, "repo = \"a/b\"\n");
    }

    #[test]
    fn emits_repo_and_branch_in_order() {
        let out = emit(&InitArgs {
            repo: "a/b".to_string(),
            branch: Some("dev".to_string()),
            ignore: vec![],
        });
        assert_eq!(out, "repo = \"a/b\"\nbranch = \"dev\"\n");
    }

    #[test]
    fn emits_ignore_with_single_pattern() {
        let out = emit(&InitArgs {
            repo: "a/b".to_string(),
            branch: None,
            ignore: vec!["*.tmp".to_string()],
        });
        assert_eq!(out, "repo = \"a/b\"\nignore = [\"*.tmp\"]\n");
    }

    #[test]
    fn emits_ignore_with_multiple_patterns_comma_space_joined() {
        let out = emit(&InitArgs {
            repo: "a/b".to_string(),
            branch: None,
            ignore: vec!["dist/".to_string(), "*.tmp".to_string()],
        });
        assert_eq!(out, "repo = \"a/b\"\nignore = [\"dist/\", \"*.tmp\"]\n");
    }

    #[test]
    fn emits_all_fields_when_present() {
        let out = emit(&InitArgs {
            repo: "owner/name".to_string(),
            branch: Some("main".to_string()),
            ignore: vec![
                "dist/".to_string(),
                "*.tmp".to_string(),
                "node_modules/".to_string(),
            ],
        });
        assert_eq!(
            out,
            "repo = \"owner/name\"\nbranch = \"main\"\nignore = [\"dist/\", \"*.tmp\", \"node_modules/\"]\n"
        );
    }

    #[test]
    fn round_trips_full_payload_through_config_struct() {
        let args = InitArgs {
            repo: "owner/name".to_string(),
            branch: Some("dev".to_string()),
            ignore: vec!["dist/".to_string(), "*.tmp".to_string()],
        };
        let toml_text = emit(&args);
        let parsed: Config = toml::from_str(&toml_text).expect("emitted toml should parse");
        assert_eq!(parsed.repo.as_deref(), Some("owner/name"));
        assert_eq!(parsed.branch.as_deref(), Some("dev"));
        assert_eq!(
            parsed.ignore,
            vec!["dist/".to_string(), "*.tmp".to_string()]
        );
    }

    #[test]
    fn round_trips_repo_only_through_config_struct() {
        let args = InitArgs {
            repo: "a/b".to_string(),
            branch: None,
            ignore: vec![],
        };
        let toml_text = emit(&args);
        let parsed: Config = toml::from_str(&toml_text).expect("emitted toml should parse");
        assert_eq!(parsed.repo.as_deref(), Some("a/b"));
        assert_eq!(parsed.branch, None);
        assert!(parsed.ignore.is_empty());
    }
}
