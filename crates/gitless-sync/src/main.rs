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

    #[arg(long, global = true)]
    repo: Option<String>,

    #[arg(long, global = true)]
    branch: Option<String>,

    #[arg(long, global = true, default_value = ".")]
    local: String,

    #[arg(long, global = true)]
    ignore: Vec<String>,

    #[arg(long, global = true)]
    keep_bom: bool,

    #[arg(long, global = true)]
    pretty: bool,

    #[arg(long, global = true, value_enum, default_value_t = Backend::Graphql)]
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
        Commands::Diff { path } => commands::diff::run_with_client(
            &commands::diff::DiffArgs {
                repo: cli.repo,
                branch: cli.branch.unwrap_or_else(|| "main".to_string()),
                local: cli.local,
                keep_bom: cli.keep_bom,
                path,
            },
            &client,
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
