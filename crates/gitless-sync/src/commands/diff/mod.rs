use std::fs;
use std::io::Write;
use std::path::Path;

use similar::TextDiff;

use crate::shared::config;
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::github;
use crate::shared::normalize::{is_binary, normalize_text};

#[derive(Debug)]
pub struct DiffArgs {
    pub repo: Option<String>,
    pub branch: String,
    pub local: String,
    pub keep_bom: bool,
    pub path: String,
}

/// Run the `diff` command and write the unified diff (or a one-sided message)
/// to stdout, with status messages on stderr.
///
/// Production callers inject `RealGhClient`; tests inject `MockGhClient`.
///
/// # Errors
/// Returns any [`GitlessError`] raised by config loading, GitHub API calls,
/// or local IO. Returns [`GitlessError::Config`] when the requested path
/// exists on neither side.
pub fn run_with_client<C: GhClient>(args: &DiffArgs, client: &C) -> Result<(), GitlessError> {
    let outcome = compute_diff(args, client)?;
    if !outcome.stderr_message.is_empty() {
        eprintln!("{}", outcome.stderr_message);
    }
    std::io::stdout().write_all(&outcome.stdout)?;
    Ok(())
}

/// Result of a `diff` invocation, separated from actual I/O so tests can
/// inspect `stdout` / `stderr_message` without capturing real handles.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DiffOutcome {
    pub stdout: Vec<u8>,
    pub stderr_message: String,
}

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
    let branch = &args.branch;

    let key = args.path.replace('\\', "/");

    let tree = github::fetch_tree(client, &repo, branch)?;
    let remote_entry = tree.iter().find(|e| e.path == key);

    let local_abs = local_root.join(&args.path);
    let local_raw = read_local_optional(&local_abs)?;

    let remote_raw = match remote_entry {
        Some(entry) => Some(github::fetch_blob(client, &repo, &entry.sha)?),
        None => None,
    };

    match (local_raw, remote_raw) {
        (None, None) => Err(GitlessError::Config(format!(
            "path not found locally or remotely: {key}"
        ))),
        (Some(local), None) => Ok(one_sided(&local, "(local only)", args.keep_bom)),
        (None, Some(remote)) => Ok(one_sided(&remote, "(remote only)", args.keep_bom)),
        (Some(local), Some(remote)) => Ok(both_sides(&local, &remote, &key, args.keep_bom)),
    }
}

fn one_sided(content: &[u8], label: &'static str, keep_bom: bool) -> DiffOutcome {
    if is_binary(content) {
        DiffOutcome {
            stdout: Vec::new(),
            stderr_message: format!("{label} (binary file, content not shown)"),
        }
    } else {
        DiffOutcome {
            stdout: normalize_text(content, keep_bom),
            stderr_message: label.to_string(),
        }
    }
}

fn both_sides(local: &[u8], remote: &[u8], key: &str, keep_bom: bool) -> DiffOutcome {
    if is_binary(local) || is_binary(remote) {
        return DiffOutcome {
            stdout: Vec::new(),
            stderr_message: "binary file, diff skipped".to_string(),
        };
    }
    let old_bytes = normalize_text(remote, keep_bom);
    let new_bytes = normalize_text(local, keep_bom);
    let old_str = String::from_utf8_lossy(&old_bytes);
    let new_str = String::from_utf8_lossy(&new_bytes);
    let diff = TextDiff::from_lines(old_str.as_ref(), new_str.as_ref());
    let header_a = format!("a/{key}");
    let header_b = format!("b/{key}");
    let mut unified = diff.unified_diff();
    unified.header(&header_a, &header_b);
    DiffOutcome {
        stdout: unified.to_string().into_bytes(),
        stderr_message: String::new(),
    }
}

fn read_local_optional(path: &Path) -> Result<Option<Vec<u8>>, GitlessError> {
    match fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(GitlessError::Io(e)),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use tempfile::TempDir;

    use super::*;
    use crate::shared::gh::{GhResponse, MockGhClient};

    fn args_for(dir: &Path, path: &str) -> DiffArgs {
        DiffArgs {
            repo: Some("o/r".to_string()),
            branch: "main".to_string(),
            local: dir.to_str().unwrap().to_string(),
            keep_bom: false,
            path: path.to_string(),
        }
    }

    fn ok_resp(body: &[u8]) -> GhResponse {
        GhResponse {
            stdout: body.to_vec(),
            stderr: String::new(),
            exit_code: 0,
        }
    }

    fn err_resp(stderr: &str) -> GhResponse {
        GhResponse {
            stdout: Vec::new(),
            stderr: stderr.to_string(),
            exit_code: 1,
        }
    }

    fn tree_args(repo: &str, branch: &str) -> Vec<String> {
        vec![
            "api".to_string(),
            format!("repos/{repo}/git/trees/{branch}?recursive=1"),
        ]
    }

    fn blob_args(repo: &str, sha: &str) -> Vec<String> {
        vec!["api".to_string(), format!("repos/{repo}/git/blobs/{sha}")]
    }

    fn tree_body_with_blob(path: &str, sha: &str) -> String {
        format!(
            r#"{{"sha":"x","tree":[{{"path":"{path}","mode":"100644","type":"blob","sha":"{sha}","size":1}}],"truncated":false}}"#
        )
    }

    fn blob_body_for(content: &[u8]) -> String {
        let b64 = BASE64_STANDARD.encode(content);
        format!(r#"{{"sha":"abc","content":"{b64}","encoding":"base64","size":1,"url":"u"}}"#)
    }

    fn stub_tree(mock: &mut MockGhClient, repo: &str, branch: &str, body: &str) {
        mock.stub(tree_args(repo, branch), ok_resp(body.as_bytes()));
    }

    fn stub_blob(mock: &mut MockGhClient, repo: &str, sha: &str, content: &[u8]) {
        mock.stub(
            blob_args(repo, sha),
            ok_resp(blob_body_for(content).as_bytes()),
        );
    }

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
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            r#"{"sha":"x","tree":[],"truncated":false}"#,
        );

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
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            r#"{"sha":"x","tree":[],"truncated":false}"#,
        );

        let args = args_for(dir.path(), "only_local.md");
        let outcome = compute_diff(&args, &mock).unwrap();
        assert_eq!(outcome.stderr_message, "(local only)");
        assert_eq!(outcome.stdout, b"hello\n");
    }

    #[test]
    fn compute_diff_remote_only_returns_remote_content_and_label() {
        let dir = TempDir::new().unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            &tree_body_with_blob("only_remote.md", "remoteSha"),
        );
        stub_blob(&mut mock, "o/r", "remoteSha", b"remote-content\n");

        let args = args_for(dir.path(), "only_remote.md");
        let outcome = compute_diff(&args, &mock).unwrap();
        assert_eq!(outcome.stderr_message, "(remote only)");
        assert_eq!(outcome.stdout, b"remote-content\n");
    }

    #[test]
    fn compute_diff_one_sided_binary_emits_message_only() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("img.png"), [0u8, 1, 2, 3, 0, 5]).unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            r#"{"sha":"x","tree":[],"truncated":false}"#,
        );

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
        run_with_client(&args, &mock).unwrap();
    }

    #[test]
    fn read_local_optional_returns_none_for_missing_file() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("nope.md");
        let result = read_local_optional(&p).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn read_local_optional_returns_bytes_for_existing_file() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("exists.md");
        fs::write(&p, b"abc").unwrap();
        let result = read_local_optional(&p).unwrap().unwrap();
        assert_eq!(result, b"abc");
    }
}
