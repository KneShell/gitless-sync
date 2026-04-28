// TODO(v0.1 cleanup): remove once all module functions are wired up.
#![allow(dead_code, clippy::needless_pass_by_value)]

use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod commands;
mod shared;

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

    let result = match cli.command {
        Commands::Scan {
            summary_only,
            status,
        } => commands::scan::run(commands::scan::ScanArgs {
            repo: cli.repo,
            branch: cli.branch,
            local: cli.local,
            ignore: cli.ignore,
            token: cli.token,
            keep_bom: cli.keep_bom,
            pretty: cli.pretty,
            summary_only,
            status,
        }),
        Commands::Diff { path } => commands::diff::run(commands::diff::DiffArgs {
            repo: cli.repo,
            branch: cli.branch,
            local: cli.local,
            token: cli.token,
            keep_bom: cli.keep_bom,
            path,
        }),
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
