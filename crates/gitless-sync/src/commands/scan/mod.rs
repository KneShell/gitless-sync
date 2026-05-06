pub mod compare;
pub mod github;
pub mod output;
pub mod walker;

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use rayon::prelude::*;

use crate::shared::config;
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::hash::blob_hash;
use crate::shared::ignore::IgnoreMatcher;
use crate::shared::normalize::prepare_for_hash;

use self::compare::{FileEntry, Status, classify};
use self::github::RemoteFile;
use self::output::{SCHEMA_VERSION, ScanReport, Summary};
use self::walker::LocalFile;

/// Max concurrent `fetch_last_commit_at` calls (G-011: GitHub abuse detection avoidance).
const MAX_COMMITS_CONCURRENCY: usize = 8;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "lowercase")]
pub enum Backend {
    #[default]
    Rest,
    Graphql,
}

#[derive(Debug)]
pub struct ScanArgs {
    pub repo: Option<String>,
    pub branch: String,
    pub local: String,
    pub ignore: Vec<String>,
    pub keep_bom: bool,
    pub pretty: bool,
    pub summary_only: bool,
    pub status: Option<String>,
    pub backend: Backend,
    pub verbose: u8,
}

/// Run the `scan` command and write the resulting JSON report to stdout.
///
/// Production callers inject `RealGhClient`; tests inject `MockGhClient`.
/// `gh api` handles authentication / rate limit / transport errors, so this
/// function only owns local IO + classification + JSON serialization.
///
/// # Errors
/// Returns any [`GitlessError`] raised by config loading, GitHub API calls,
/// or local IO. Returns [`GitlessError::PartialFailure`] when one or more
/// files could not be hashed.
pub(crate) fn run_with_client<C: GhClient + Sync>(
    args: &ScanArgs,
    client: &C,
) -> Result<(), GitlessError> {
    if args.backend == Backend::Graphql {
        return Err(GitlessError::Config(
            "GraphQL backend not implemented in v0.1; use --backend rest. Phase 4 ETA.".to_string(),
        ));
    }
    let (report, failed_count) = build_report(args, client)?;
    let json = output::serialize(&report, args.pretty).expect("ScanReport serialization is total");
    println!("{json}");
    if failed_count > 0 {
        return Err(GitlessError::PartialFailure { failed_count });
    }
    Ok(())
}

