#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::process::ExitCode;

use clap::{ArgAction, Parser, Subcommand};

use gitless_sync::commands;
use gitless_sync::commands::scan::Backend;
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

    /// Branch to read from the repo (defaults to `main`).
    #[arg(long, global = true)]
    branch: Option<String>,

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
        /// Emit only the summary object, omit the files array.
        #[arg(long)]
        summary_only: bool,

        /// Comma-separated status filter (e.g. `drift,local_only_changed`).
        #[arg(long)]
        status: Option<String>,
    },
    Diff {
        path: String,

        #[arg(long)]
        json: bool,
    },
    #[command(
        about = "Print a gitless-sync.toml template to stdout (you redirect to a file)",
        after_help = "Example:\n  gitless-sync init --repo owner/name --branch main > gitless-sync.toml"
    )]
    Init,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let client = RealGhClient::new();

    let result = match cli.command {
        Commands::Scan {
            summary_only,
            status,
        } => {
            let scan_args = commands::scan::ScanArgs {
                repo: cli.repo,
                branch: cli.branch.unwrap_or_else(|| "main".to_string()),
                local: cli.local,
                ignore: cli.ignore,
                keep_bom: cli.keep_bom,
                pretty: cli.pretty,
                summary_only,
                status,
                backend: cli.backend,
                verbose: cli.verbose,
            };
            commands::scan::run_with_client(&scan_args, &client)
        }
        Commands::Diff { path, json } => commands::diff::run_with_client(
            &commands::diff::DiffArgs {
                repo: cli.repo,
                branch: cli.branch.unwrap_or_else(|| "main".to_string()),
                local: cli.local,
                keep_bom: cli.keep_bom,
                path,
                json,
            },
            &client,
            &mut std::io::stdout().lock(),
            &mut std::io::stderr().lock(),
        ),
        Commands::Init => {
            let init_args = commands::init::InitArgs {
                repo: cli.repo.unwrap_or_default(),
                branch: cli.branch,
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
