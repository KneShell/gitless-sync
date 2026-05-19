//! Orchestration for `diff` — config load + GitHub Trees/Blobs lookup +
//! local file read + dispatch to `render::{one_sided, both_sides}`.
//! `mod.rs` calls `compute_diff` and only handles stdout/stderr writing.

use std::path::Path;

use crate::shared::config;
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::github;

use super::args::DiffArgs;
use super::io::read_local_optional;
use super::render::{
    DiffKey, DiffOutcome, Side, both_sides, both_sides_json, one_sided, one_sided_json,
};

/// Compute the diff between the local file and the remote blob for `args.path`
/// without writing to stdout/stderr.
///
/// # Errors
/// - [`GitlessError::Config`] when `--repo` resolves to nothing or the path
///   does not exist on either side.
/// - GitHub API errors propagated from `fetch_tree` / `fetch_blob` (auth /
///   rate limit / transport failures owned by `gh`).
/// - [`GitlessError::Io`] for unexpected local IO failures (other than the
///   "file does not exist locally" case, which is treated as one-sided).
pub(crate) fn compute_diff<C: GhClient>(
    args: &DiffArgs,
    client: &C,
) -> Result<DiffOutcome, GitlessError> {
    let local_root = Path::new(&args.local);
    let toml_path = local_root.join("gitless-sync.toml");
    let cfg = config::load(Some(&toml_path))?;

    let repo = args
        .repo
        .as_deref()
        .or(cfg.repo.as_deref())
        .ok_or_else(|| GitlessError::Config("repo not specified".to_string()))?
        .to_string();
    let branch = config::resolve_branch(args.branch.as_deref(), cfg.branch.as_deref());

    let local_key = args.path.replace('\\', "/");
    let remote_key = args
        .remote_path
        .as_deref()
        .map_or_else(|| local_key.clone(), |p| p.replace('\\', "/"));

    let tree = github::fetch_tree_with_fallback(client, &repo, &branch)?;
    let remote_entry = tree.iter().find(|e| e.path == remote_key);

    let local_abs = local_root.join(&args.path);
    let local_raw = read_local_optional(&local_abs)?;

    let remote_raw = match remote_entry {
        Some(entry) => Some(github::fetch_blob(client, &repo, &entry.sha)?),
        None => None,
    };

    let diff_key = DiffKey {
        local: &local_key,
        remote: &remote_key,
    };

    match (local_raw, remote_raw, args.json) {
        (None, None, _) => Err(GitlessError::Config(format!(
            "path not found locally or remotely: local={local_key} remote={remote_key}"
        ))),
        (Some(local), None, false) => Ok(one_sided(&local, "(local only)", args.keep_bom)),
        (Some(local), None, true) => Ok(one_sided_json(&local, Side::LocalOnly, args.keep_bom)),
        (None, Some(remote), false) => Ok(one_sided(&remote, "(remote only)", args.keep_bom)),
        (None, Some(remote), true) => Ok(one_sided_json(&remote, Side::RemoteOnly, args.keep_bom)),
        (Some(local), Some(remote), false) => {
            Ok(both_sides(&local, &remote, diff_key, args.keep_bom))
        }
        (Some(local), Some(remote), true) => {
            Ok(both_sides_json(&local, &remote, diff_key, args.keep_bom))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::shared::gh::MockGhClient;

    use super::super::test_helpers::{
        args_for, err_resp, stub_blob, stub_empty_tree, stub_tree, tree_args, tree_body_with_blob,
    };

    #[test]
    fn compute_diff_returns_error_when_repo_missing() {
        let dir = TempDir::new().unwrap();
        let mock = MockGhClient::new();
        let mut args = args_for(dir.path(), "x.md");
        args.repo = None;
        let err = compute_diff(&args, &mock).unwrap_err();
        assert!(matches!(err, GitlessError::Config(_)));
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn compute_diff_neither_side_yields_config_error() {
        let dir = TempDir::new().unwrap();
        let mut mock = MockGhClient::new();
        stub_empty_tree(&mut mock);

        let args = args_for(dir.path(), "ghost.md");
        let err = compute_diff(&args, &mock).unwrap_err();
        assert!(matches!(err, GitlessError::Config(_)));
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn compute_diff_local_only_returns_local_content_and_label() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("only_local.md"), "hello\n").unwrap();
        let mut mock = MockGhClient::new();
        stub_empty_tree(&mut mock);

        let args = args_for(dir.path(), "only_local.md");
        let outcome = compute_diff(&args, &mock).unwrap();
        assert_eq!(outcome.stderr_message, "(local only)");
        assert_eq!(outcome.stdout, b"hello\n");
    }

    #[test]
    fn compute_diff_one_sided_binary_emits_message_only() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("img.png"), [0u8, 1, 2, 3, 0, 5]).unwrap();
        let mut mock = MockGhClient::new();
        stub_empty_tree(&mut mock);

        let args = args_for(dir.path(), "img.png");
        let outcome = compute_diff(&args, &mock).unwrap();
        assert!(
            outcome
                .stderr_message
                .starts_with("(local only) (binary file"),
            "got: {}",
            outcome.stderr_message
        );
        assert!(outcome.stdout.is_empty());
    }

    #[test]
    fn compute_diff_both_sides_identical_yields_empty_diff() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.md"), "hello\n").unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            &tree_body_with_blob("a.md", "shaIdentical"),
        );
        stub_blob(&mut mock, "o/r", "shaIdentical", b"hello\n");

        let args = args_for(dir.path(), "a.md");
        let outcome = compute_diff(&args, &mock).unwrap();
        assert!(outcome.stderr_message.is_empty());
        let s = String::from_utf8(outcome.stdout).unwrap();
        assert!(
            !s.contains("@@"),
            "expected no diff hunk for identical inputs, got: {s}"
        );
    }

    #[test]
    fn compute_diff_both_sides_different_emits_unified_diff() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.md"), "alpha\nbeta\n").unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            &tree_body_with_blob("a.md", "shaRemote"),
        );
        stub_blob(&mut mock, "o/r", "shaRemote", b"alpha\ngamma\n");

        let args = args_for(dir.path(), "a.md");
        let outcome = compute_diff(&args, &mock).unwrap();
        let s = String::from_utf8(outcome.stdout).unwrap();
        assert!(s.contains("--- a/a.md"), "missing a header: {s}");
        assert!(s.contains("+++ b/a.md"), "missing b header: {s}");
        assert!(s.contains("-gamma"), "missing remote line marker: {s}");
        assert!(s.contains("+beta"), "missing local line marker: {s}");
    }

    #[test]
    fn compute_diff_normalizes_crlf_before_comparing() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.md"), b"hello\r\n").unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            &tree_body_with_blob("a.md", "shaLF"),
        );
        stub_blob(&mut mock, "o/r", "shaLF", b"hello\n");

        let args = args_for(dir.path(), "a.md");
        let outcome = compute_diff(&args, &mock).unwrap();
        let s = String::from_utf8(outcome.stdout).unwrap();
        assert!(
            !s.contains("@@"),
            "CRLF vs LF should be normalized away, got: {s}"
        );
    }

    #[test]
    fn compute_diff_binary_local_skips_diff() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("blob.bin"), [0u8, 1, 2, 3]).unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            &tree_body_with_blob("blob.bin", "shaBin"),
        );
        stub_blob(&mut mock, "o/r", "shaBin", b"hello\n");

        let args = args_for(dir.path(), "blob.bin");
        let outcome = compute_diff(&args, &mock).unwrap();
        assert_eq!(outcome.stderr_message, "binary file, diff skipped");
        assert!(outcome.stdout.is_empty());
    }

    #[test]
    fn compute_diff_binary_remote_skips_diff() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.md"), "hello\n").unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            &tree_body_with_blob("a.md", "shaBinRemote"),
        );
        stub_blob(&mut mock, "o/r", "shaBinRemote", &[0u8, 1, 2, 3, 0, 5]);

        let args = args_for(dir.path(), "a.md");
        let outcome = compute_diff(&args, &mock).unwrap();
        assert_eq!(outcome.stderr_message, "binary file, diff skipped");
        assert!(outcome.stdout.is_empty());
    }

    #[test]
    fn compute_diff_propagates_auth_error_from_trees() {
        let dir = TempDir::new().unwrap();
        let mut mock = MockGhClient::new();
        mock.stub(
            tree_args("o/r", "main"),
            err_resp("gh: Bad credentials (HTTP 401)"),
        );

        let args = args_for(dir.path(), "a.md");
        let err = compute_diff(&args, &mock).unwrap_err();
        assert!(matches!(err, GitlessError::AuthFailed));
    }

    #[test]
    #[cfg(windows)]
    fn compute_diff_normalizes_backslash_path_to_forward_slash() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("sub");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("a.md"), "hello\n").unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            &tree_body_with_blob("sub/a.md", "shaSub"),
        );
        stub_blob(&mut mock, "o/r", "shaSub", b"hello\n");

        let args = args_for(dir.path(), r"sub\a.md");
        let outcome = compute_diff(&args, &mock).unwrap();
        let s = String::from_utf8(outcome.stdout).unwrap();
        assert!(!s.contains("@@"), "expected identical content, got: {s}");
        assert!(outcome.stderr_message.is_empty());
    }
}