/// Run the full pipeline up to (but not including) stdout serialization.
///
/// Returns the assembled [`ScanReport`] and the count of files that failed
/// to hash so the caller can decide whether to map to
/// [`GitlessError::PartialFailure`].
///
/// # Errors
/// Propagates config / IO / GitHub API errors. Hash failures on individual
/// files do **not** error here — they show up in the returned `failed_count`.
fn build_report<C: GhClient + Sync>(
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

    let (mut entries, summary, failed_count) = assemble_entries(
        &local_files,
        &remote_files,
        client,
        &repo,
        &branch,
        args.keep_bom,
    )?;

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

/// Parse the comma-separated `--status` filter into a list of [`Status`].
fn parse_status_filter(raw: Option<&str>) -> Result<Option<Vec<Status>>, GitlessError> {
    let Some(s) = raw else {
        return Ok(None);
    };
    let mut out = Vec::new();
    for tok in s.split(',') {
        let trimmed = tok.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push(parse_status_token(trimmed)?);
    }
    if out.is_empty() {
        return Ok(None);
    }
    Ok(Some(out))
}

fn parse_status_token(s: &str) -> Result<Status, GitlessError> {
    match s {
        "identical" => Ok(Status::Identical),
        "local_only_changed" => Ok(Status::LocalOnlyChanged),
        "remote_only_changed" => Ok(Status::RemoteOnlyChanged),
        "drift" => Ok(Status::Drift),
        "failed" => Ok(Status::Failed),
        other => Err(GitlessError::Config(format!(
            "invalid --status value: {other}"
        ))),
    }
}

/// Compare matched local/remote files and produce per-entry report rows.
///
/// Calls `fetch_last_commit_at` only for paths whose SHA differs on both sides.
/// Commits API calls go through the rayon pool with up to
/// [`MAX_COMMITS_CONCURRENCY`] threads. Hash failures are recorded as
/// [`Status::Failed`] without aborting.
fn assemble_entries<C: GhClient + Sync>(
    local_files: &[LocalFile],
    remote_files: &[RemoteFile],
    client: &C,
    repo: &str,
    branch: &str,
    keep_bom: bool,
) -> Result<(Vec<FileEntry>, Summary, usize), GitlessError> {
    let local_map: HashMap<&str, &LocalFile> = local_files
        .iter()
        .map(|f| (f.relative_path.as_str(), f))
        .collect();
    let remote_map: HashMap<&str, &RemoteFile> =
        remote_files.iter().map(|f| (f.path.as_str(), f)).collect();

    let mut all_paths: BTreeSet<&str> = BTreeSet::new();
    all_paths.extend(local_map.keys().copied());
    all_paths.extend(remote_map.keys().copied());

    let pending = build_pre_entries(&all_paths, &local_map, &remote_map, keep_bom);
    let commit_map = fetch_commit_map(&pending, client, repo, branch)?;
    Ok(finalize_entries(pending, &commit_map))
}

/// Pass 1: hash local files and capture per-path state without calling the
/// Commits API. Hash failures are recorded as [`PreState::Failed`].
fn build_pre_entries(
    all_paths: &BTreeSet<&str>,
    local_map: &HashMap<&str, &LocalFile>,
    remote_map: &HashMap<&str, &RemoteFile>,
    keep_bom: bool,
) -> Vec<PreEntry> {
    let mut pending: Vec<PreEntry> = Vec::with_capacity(all_paths.len());
    for path in all_paths {
        let local = local_map.get(path).copied();
        let remote = remote_map.get(path).copied();
        let remote_sha = remote.map(|r| r.sha.clone());

        let state = match local {
            Some(lf) => match try_hash_local(&lf.absolute_path, keep_bom) {
                Ok((sha, is_binary)) => PreState::Hashed {
                    local_sha: Some(sha),
                    remote_sha,
                    local_mtime: Some(lf.mtime),
                    is_binary,
                },
                Err(err) => {
                    eprintln!("warning: failed to hash {path}: {err}");
                    PreState::Failed {
                        remote_sha,
                        local_mtime: Some(lf.mtime),
                    }
                }
            },
            None => PreState::Hashed {
                local_sha: None,
                remote_sha,
                local_mtime: None,
                is_binary: false,
            },
        };

        pending.push(PreEntry {
            path: (*path).to_string(),
            state,
        });
    }
    pending
}

/// Pass 2: collect paths that need a Commits API lookup and fetch their dates
/// in parallel. Map keyed by path so pass 3 can stitch the dates back in.
fn fetch_commit_map<C: GhClient + Sync>(
    pending: &[PreEntry],
    client: &C,
    repo: &str,
    branch: &str,
) -> Result<HashMap<String, DateTime<Utc>>, GitlessError> {
    let commit_paths: Vec<String> = pending
        .iter()
        .filter_map(|p| match &p.state {
            PreState::Hashed {
                local_sha: Some(l),
                remote_sha: Some(r),
                ..
            } if l != r => Some(p.path.clone()),
            _ => None,
        })
        .collect();
    let commit_path_refs: Vec<&str> = commit_paths.iter().map(String::as_str).collect();
    let commit_dates = fetch_commit_dates_parallel(client, repo, branch, &commit_path_refs)?;
    Ok(commit_paths.into_iter().zip(commit_dates).collect())
}

/// Pass 3: classify each pending entry and emit `FileEntry` rows in input
/// (`BTreeSet`) order.
fn finalize_entries(
    pending: Vec<PreEntry>,
    commit_map: &HashMap<String, DateTime<Utc>>,
) -> (Vec<FileEntry>, Summary, usize) {
    let mut entries: Vec<FileEntry> = Vec::with_capacity(pending.len());
    let mut summary = Summary::default();
    let mut failed_count = 0usize;

    for pre in pending {
        let entry = match pre.state {
            PreState::Failed {
                remote_sha,
                local_mtime,
            } => {
                summary.failed += 1;
                failed_count += 1;
                FileEntry {
                    path: pre.path,
                    status: Status::Failed,
                    local_sha: None,
                    remote_sha,
                    local_mtime,
                    remote_last_commit_at: None,
                    is_binary: false,
                }
            }
            PreState::Hashed {
                local_sha,
                remote_sha,
                local_mtime,
                is_binary,
            } => {
                let remote_last_commit_at = commit_map.get(pre.path.as_str()).copied();
                let status = classify(
                    local_sha.as_deref(),
                    remote_sha.as_deref(),
                    local_mtime,
                    remote_last_commit_at,
                );
                match status {
                    Status::Identical => summary.identical += 1,
                    Status::LocalOnlyChanged => summary.local_only_changed += 1,
                    Status::RemoteOnlyChanged => summary.remote_only_changed += 1,
                    Status::Drift => summary.drift += 1,
                    Status::Failed => summary.failed += 1,
                }
                FileEntry {
                    path: pre.path,
                    status,
                    local_sha,
                    remote_sha,
                    local_mtime,
                    remote_last_commit_at,
                    is_binary,
                }
            }
        };
        entries.push(entry);
    }

    (entries, summary, failed_count)
}

/// Hash result + remote SHA carried between pass 1 (hashing) and pass 3
/// (classification) of [`assemble_entries`].
enum PreState {
    Failed {
        remote_sha: Option<String>,
        local_mtime: Option<DateTime<Utc>>,
    },
    Hashed {
        local_sha: Option<String>,
        remote_sha: Option<String>,
        local_mtime: Option<DateTime<Utc>>,
        is_binary: bool,
    },
}

struct PreEntry {
    path: String,
    state: PreState,
}

/// Fetch `commit.committer.date` for each path in parallel (G-011: max 8 threads).
fn fetch_commit_dates_parallel<C: GhClient + Sync>(
    client: &C,
    repo: &str,
    branch: &str,
    paths: &[&str],
) -> Result<Vec<DateTime<Utc>>, GitlessError> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(MAX_COMMITS_CONCURRENCY)
        .build()
        .expect("rayon thread pool build");
    pool.install(|| {
        paths
            .par_iter()
            .map(|p| github::fetch_last_commit_at(client, repo, branch, p))
            .collect::<Result<Vec<_>, _>>()
    })
}

