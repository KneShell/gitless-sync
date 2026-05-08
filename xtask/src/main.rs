use std::env;
use std::process::ExitCode;

mod check_line_limits;

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
    println!("  check-line-limits  Check LOC <= 300 per file (warn stage)");
    println!();
    println!("Future commands (Phase 6):");
    println!("  check-cycles       Detect cycles via cargo-modules (task E)");
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
