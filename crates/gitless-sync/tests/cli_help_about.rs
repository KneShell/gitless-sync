#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Regression coverage for cli-ux-feedback.md F1 (scan/diff `about`
//! description) and F2 (init `about` wording precision). Verifies the
//! substrings asserted in spec-cli-interface.md § Acceptance Criteria
//! actually surface in the rendered `--help` output.

use std::process::Command;

fn help_stdout(args: &[&str]) -> String {
    let binary = env!("CARGO_BIN_EXE_gitless-sync");
    let output = Command::new(binary)
        .args(args)
        .output()
        .expect("spawn gitless-sync binary");
    assert!(
        output.status.success(),
        "--help should exit 0, got status {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("--help stdout is utf-8")
}

#[test]
fn top_level_help_lists_subcommand_about_descriptions() {
    let stdout = help_stdout(&["--help"]);
    for expected in [
        // F1: scan
        "Compare local directory against remote repo, emit 4-state classification JSON",
        // F1: diff
        "Show unified diff (or JSON) of a single file vs remote",
        // F2: init (top-level listing mirrors the subcommand-level about)
        "Emit gitless-sync.toml body from input args (stdout)",
    ] {
        assert!(
            stdout.contains(expected),
            "expected `{expected}` in `--help` stdout, got:\n{stdout}"
        );
    }
}

#[test]
fn init_subcommand_help_first_line_matches_f2_wording() {
    let stdout = help_stdout(&["init", "--help"]);
    assert!(
        stdout.contains("Emit gitless-sync.toml body from input args (stdout)"),
        "expected F2 wording in `init --help` stdout, got:\n{stdout}"
    );
}