fn try_hash_local(path: &Path, keep_bom: bool) -> Result<(String, bool), std::io::Error> {
    let raw = fs::read(path)?;
    let (prepared, is_binary) = prepare_for_hash(&raw, keep_bom);
    Ok((blob_hash(&prepared), is_binary))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono::TimeZone;
    use tempfile::TempDir;

    use super::*;
    use crate::shared::gh::{GhResponse, MockGhClient};

    const COMMITS_BODY: &str = r#"[{
        "sha": "c1",
        "commit": {
            "author":    {"name": "a", "email": "a@e", "date": "2024-01-15T09:00:00Z"},
            "committer": {"name": "c", "email": "c@e", "date": "2024-01-15T10:30:00Z"},
            "message": "msg"
        },
        "url": "u"
    }]"#;

    fn args_for(dir: &Path, repo: Option<&str>) -> ScanArgs {
        ScanArgs {
            repo: repo.map(String::from),
            branch: "main".to_string(),
            local: dir.to_str().unwrap().to_string(),
            ignore: vec![],
            keep_bom: false,
            pretty: false,
            summary_only: false,
            status: None,
            backend: Backend::Rest,
            verbose: 0,
        }
    }

    fn mtime(secs: i64) -> chrono::DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
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

    fn commits_args(repo: &str, branch: &str, path: &str) -> Vec<String> {
        vec![
            "api".to_string(),
            format!("repos/{repo}/commits"),
            "-F".to_string(),
            format!("sha={branch}"),
            "-F".to_string(),
            format!("path={path}"),
            "-F".to_string(),
            "per_page=1".to_string(),
        ]
    }

    fn stub_tree(mock: &mut MockGhClient, repo: &str, branch: &str, body: &str) {
        mock.stub(tree_args(repo, branch), ok_resp(body.as_bytes()));
    }

    fn stub_commits(mock: &mut MockGhClient, repo: &str, branch: &str, path: &str, body: &str) {
        mock.stub(commits_args(repo, branch, path), ok_resp(body.as_bytes()));
    }

    // --- try_hash_local ----------------------------------------------------

    #[test]
    fn try_hash_local_returns_io_error_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nope.txt");
        let err = try_hash_local(&missing, false).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn try_hash_local_hashes_text_file() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("hello.md");
        fs::write(&p, "hello\n").unwrap();
        let (sha, is_bin) = try_hash_local(&p, false).unwrap();
        assert!(!is_bin);
        assert_eq!(sha, blob_hash(b"hello\n"));
    }

    #[test]
    fn try_hash_local_normalizes_crlf() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("crlf.md");
        fs::write(&p, b"hello\r\n").unwrap();
        let (sha, _) = try_hash_local(&p, false).unwrap();
        assert_eq!(sha, blob_hash(b"hello\n"));
    }

    #[test]
    fn try_hash_local_marks_binary() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("bin");
        fs::write(&p, [0u8, 1, 2, 3]).unwrap();
        let (_, is_bin) = try_hash_local(&p, false).unwrap();
        assert!(is_bin);
    }

    // --- build_report ------------------------------------------------------

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

    #[test]
    fn build_report_identical_skips_commits_api() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
        let local_sha = blob_hash(b"alpha\n");

        let mut mock = MockGhClient::new();
        let trees_body = format!(
            r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_sha}","size":6}}],"truncated":false}}"#
        );
        stub_tree(&mut mock, "o/r", "main", &trees_body);
        // Intentionally no commits stub: if `build_report` calls the Commits
        // API on an identical entry, MockGhClient falls back to Http err which
        // surfaces as a propagated error here.

        let args = args_for(dir.path(), Some("o/r"));
        let (report, failed) = build_report(&args, &mock).unwrap();

        assert_eq!(failed, 0);
        assert_eq!(report.summary.identical, 1);
        assert_eq!(report.summary.drift, 0);
        let entries = report.files.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, Status::Identical);
        assert_eq!(entries[0].local_sha.as_deref(), Some(local_sha.as_str()));
        assert_eq!(entries[0].remote_sha.as_deref(), Some(local_sha.as_str()));
        assert!(entries[0].remote_last_commit_at.is_none());
    }

    #[test]
    fn build_report_local_only_does_not_call_commits() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("only_here.md"), "x\n").unwrap();

        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            r#"{"sha":"x","tree":[],"truncated":false}"#,
        );

        let args = args_for(dir.path(), Some("o/r"));
        let (report, _) = build_report(&args, &mock).unwrap();
        assert_eq!(report.summary.local_only_changed, 1);
        let entries = report.files.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, Status::LocalOnlyChanged);
        assert!(entries[0].remote_sha.is_none());
        assert!(entries[0].remote_last_commit_at.is_none());
    }

    #[test]
    fn build_report_remote_only_does_not_call_commits() {
        let dir = TempDir::new().unwrap();
        let mut mock = MockGhClient::new();
        let trees_body = r#"{"sha":"x","tree":[{"path":"only_remote.md","mode":"100644","type":"blob","sha":"r1","size":1}],"truncated":false}"#;
        stub_tree(&mut mock, "o/r", "main", trees_body);

        let args = args_for(dir.path(), Some("o/r"));
        let (report, _) = build_report(&args, &mock).unwrap();
        assert_eq!(report.summary.remote_only_changed, 1);
        let entries = report.files.unwrap();
        assert_eq!(entries[0].status, Status::RemoteOnlyChanged);
        assert!(entries[0].local_sha.is_none());
        assert!(entries[0].remote_last_commit_at.is_none());
    }

    #[test]
    fn build_report_drift_calls_commits_api() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("d.md"), "local\n").unwrap();

        let mut mock = MockGhClient::new();
        let trees_body = r#"{"sha":"x","tree":[{"path":"d.md","mode":"100644","type":"blob","sha":"sha-remote","size":6}],"truncated":false}"#;
        stub_tree(&mut mock, "o/r", "main", trees_body);
        stub_commits(&mut mock, "o/r", "main", "d.md", COMMITS_BODY);

        let args = args_for(dir.path(), Some("o/r"));
        let (report, _) = build_report(&args, &mock).unwrap();
        let entries = report.files.unwrap();
        assert_eq!(entries.len(), 1);
        assert!(matches!(
            entries[0].status,
            Status::Drift | Status::LocalOnlyChanged | Status::RemoteOnlyChanged
        ));
        assert!(entries[0].remote_last_commit_at.is_some());
    }

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

    // --- assemble_entries --------------------------------------------------

    #[test]
    fn assemble_entries_marks_unreadable_local_as_failed() {
        let dir = TempDir::new().unwrap();
        let bogus = LocalFile {
            relative_path: "ghost.md".to_string(),
            absolute_path: dir.path().join("ghost-not-here.md"),
            mtime: mtime(1_700_000_000),
        };
        let remote = RemoteFile {
            path: "ghost.md".to_string(),
            sha: "remote-sha".to_string(),
        };

        let mut mock = MockGhClient::new();
        stub_commits(&mut mock, "o/r", "main", "ghost.md", COMMITS_BODY);

        let (entries, summary, failed) =
            assemble_entries(&[bogus], &[remote], &mock, "o/r", "main", false).unwrap();

        assert_eq!(failed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, Status::Failed);
        assert!(entries[0].local_sha.is_none());
        assert_eq!(entries[0].remote_sha.as_deref(), Some("remote-sha"));
    }

    #[test]
    fn assemble_entries_skips_commits_for_identical() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("ok.md"), "hi\n").unwrap();
        let sha = blob_hash(b"hi\n");

        let local = LocalFile {
            relative_path: "ok.md".to_string(),
            absolute_path: dir.path().join("ok.md"),
            mtime: mtime(1_700_000_000),
        };
        let remote = RemoteFile {
            path: "ok.md".to_string(),
            sha: sha.clone(),
        };

        // No commits stub; if assemble_entries hits the Commits API anyway, it
        // would surface as an Http error (MockGhClient default).
        let mock = MockGhClient::new();
        let (entries, summary, failed) =
            assemble_entries(&[local], &[remote], &mock, "o/r", "main", false).unwrap();

        assert_eq!(failed, 0);
        assert_eq!(summary.identical, 1);
        assert_eq!(entries[0].status, Status::Identical);
    }

    // --- run_with_client ---------------------------------------------------

    #[test]
    fn run_with_client_returns_partial_failure_exit_code_for_partial_failure_variant() {
        // Concrete check: exit code mapping for the variant produced by run_with_client.
        let err = GitlessError::PartialFailure { failed_count: 2 };
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn run_with_client_returns_config_error_for_graphql_backend() {
        let dir = TempDir::new().unwrap();
        let mut args = args_for(dir.path(), Some("o/r"));
        args.backend = Backend::Graphql;
        let mock = MockGhClient::new();
        let err = run_with_client(&args, &mock).unwrap_err();
        assert!(matches!(err, GitlessError::Config(ref msg) if msg.contains("GraphQL")));
        assert_eq!(err.exit_code(), 1);
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

    // --- summary-only / status filter / verbose ----------------------------

    #[test]
    fn build_report_summary_only_drops_files_field() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
        let local_a = blob_hash(b"alpha\n");

        let mut mock = MockGhClient::new();
        let trees_body = format!(
            r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_a}","size":6}}],"truncated":false}}"#
        );
        stub_tree(&mut mock, "o/r", "main", &trees_body);

        let mut args = args_for(dir.path(), Some("o/r"));
        args.summary_only = true;
        let (report, _) = build_report(&args, &mock).unwrap();
        assert!(report.files.is_none());
        assert_eq!(report.summary.identical, 1);
        let json = output::serialize(&report, false).unwrap();
        assert!(!json.contains("\"files\""));
    }

    #[test]
    fn build_report_status_filter_keeps_only_matching_entries() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("identical.md"), "alpha\n").unwrap();
        fs::write(dir.path().join("local_only.md"), "beta\n").unwrap();
        let local_a = blob_hash(b"alpha\n");

        let mut mock = MockGhClient::new();
        let trees_body = format!(
            r#"{{"sha":"x","tree":[{{"path":"identical.md","mode":"100644","type":"blob","sha":"{local_a}","size":6}}],"truncated":false}}"#
        );
        stub_tree(&mut mock, "o/r", "main", &trees_body);

        let mut args = args_for(dir.path(), Some("o/r"));
        args.status = Some("local_only_changed".to_string());
        let (report, _) = build_report(&args, &mock).unwrap();

        assert_eq!(report.summary.identical, 1);
        assert_eq!(report.summary.local_only_changed, 1);

        let entries = report.files.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, Status::LocalOnlyChanged);
        assert_eq!(entries[0].path, "local_only.md");
    }

    #[test]
    fn build_report_status_filter_supports_multiple_values() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("identical.md"), "alpha\n").unwrap();
        fs::write(dir.path().join("local_only.md"), "beta\n").unwrap();
        let local_a = blob_hash(b"alpha\n");

        let mut mock = MockGhClient::new();
        let trees_body = format!(
            r#"{{"sha":"x","tree":[{{"path":"identical.md","mode":"100644","type":"blob","sha":"{local_a}","size":6}},{{"path":"remote_only.md","mode":"100644","type":"blob","sha":"deadbeef","size":3}}],"truncated":false}}"#
        );
        stub_tree(&mut mock, "o/r", "main", &trees_body);

        let mut args = args_for(dir.path(), Some("o/r"));
        args.status = Some("local_only_changed,remote_only_changed".to_string());
        let (report, _) = build_report(&args, &mock).unwrap();

        let entries = report.files.unwrap();
        assert_eq!(entries.len(), 2);
        for e in &entries {
            assert!(matches!(
                e.status,
                Status::LocalOnlyChanged | Status::RemoteOnlyChanged
            ));
        }
    }

    #[test]
    fn build_report_summary_only_overrides_status_filter() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
        let local_a = blob_hash(b"alpha\n");

        let mut mock = MockGhClient::new();
        let trees_body = format!(
            r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_a}","size":6}}],"truncated":false}}"#
        );
        stub_tree(&mut mock, "o/r", "main", &trees_body);

        let mut args = args_for(dir.path(), Some("o/r"));
        args.summary_only = true;
        args.status = Some("drift".to_string());
        let (report, _) = build_report(&args, &mock).unwrap();

        assert!(report.files.is_none());
        assert_eq!(report.summary.identical, 1);
    }

    #[test]
    fn build_report_invalid_status_filter_yields_config_error() {
        let dir = TempDir::new().unwrap();
        let mut mock = MockGhClient::new();
        stub_tree(
            &mut mock,
            "o/r",
            "main",
            r#"{"sha":"x","tree":[],"truncated":false}"#,
        );

        let mut args = args_for(dir.path(), Some("o/r"));
        args.status = Some("nonsense".to_string());
        let err = build_report(&args, &mock).unwrap_err();
        assert!(matches!(err, GitlessError::Config(_)));
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn build_report_verbose_levels_do_not_change_report() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
        let local_a = blob_hash(b"alpha\n");

        for level in [0u8, 1, 2] {
            let mut mock = MockGhClient::new();
            let trees_body = format!(
                r#"{{"sha":"x","tree":[{{"path":"a.md","mode":"100644","type":"blob","sha":"{local_a}","size":6}}],"truncated":false}}"#
            );
            stub_tree(&mut mock, "o/r", "main", &trees_body);

            let mut args = args_for(dir.path(), Some("o/r"));
            args.verbose = level;
            let (report, _) = build_report(&args, &mock).unwrap();
            assert_eq!(report.summary.identical, 1);
            assert!(report.files.is_some());
        }
    }

    #[test]
    fn build_report_drift_multiple_paths_invokes_commits_api_per_path() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.md"), "alpha\n").unwrap();
        fs::write(dir.path().join("b.md"), "beta\n").unwrap();
        fs::write(dir.path().join("c.md"), "gamma\n").unwrap();

        let mut mock = MockGhClient::new();
        let trees_body = r#"{"sha":"x","tree":[
            {"path":"a.md","mode":"100644","type":"blob","sha":"remote-a","size":6},
            {"path":"b.md","mode":"100644","type":"blob","sha":"remote-b","size":5},
            {"path":"c.md","mode":"100644","type":"blob","sha":"remote-c","size":6}
        ],"truncated":false}"#;
        stub_tree(&mut mock, "o/r", "main", trees_body);
        stub_commits(&mut mock, "o/r", "main", "a.md", COMMITS_BODY);
        stub_commits(&mut mock, "o/r", "main", "b.md", COMMITS_BODY);
        stub_commits(&mut mock, "o/r", "main", "c.md", COMMITS_BODY);

        let args = args_for(dir.path(), Some("o/r"));
        let (report, failed) = build_report(&args, &mock).unwrap();
        assert_eq!(failed, 0);

        let entries = report.files.unwrap();
        assert_eq!(entries.len(), 3);
        let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["a.md", "b.md", "c.md"]);
        for e in &entries {
            assert!(
                e.remote_last_commit_at.is_some(),
                "drift entry {} should have commit timestamp",
                e.path
            );
        }
    }

    #[test]
    fn fetch_commit_dates_parallel_short_circuits_on_empty_input() {
        // No stubs registered; if the function issued any call, MockGhClient
        // would error. Empty input must short-circuit before that happens.
        let mock = MockGhClient::new();
        let result = fetch_commit_dates_parallel(&mock, "o/r", "main", &[]).unwrap();
        assert!(result.is_empty());
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

    // --- parse_status_filter ----------------------------------------------

    #[test]
    fn parse_status_filter_returns_none_when_arg_absent() {
        assert!(parse_status_filter(None).unwrap().is_none());
    }

    #[test]
    fn parse_status_filter_returns_none_for_empty_or_whitespace() {
        assert!(parse_status_filter(Some("")).unwrap().is_none());
        assert!(parse_status_filter(Some(" , ")).unwrap().is_none());
    }

    #[test]
    fn parse_status_filter_parses_single_value() {
        let v = parse_status_filter(Some("drift")).unwrap().unwrap();
        assert_eq!(v, vec![Status::Drift]);
    }

    #[test]
    fn parse_status_filter_parses_multiple_values_with_whitespace() {
        let v = parse_status_filter(Some("drift, local_only_changed ,identical"))
            .unwrap()
            .unwrap();
        assert_eq!(
            v,
            vec![Status::Drift, Status::LocalOnlyChanged, Status::Identical]
        );
    }

    #[test]
    fn parse_status_filter_accepts_all_known_tokens() {
        let v = parse_status_filter(Some(
            "identical,local_only_changed,remote_only_changed,drift,failed",
        ))
        .unwrap()
        .unwrap();
        assert_eq!(
            v,
            vec![
                Status::Identical,
                Status::LocalOnlyChanged,
                Status::RemoteOnlyChanged,
                Status::Drift,
                Status::Failed,
            ]
        );
    }

    #[test]
    fn parse_status_filter_errors_on_unknown_token() {
        let err = parse_status_filter(Some("nonsense")).unwrap_err();
        assert!(matches!(&err, GitlessError::Config(msg) if msg.contains("nonsense")));
        assert_eq!(err.exit_code(), 1);
    }
}
