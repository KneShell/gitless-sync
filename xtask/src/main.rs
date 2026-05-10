#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::env;
use std::process::ExitCode;

mod check_cycles;
mod check_line_limits;
mod check_readme_examples;
mod synth_vault;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    ExitCode::from(run(&args))
}

fn run(args: &[String]) -> u8 {
    match args.first().map(String::as_str) {
        None | Some("--help" | "-h" | "help") => {
            print_help();
            0
        }
        Some("check-line-limits") => match check_line_limits::run_default() {
            Ok(code) => code,
            Err(err) => {
                eprintln!("xtask check-line-limits: {err}");
                1
            }
        },
        Some("check-cycles") => match check_cycles::run() {
            Ok(code) => code,
            Err(err) => {
                eprintln!("xtask check-cycles: {err}");
                1
            }
        },
        Some("synth-vault") => match synth_vault::run(&args[1..]) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("xtask synth-vault: {err}");
                1
            }
        },
        Some("check-readme-examples") => match check_readme_examples::run() {
            Ok(code) => code,
            Err(err) => {
                eprintln!("xtask check-readme-examples: {err}");
                1
            }
        },
        Some(cmd) => {
            eprintln!("xtask: unknown command '{cmd}'");
            print_help();
            1
        }
    }
}

fn print_help() {
    println!("Usage: cargo xtask <command>");
    println!();
    println!("Commands:");
    println!("  help               Show this help message");
    println!("  check-line-limits  Check LOC <= 300 per file (deny stage)");
    println!("  check-cycles       Detect module cycles + cross-slice refs (deny stage)");
    println!(
        "  synth-vault        Generate synthetic markdown vault (--out PATH [--count N] [--seed N])"
    );
    println!("  check-readme-examples  Run Quick Start `init` line(s) from README.md (deny stage)");
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn run_with_no_args_returns_zero() {
        assert_eq!(run(&[]), 0);
    }

    #[test]
    fn run_with_long_help_flag_returns_zero() {
        assert_eq!(run(&["--help".to_string()]), 0);
    }

    #[test]
    fn run_with_short_help_flag_returns_zero() {
        assert_eq!(run(&["-h".to_string()]), 0);
    }

    #[test]
    fn run_with_help_subcommand_returns_zero() {
        assert_eq!(run(&["help".to_string()]), 0);
    }

    #[test]
    fn run_with_unknown_command_returns_one() {
        assert_eq!(run(&["bogus".to_string()]), 1);
    }
}
