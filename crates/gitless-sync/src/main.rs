// TODO(v0.1 cleanup): remove once all module functions are wired up.
#![allow(dead_code, clippy::needless_pass_by_value)]

use std::process::ExitCode;

use clap::{ArgAction, Parser, Subcommand};

mod commands;
mod shared;

use commands::scan::{Backend, github::GITHUB_API_BASE};

/// Resolve the GitHub API base URL.
///
/// Honors `GITLESS_API_BASE` so integration tests can point the binary at a
/// mockito server. Production usage leaves the variable unset, falling back to
/// [`GITHUB_API_BASE`].
fn resolve_api_base() -> String {
    resolve_api_base_with(|name| std::env::var(name).ok())
}

fn resolve_api_base_with<F>(env_lookup: F) -> String
where
    F: Fn(&str) -> Option<String>,
{
    env_lookup("GITLESS_API_BASE").unwrap_or_else(|| GITHUB_API_BASE.to_string())
}

#[derive(Parser, Debug)]
#[command(
    name = "gitless-sync",
    version,
    about = "Read-only diff between a local directory and a GitHub repo (no git required)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, global = true)]
    repo: Option<String>,

    #[arg(long, global = true, default_value = "main")]
    branch: String,

    #[arg(long, global = true, default_value = ".")]
    local: String,

    #[arg(long, global = true)]
    ignore: Vec<String>,

    #[arg(long, global = true, env = "GITHUB_TOKEN")]
    token: Option<String>,

    #[arg(long, global = true)]
    keep_bom: bool,

    #[arg(long, global = true)]
    pretty: bool,

    #[arg(long, global = true, value_enum, default_value_t = Backend::Rest)]
    backend: Backend,

    #[arg(short = 'v', long = "verbose", global = true, action = ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Scan {
        #[arg(long)]
        summary_only: bool,

        #[arg(long)]
        status: Option<String>,
    },
    Diff {
        path: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let api_base = resolve_api_base();

    let result = match cli.command {
        Commands::Scan {
            summary_only,
            status,
        } => commands::scan::run_with_base(
            commands::scan::ScanArgs {
                repo: cli.repo,
                branch: cli.branch,
                local: cli.local,
                ignore: cli.ignore,
                token: cli.token,
                keep_bom: cli.keep_bom,
                pretty: cli.pretty,
                summary_only,
                status,
                backend: cli.backend,
                verbose: cli.verbose,
            },
            &api_base,
        ),
        Commands::Diff { path } => commands::diff::run_with_base(
            commands::diff::DiffArgs {
                repo: cli.repo,
                branch: cli.branch,
                local: cli.local,
                token: cli.token,
                keep_bom: cli.keep_bom,
                path,
            },
            &api_base,
        ),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_api_base_with_returns_env_value_when_set() {
        let got = resolve_api_base_with(|name| {
            assert_eq!(name, "GITLESS_API_BASE");
            Some("http://mock.example".to_string())
        });
        assert_eq!(got, "http://mock.example");
    }

    #[test]
    fn resolve_api_base_with_falls_back_to_github_default_when_absent() {
        let got = resolve_api_base_with(|_| None);
        assert_eq!(got, GITHUB_API_BASE);
    }
}
