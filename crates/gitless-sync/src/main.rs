#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::process::ExitCode;

use clap::{ArgAction, Parser, Subcommand};

use gitless_sync::commands;
use gitless_sync::commands::scan::Backend;
use gitless_sync::commands::scan::args::{StatusFilter, collect_status_filter};
use gitless_sync::shared::error::GitlessError;
use gitless_sync::shared::gh::RealGhClient;

#[derive(Parser, Debug)]
#[command(
    name = "gitless-sync",
    version,
    about = "Read-only diff between a local directory and a GitHub repo (no git required)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// GitHub repository to compare against, in owner/name format.
    #[arg(long, global = true)]
    repo: Option<String>,

    /// Local directory to scan against the repo.
    #[arg(long, global = true, default_value = ".")]
    local: String,

    /// Ignore pattern using gitignore syntax (repeatable).
    #[arg(long, global = true)]
    ignore: Vec<String>,

    /// Preserve UTF-8 BOM when comparing text files.
    #[arg(long, global = true)]
    keep_bom: bool,

    /// Pretty-print JSON output (default is compact one-line).
    #[arg(long, global = true)]
    pretty: bool,

    /// GitHub API backend (graphql default, rest fallback).
    #[arg(long, global = true, value_enum, default_value_t = Backend::Graphql)]
    backend: Backend,

    #[arg(short = 'v', long = "verbose", global = true, action = ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Scan {
        /// Branch to read from the repo.
        #[arg(long, default_value = "main")]
        branch: String,

        /// Emit only the summary object, omit the files array.
        #[arg(long)]
        summary_only: bool,

        /// Comma-separated status filter (e.g. drift,local_only_changed).
        #[arg(long, value_enum, value_delimiter = ',')]
        status: Vec<StatusFilter>,
    },
    Diff {
        /// Branch to read from the repo.
        #[arg(long, default_value = "main")]
        branch: String,

        /// Relative path (forward slash) of the file to diff.
        path: String,

        /// Emit JSON output instead of unified text (opt-in).
        #[arg(long)]
        json: bool,
    },
    #[command(
        about = "Print a gitless-sync.toml template to stdout (you redirect to a file)",
        after_help = "Example:\n  gitless-sync init --repo owner/name --branch main > gitless-sync.toml"
    )]
    Init {
        /// Branch name to emit. Omit to leave the field out of the generated toml.
        #[arg(long)]
        branch: Option<String>,
    },
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => return handle_clap_parse_error(err),
    };
    let client = RealGhClient::new();

    let result = match cli.command {
        Commands::Scan {
            branch,
            summary_only,
            status,
        } => {
            let scan_args = commands::scan::ScanArgs {
                repo: cli.repo,
                branch,
                local: cli.local,
                ignore: cli.ignore,
                keep_bom: cli.keep_bom,
                pretty: cli.pretty,
                summary_only,
                status: collect_status_filter(status),
                backend: cli.backend,
                verbose: cli.verbose,
            };
            commands::scan::run_with_client(&scan_args, &client)
        }
        Commands::Diff { branch, path, json } => commands::diff::run_with_client(
            &commands::diff::DiffArgs {
                repo: cli.repo,
                branch,
                local: cli.local,
                keep_bom: cli.keep_bom,
                path,
                json,
            },
            &client,
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
        ),
        Commands::Init { branch } => {
            let init_args = commands::init::InitArgs {
                repo: cli.repo.unwrap_or_default(),
                branch,
                ignore: cli.ignore,
            };
            commands::init::run(
                &init_args,
                &mut std::io::stdout().lock(),
                &mut std::io::stderr().lock(),
            )
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!(
                "{}",
                serde_json::to_string(&err.to_stderr_payload()).unwrap_or_else(|_| err.to_string())
            );
            ExitCode::from(err.exit_code())
        }
    }
}

fn handle_clap_parse_error(err: clap::Error) -> ExitCode {
    match map_clap_parse_error(&err) {
        None => {
            let _ = err.print();
            ExitCode::SUCCESS
        }
        Some(gle) => {
            eprintln!(
                "{}",
                serde_json::to_string(&gle.to_stderr_payload())
                    .unwrap_or_else(|_| gle.to_string())
            );
            ExitCode::from(gle.exit_code())
        }
    }
}

// `--status drif` 같은 argument-parse 실패도 다른 CLI 에러와 같은 한 줄
// JSON contract (`{"error_code":"CONFIG_ERROR","message":"..."}`)로 떨어지도록
// clap 에러를 GitlessError::Config 로 wrap. message 에는 clap multi-line
// 출력을 그대로 (escape 후) 박아 valid 후보 리스트와 did-you-mean hint 까지
// caller 에게 전달한다 (spec-error-contracts.md § Custom Error Types). Display
// kind (`--help` / `--version` 등) 는 None 으로 분기해 clap 기본 stdout
// 출력 + exit 0 을 그대로 통과.
fn map_clap_parse_error(err: &clap::Error) -> Option<GitlessError> {
    use clap::error::ErrorKind;
    match err.kind() {
        ErrorKind::DisplayHelp
        | ErrorKind::DisplayVersion
        | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => None,
        _ => {
            let stripped = err
                .to_string()
                .trim_start_matches("error: ")
                .trim_end()
                .to_string();
            Some(GitlessError::Config(stripped))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn display_help_returns_none() {
        let err = Cli::try_parse_from(["gitless-sync", "--help"]).unwrap_err();
        assert!(map_clap_parse_error(&err).is_none());
    }

    #[test]
    fn display_version_returns_none() {
        let err = Cli::try_parse_from(["gitless-sync", "--version"]).unwrap_err();
        assert!(map_clap_parse_error(&err).is_none());
    }

    #[test]
    fn missing_subcommand_routes_to_clap_default_help() {
        // clap 4.6 + Subcommand-derived enum: a bare invocation triggers
        // DisplayHelpOnMissingArgumentOrSubcommand (auto-help fallback) →
        // mapped to None → caller hands back to clap's default printer
        // (stdout help + exit 0).
        let err = Cli::try_parse_from(["gitless-sync"]).unwrap_err();
        assert!(map_clap_parse_error(&err).is_none());
    }

    #[test]
    fn invalid_status_maps_to_config_with_candidates_and_did_you_mean() {
        let err =
            Cli::try_parse_from(["gitless-sync", "scan", "--status", "drif"]).unwrap_err();
        let mapped = map_clap_parse_error(&err).expect("non-display kind must map to Some");
        let GitlessError::Config(msg) = &mapped else {
            panic!("expected Config variant, got {mapped:?}");
        };
        assert!(msg.contains("drif"), "echo bad value: {msg}");
        for candidate in [
            "identical",
            "local_only_changed",
            "remote_only_changed",
            "drift",
            "failed",
        ] {
            assert!(msg.contains(candidate), "candidate {candidate} in: {msg}");
        }
        assert!(
            msg.contains("drift"),
            "did-you-mean line should suggest 'drift': {msg}"
        );
        assert_eq!(mapped.error_code(), "CONFIG_ERROR");
        assert_eq!(mapped.exit_code(), 1);
    }
}
