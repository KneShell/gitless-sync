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

/// Run `scan` and write JSON to stdout. Inject `RealGhClient` (prod) or
/// `MockGhClient` (tests).
///
/// # Errors
/// `GitlessError` from config / GitHub API / local IO. `PartialFailure`
/// when files fail to hash. `Config` for JSON serialize failure (unreachable
/// for current schema).
pub fn run_with_client<C: GhClient + Sync>(
    args: &ScanArgs,
    client: &C,
) -> Result<(), GitlessError> {
    let (report, failed_count) = build_report(args, client)?;
    let json = output::serialize(&report, args.pretty)
        .map_err(|e| GitlessError::Config(format!("ScanReport JSON serialization failed: {e}")))?;
    println!("{json}");
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
        run_with_client(&args, &mock).unwrap();
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
        let err = run_with_client(&args, &mock).unwrap_err();
        assert!(matches!(err, GitlessError::TreesTruncated));
        assert_eq!(err.exit_code(), 5);
    }
}
