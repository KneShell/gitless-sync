//! `scan` slice — orchestrator entry point. Owns the public API
//! (`run_with_client`, `build_report`, `Backend` / `ScanArgs` re-exports).
//! Heavy lifting lives in sibling files (`build.rs`, `pipeline/`, etc.) so
//! this module stays a thin re-export + dispatch hub under the 300 LOC gate.

pub mod args;
pub mod build;
pub mod case_collision;
pub mod commits;
pub mod compare;
pub mod graphql;
pub mod hash_local;
pub mod hash_remote;
pub mod lfs;
pub mod long_path;
pub mod nfd_collision;
pub mod output;
pub mod pipeline;
pub mod renames;
pub mod summary_view;
pub mod walker;

#[cfg(test)]
mod status_filter;
#[cfg(test)]
mod test_helpers;

pub use self::args::{Backend, ScanArgs};
pub use self::build::build_report;
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;

/// Run `scan` and write JSON to the provided `stdout` writer. Inject
/// `RealGhClient` + `std::io::stdout().lock()` (prod) or `MockGhClient` +
/// a `Vec<u8>` writer (tests).
///
/// Writing via an injected writer (mirroring the `diff` / `init` slices) lets
/// a closed downstream pipe surface as `GitlessError::BrokenPipe` through `?`
/// instead of panicking inside `println!` (issue #25).
///
/// # Errors
/// `GitlessError` from config / GitHub API / local IO. `BrokenPipe` when the
/// consumer closes stdout early. `PartialFailure` when files fail to hash.
/// `Config` for JSON serialize failure (unreachable for current schema).
pub fn run_with_client<C: GhClient + Sync, W: std::io::Write>(
    args: &ScanArgs,
    client: &C,
    stdout: &mut W,
) -> Result<(), GitlessError> {
    let (report, failed_count) = build_report(args, client)?;
    let json = output::serialize(&report, args.pretty)
        .map_err(|e| GitlessError::Config(format!("ScanReport JSON serialization failed: {e}")))?;
    writeln!(stdout, "{json}")?;
    if failed_count > 0 {
        return Err(GitlessError::PartialFailure { failed_count });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::test_helpers::{args_for, stub_tree, stub_truncated_fallback_chain};
    use super::*;
    use crate::shared::gh::MockGhClient;
    use tempfile::TempDir;

    #[test]
    fn run_with_client_returns_partial_failure_exit_code_for_partial_failure_variant() {
        let err = GitlessError::PartialFailure { failed_count: 2 };
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn run_with_client_uses_rest_backend_by_default() {
        let dir = TempDir::new().unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            r#"{"sha":"x","tree":[],"truncated":false}"#,
        );
        let args = args_for(dir.path(), Some("o/r"));
        run_with_client(&args, &mock, &mut Vec::new()).unwrap();
    }

    #[test]
    fn run_with_client_propagates_truncated_from_mock() {
        let dir = TempDir::new().unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            r#"{"sha":"x","tree":[],"truncated":true}"#,
        );
        stub_truncated_fallback_chain(&mut mock, "o/r", "main");
        let args = args_for(dir.path(), Some("o/r"));
        let err = run_with_client(&args, &mock, &mut Vec::new()).unwrap_err();
        assert!(matches!(err, GitlessError::TreesTruncated));
        assert_eq!(err.exit_code(), 5);
    }

    /// A writer whose every `write` fails with `BrokenPipe`, emulating a
    /// downstream consumer (`| head`) that closed stdout early.
    struct BrokenWriter;

    impl std::io::Write for BrokenWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    // issue #25: writing into a closed pipe must surface as
    // `GitlessError::BrokenPipe` (exit 0), NOT panic inside the write.
    #[test]
    fn run_with_client_maps_broken_pipe_writer_to_broken_pipe_error() {
        let dir = TempDir::new().unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            r#"{"sha":"x","tree":[],"truncated":false}"#,
        );
        let args = args_for(dir.path(), Some("o/r"));
        let err = run_with_client(&args, &mock, &mut BrokenWriter).unwrap_err();
        assert!(
            matches!(err, GitlessError::BrokenPipe),
            "broken-pipe write must map to GitlessError::BrokenPipe, got {err:?}"
        );
        assert_eq!(err.exit_code(), 0);
    }

    // A normal writer captures the serialized report: valid JSON + trailing newline.
    #[test]
    fn run_with_client_writes_valid_json_line_to_writer() {
        let dir = TempDir::new().unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            r#"{"sha":"x","tree":[],"truncated":false}"#,
        );
        let args = args_for(dir.path(), Some("o/r"));
        let mut out = Vec::new();
        run_with_client(&args, &mock, &mut out).unwrap();
        assert!(out.ends_with(b"\n"), "output must end with a newline");
        let parsed: serde_json::Value =
            serde_json::from_slice(&out).expect("scan output must be valid JSON");
        assert_eq!(parsed["repo"], "o/r");
    }
}
