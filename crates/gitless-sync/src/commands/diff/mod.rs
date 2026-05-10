//! `diff` slice — orchestrator entry point. Owns the public API
//! (`run_with_client`, `DiffArgs`); orchestration (`compute`), pure rendering
//! (`render`), and IO leaf (`io`) live in siblings. `args` holds the public
//! arg type to avoid a `mod.rs ↔ compute.rs` cycle.

mod args;
mod compute;
mod io;
mod render;

#[cfg(test)]
mod test_helpers;

use std::io::Write;

use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;

pub use self::args::DiffArgs;
use self::compute::compute_diff;

/// Run the `diff` command and write the unified diff (or a one-sided message)
/// to the provided `stdout` / `stderr` writers.
///
/// Production callers inject `RealGhClient` + `std::io::stdout().lock()` /
/// `std::io::stderr().lock()`; integration tests inject `MockGhClient` +
/// `Vec<u8>` writers to capture the output.
///
/// # Errors
/// Returns any [`GitlessError`] raised by config loading, GitHub API calls,
/// or local IO. Returns [`GitlessError::Config`] when the requested path
/// exists on neither side.
pub fn run_with_client<C: GhClient, W: Write, E: Write>(
    args: &DiffArgs,
    client: &C,
    stdout: &mut W,
    stderr: &mut E,
) -> Result<(), GitlessError> {
    let outcome = compute_diff(args, client)?;
    if !outcome.stderr_message.is_empty() {
        writeln!(stderr, "{}", outcome.stderr_message)?;
    }
    stdout.write_all(&outcome.stdout)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::shared::gh::MockGhClient;

    use super::test_helpers::{args_for, stub_tree};

    #[test]
    fn run_with_client_returns_ok_for_local_only_path() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("only.md"), "hi\n").unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            r#"{"sha":"x","tree":[],"truncated":false}"#,
        );

        let args = args_for(dir.path(), "only.md");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        run_with_client(&args, &mock, &mut stdout, &mut stderr).unwrap();
    }
}
