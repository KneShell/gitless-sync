#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::process::ExitCode;

use clap::{ArgAction, Parser, Subcommand};

use gitless_sync::commands;
use gitless_sync::commands::scan::Backend;
use gitless_sync::commands::scan::args::{StatusFilter, collect_status_filter};
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

    /// `GitHub` repository to compare against, in `owner/name` format.
    #[arg(long, global = true)]
    repo: Option<String>,

    /// Local directory to scan against the repo.
    #[arg(long, global = true, default_value = ".")]
    local: String,

    /// Ignore pattern using `gitignore` syntax (repeatable).
    #[arg(long, global = true)]
    ignore: Vec<String>,

    /// Preserve `UTF-8` BOM when comparing text files.
    #[arg(long, global = true)]
    keep_bom: bool,

    /// Pretty-print `JSON` output (default is compact one-line).
    #[arg(long, global = true)]
    pretty: bool,

    /// `GitHub` API backend (`graphql` default, `rest` fallback).
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

        /// Comma-separated status filter (e.g. `drift,local_only_changed`).
        #[arg(long, value_enum, value_delimiter = ',')]
        status: Vec<StatusFilter>,
    },
    Diff {
        /// Branch to read from the repo.
        #[arg(long, default_value = "main")]
        branch: String,

        /// Relative path (forward slash) of the file to diff.
        path: String,

        /// Emit `JSON` output instead of unified text (opt-in).
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
    let cli = Cli::parse();
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
