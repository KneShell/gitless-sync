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

/// Hint always written to stderr after a successful emit. tty 감지 분기 0
/// (`spec-cli-interface.md` § stderr hint).
pub const STDERR_HINT: &str = "Tip: redirect stdout to ./gitless-sync.toml to persist this config.";

/// Emit a `gitless-sync.toml` body to `stdout` and a redirect hint to
/// `stderr`.
///
/// Stable order: `repo` then `branch` then `ignore`. Optional fields are
/// emitted only when set / non-empty (`spec-cli-interface.md` § init).
///
/// # Errors
/// - [`GitlessError::Config`] when `repo` is empty
///   (`spec-error-contracts.md` § init 에러 케이스). stdout / stderr 둘 다
///   이 경우 미emit.
/// - Propagates any [`std::io::Error`] from either writer as
///   [`GitlessError::Io`].
pub fn run<W: Write, E: Write>(
    args: &InitArgs,
    stdout: &mut W,
    stderr: &mut E,
) -> Result<(), GitlessError> {
    if args.repo.is_empty() {
        return Err(GitlessError::Config("repo not specified".to_string()));
    }
    writeln!(stdout, "repo = \"{}\"", args.repo)?;
    if let Some(branch) = args.branch.as_deref() {
        writeln!(stdout, "branch = \"{branch}\"")?;
    }
    if !args.ignore.is_empty() {
        let joined = args
            .ignore
            .iter()
            .map(|p| format!("\"{p}\""))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(stdout, "ignore = [{joined}]")?;
    }
    writeln!(stderr, "{STDERR_HINT}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::config::Config;

    fn emit(args: &InitArgs) -> (String, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        run(args, &mut stdout, &mut stderr).expect("run should succeed for valid args");
        (
            String::from_utf8(stdout).expect("emitted stdout bytes are utf-8"),
            String::from_utf8(stderr).expect("emitted stderr bytes are utf-8"),
        )
    }

    #[test]
    fn emits_repo_only_when_branch_and_ignore_unset() {
        let (out, _) = emit(&InitArgs {
            repo: "a/b".to_string(),
            branch: None,
            ignore: vec![],
        });
        assert_eq!(out, "repo = \"a/b\"\n");
    }

    #[test]
    fn emits_repo_and_branch_in_order() {
        let (out, _) = emit(&InitArgs {
            repo: "a/b".to_string(),
            branch: Some("dev".to_string()),
            ignore: vec![],
        });
        assert_eq!(out, "repo = \"a/b\"\nbranch = \"dev\"\n");
    }

    #[test]
    fn emits_ignore_with_single_pattern() {
        let (out, _) = emit(&InitArgs {
            repo: "a/b".to_string(),
            branch: None,
            ignore: vec!["*.tmp".to_string()],
        });
        assert_eq!(out, "repo = \"a/b\"\nignore = [\"*.tmp\"]\n");
    }

    #[test]
    fn emits_ignore_with_multiple_patterns_comma_space_joined() {
        let (out, _) = emit(&InitArgs {
            repo: "a/b".to_string(),
            branch: None,
            ignore: vec!["dist/".to_string(), "*.tmp".to_string()],
        });
        assert_eq!(out, "repo = \"a/b\"\nignore = [\"dist/\", \"*.tmp\"]\n");
    }

    #[test]
    fn emits_all_fields_when_present() {
        let (out, _) = emit(&InitArgs {
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
        let (toml_text, _) = emit(&args);
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
        let (toml_text, _) = emit(&args);
        let parsed: Config = toml::from_str(&toml_text).expect("emitted toml should parse");
        assert_eq!(parsed.repo.as_deref(), Some("a/b"));
        assert_eq!(parsed.branch, None);
        assert!(parsed.ignore.is_empty());
    }

    #[test]
    fn empty_repo_returns_config_error_with_exit_code_one() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let err = run(
            &InitArgs {
                repo: String::new(),
                branch: None,
                ignore: vec![],
            },
            &mut stdout,
            &mut stderr,
        )
        .expect_err("empty repo must error");
        assert!(matches!(err, GitlessError::Config(_)));
        assert_eq!(err.exit_code(), 1);
        let GitlessError::Config(msg) = &err else {
            panic!("expected Config variant, got {err:?}");
        };
        assert!(
            msg.contains("repo not specified"),
            "expected 'repo not specified' substring, got: {msg}"
        );
        assert!(stdout.is_empty(), "stdout must stay empty on error");
        assert!(stderr.is_empty(), "stderr hint must not emit on error path");
    }

    #[test]
    fn emits_redirect_hint_to_stderr_on_success() {
        let (_, hint) = emit(&InitArgs {
            repo: "a/b".to_string(),
            branch: None,
            ignore: vec![],
        });
        assert_eq!(hint, format!("{STDERR_HINT}\n"));
        assert!(
            hint.contains("redirect stdout"),
            "expected redirect hint, got: {hint}"
        );
        assert!(
            hint.contains("gitless-sync.toml"),
            "expected hint to mention target file, got: {hint}"
        );
    }
}
