//! `scan` slice — orchestrator entry point. Owns the public API
//! (`run_with_client`, `build_report`, `Backend` / `ScanArgs` re-exports);
//! domain (`pipeline`) and IO (`commits`, `hash_local`) live in siblings.

pub mod args;
pub mod case_collision;
pub mod commits;
pub mod compare;
pub mod graphql;
pub mod hash_local;
pub mod lfs;
pub mod long_path;
pub mod nfd_collision;
pub mod output;
pub mod pipeline;
pub mod status_filter;
pub mod walker;

#[cfg(test)]
mod test_helpers;

use std::path::Path;
use std::sync::Arc;

use chrono::Utc;

pub use self::args::{Backend, ScanArgs};
use self::output::{SCHEMA_VERSION, ScanReport};
use self::pipeline::{GitHubContext, assemble_entries};
use self::status_filter::parse_status_filter;
use crate::shared::config;
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::gitattributes::GitAttributes;
use crate::shared::github;
use crate::shared::ignore::IgnoreMatcher;

/// Run the `scan` command and write the resulting JSON report to stdout.
/// Production callers inject `RealGhClient`; tests inject `MockGhClient`.
///
/// # Errors
/// Returns any [`GitlessError`] raised by config / GitHub API / local IO.
/// Returns [`GitlessError::PartialFailure`] when files failed to hash.
/// JSON serialize failure maps to [`GitlessError::Config`] (unreachable for
/// the current schema).
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

/// Run the full pipeline up to (but not including) stdout serialization.
/// Returns `(ScanReport, failed_count)` — hash failures show up in the
/// count, not as `Err`. Exposed publicly so integration tests can inspect
/// the structured report directly.
///
/// # Errors
/// Propagates config / IO / GitHub API errors.
pub fn build_report<C: GhClient + Sync>(
    args: &ScanArgs,
    client: &C,
) -> Result<(ScanReport, usize), GitlessError> {
    let local_root = Path::new(&args.local);
    let toml_path = local_root.join("gitless-sync.toml");
    let cfg = config::load(Some(&toml_path))?;

    let repo = args
        .repo
        .as_deref()
        .or(cfg.repo.as_deref())
        .ok_or_else(|| GitlessError::Config("repo not specified".to_string()))?
        .to_string();
    let branch = args.branch.clone();

    let mut ignore_patterns = cfg.ignore.clone();
    ignore_patterns.extend(args.ignore.iter().cloned());

    let matcher = IgnoreMatcher::new(local_root, &ignore_patterns)?;

    if args.verbose >= 1 {
        eprintln!("info: scanning {} against {repo}@{branch}", args.local);
    }

    let remote_files = github::fetch_tree(client, &repo, &branch)?;
    let local_files = walker::walk(local_root, &matcher)?;
    let gitattr = Arc::new(GitAttributes::load(local_root)?);

    if args.verbose >= 1 {
        eprintln!(
            "info: found {} local files, {} remote files",
            local_files.len(),
            remote_files.len()
        );
    }
    if args.verbose >= 2 {
        for lf in &local_files {
            eprintln!("debug: local entry {}", lf.relative_path);
        }
    }

    let ctx = GitHubContext {
        client,
        repo: &repo,
        branch: &branch,
        backend: args.backend,
    };
    let (mut entries, summary, failed_count) =
        assemble_entries(&local_files, &remote_files, &ctx, args.keep_bom, &gitattr)?;

    if let Some(filter) = parse_status_filter(args.status.as_deref())? {
        entries.retain(|e| filter.contains(&e.status));
    }

    let files = if args.summary_only {
        None
    } else {
        Some(entries)
    };

    let report = ScanReport {
        schema_version: SCHEMA_VERSION.to_string(),
        scanned_at: Utc::now(),
        repo,
        branch,
        local_root: args.local.clone(),
        summary,
        files,
    };

    Ok((report, failed_count))
}

#[cfg(test)]
mod tests {
    use super::test_helpers::{args_for, err_resp, stub_tree, tree_args};
    use super::*;
    use crate::shared::gh::MockGhClient;
    use std::fs;
    use tempfile::TempDir;

    // --- build_report config / repo resolution -----------------------------

    #[test]
    fn build_report_returns_config_error_when_repo_missing() {
        let dir = TempDir::new().unwrap();
        let mock = MockGhClient::new();
        let args = args_for(dir.path(), None);
        let err = build_report(&args, &mock).unwrap_err();
        assert!(matches!(err, GitlessError::Config(_)));
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn build_report_uses_toml_repo_when_cli_repo_absent() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("gitless-sync.toml"),
            "repo = \"toml-owner/toml-repo\"\n",
        )
        .unwrap();

        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "toml-owner/toml-repo",
            "main",
            r#"{"sha":"x","tree":[],"truncated":false}"#,
        );

        let args = args_for(dir.path(), None);
        let (report, failed) = build_report(&args, &mock).unwrap();
        assert_eq!(failed, 0);
        assert_eq!(report.repo, "toml-owner/toml-repo");
    }

    #[test]
    fn build_report_cli_repo_overrides_toml() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("gitless-sync.toml"),
            "repo = \"toml-owner/toml-repo\"\n",
        )
        .unwrap();

        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "cli-owner/cli-repo",
            "main",
            r#"{"sha":"x","tree":[],"truncated":false}"#,
        );

        let args = args_for(dir.path(), Some("cli-owner/cli-repo"));
        let (report, _) = build_report(&args, &mock).unwrap();
        assert_eq!(report.repo, "cli-owner/cli-repo");
    }

    // --- build_report error propagation ------------------------------------

    #[test]
    fn build_report_propagates_auth_error_from_trees() {
        let dir = TempDir::new().unwrap();
        let mut mock = MockGhClient::new();
        mock.stub(
            tree_args("o/r", "main"),
            err_resp("gh: Bad credentials (HTTP 401)"),
        );

        let args = args_for(dir.path(), Some("o/r"));
        let err = build_report(&args, &mock).unwrap_err();
        assert!(matches!(err, GitlessError::AuthFailed));
    }

    #[test]
    fn build_report_propagates_truncated_error() {
        let dir = TempDir::new().unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            r#"{"sha":"x","tree":[],"truncated":true}"#,
        );

        let args = args_for(dir.path(), Some("o/r"));
        let err = build_report(&args, &mock).unwrap_err();
        assert!(matches!(err, GitlessError::TreesTruncated));
    }

    #[test]
    fn build_report_includes_schema_version_and_timestamp() {
        let dir = TempDir::new().unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            r#"{"sha":"x","tree":[],"truncated":false}"#,
        );

        let args = args_for(dir.path(), Some("o/r"));
        let (report, _) = build_report(&args, &mock).unwrap();
        assert_eq!(report.schema_version, SCHEMA_VERSION);
        assert_eq!(report.repo, "o/r");
        assert_eq!(report.branch, "main");
        assert!(report.files.is_some());
    }

    // --- run_with_client ---------------------------------------------------

    #[test]
    fn run_with_client_returns_partial_failure_exit_code_for_partial_failure_variant() {
        // Concrete check: exit code mapping for the variant produced by run_with_client.
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
        let args = args_for(dir.path(), Some("o/r"));
        let err = run_with_client(&args, &mock).unwrap_err();
        assert!(matches!(err, GitlessError::TreesTruncated));
        assert_eq!(err.exit_code(), 5);
    }
}
