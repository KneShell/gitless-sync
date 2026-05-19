//! End-to-end `build_report` benchmarks at 10K / 100K path scale.
//!
//! Phase 5 task R3 — bound the per-path overhead of the `.gitattributes`
//! parser inside the scan pipeline (proxy for the Phase 4 GraphQL batching
//! invariant: the gain is on the Commits API side, so as long as the
//! `.gitattributes` work does not dominate scan walltime there is room for
//! the projected ~38× speedup at 1000+ paths to survive).
//!
//! Numbers are recorded in `docs/research/phase5-scan-scale-bench.md`.
//! Threshold gating belongs to a follow-up CI task (`U`); R3 is record-only.
//!
//! Caveat: `BenchGhClient` returns `HashMap` lookups, so REST / GraphQL
//! backend cost differences are not visible here. The bench measures the
//! parse + walk + classify + serialize structure, not real `gh` subprocess
//! latency.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

use criterion::{Criterion, criterion_group, criterion_main};
use tempfile::TempDir;

use gitless_sync::commands::scan::output::serialize;
use gitless_sync::commands::scan::{Backend, ScanArgs, build_report};
use gitless_sync::shared::error::GitlessError;
use gitless_sync::shared::gh::{GhClient, GhResponse};
use gitless_sync::shared::hash::blob_hash;

const RULES: usize = 100;
const PATHS_10K: usize = 10_000;
const PATHS_100K: usize = 100_000;
const REPO: &str = "owner/repo";
const BRANCH: &str = "main";

// --- BenchGhClient ---------------------------------------------------------
//
// Bench targets compile without `#[cfg(test)]` and cannot share
// `tests/common/`. This is the smallest stub that satisfies `GhClient`:
// argv → canned stdout, no graphql wildcard (the bench scenarios are
// engineered so Commits API is never called, see fixture comments).
struct BenchGhClient {
    responses: HashMap<Vec<String>, GhResponse>,
}

impl BenchGhClient {
    fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    fn stub(&mut self, args: Vec<String>, body: Vec<u8>) {
        self.responses.insert(
            args,
            GhResponse {
                stdout: body,
                stderr: String::new(),
                exit_code: 0,
            },
        );
    }
}

impl GhClient for BenchGhClient {
    fn api(&self, args: &[String]) -> Result<GhResponse, GitlessError> {
        match self.responses.get(args) {
            Some(r) => Ok(r.clone()),
            None => Err(GitlessError::Http(format!(
                "BenchGhClient: no stub registered for args {args:?}"
            ))),
        }
    }
}

// --- Fixture content -------------------------------------------------------

/// Pure ASCII + LF-terminated content. `prepare_for_hash` Unspecified branch
/// (default for paths not matching any `.gitattributes` rule) is a no-op on
/// this shape, so `blob_hash(content)` matches the local hash on disk.
fn content_for(i: usize) -> String {
    format!("file_{i:05} content line\n")
}

/// Tree path for index `i`. Names deliberately avoid the X bench rule
/// patterns (`*.extNNN`, `dirNNN/**`, `**/fileNNN.txt`, etc.) so that the
/// 100-rule `.gitattributes` fixture lands every path on the Unspecified
/// branch — same hash as the no-attrs scenario, isolating overhead as the
/// single variable. `i / 100` for the subdir keeps consecutive files in
/// the same parent so [`write_local_files`] only mkdirs once per subdir.
fn tree_path_for(i: usize) -> String {
    format!("paths/p{:03}/file_{:05}.dat", i / 100, i)
}

fn build_gitattributes_content(rule_count: usize) -> String {
    let mut s = String::new();
    for i in 0..rule_count {
        match i % 5 {
            0 => writeln!(s, "*.ext{i:03} text=auto").unwrap(),
            1 => writeln!(s, "dir{i:03}/** binary").unwrap(),
            2 => writeln!(s, "**/file{i:03}.txt eol=lf").unwrap(),
            3 => writeln!(s, "dir{:03}/sub*/file*.bin -text", i / 4).unwrap(),
            _ => writeln!(s, "/path{i:03}/* eol=crlf").unwrap(),
        }
    }
    s
}

/// Build `(path, content, sha)` for `count` files. Each tuple is a real
/// local file on disk + a Trees entry whose SHA is the LF-blob hash of the
/// content. Both sides match → all paths classify as `Status::Identical` →
/// `extract_commit_paths` is empty → Commits API is never called → backend
/// cost difference between REST/GraphQL is structurally zero in this bench.
fn build_paths(count: usize) -> Vec<(String, String, String)> {
    (0..count)
        .map(|i| {
            let content = content_for(i);
            let sha = blob_hash(content.as_bytes());
            (tree_path_for(i), content, sha)
        })
        .collect()
}

fn build_trees_body(paths: &[(String, String, String)]) -> Vec<u8> {
    let mut s = String::with_capacity(paths.len() * 128);
    s.push_str(r#"{"sha":"x","tree":["#);
    for (i, (path, _, sha)) in paths.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(
            s,
            r#"{{"path":"{path}","mode":"100644","type":"blob","sha":"{sha}","size":1}}"#
        );
    }
    s.push_str(r#"],"truncated":false}"#);
    s.into_bytes()
}

fn write_local_files(dir: &Path, paths: &[(String, String, String)]) {
    let mut current_subdir = String::new();
    for (path, content, _) in paths {
        let abs = dir.join(path);
        let parent = abs.parent().expect("parent");
        let parent_str = parent.to_string_lossy().into_owned();
        if parent_str != current_subdir {
            fs::create_dir_all(parent).expect("create_dir_all");
            current_subdir = parent_str;
        }
        fs::write(&abs, content).expect("write file");
    }
}

fn tree_args(repo: &str, branch: &str) -> Vec<String> {
    vec![
        "api".to_string(),
        format!("repos/{repo}/git/trees/{branch}?recursive=1"),
    ]
}

fn args_for(dir: &Path) -> ScanArgs {
    ScanArgs {
        repo: Some(REPO.to_string()),
        branch: Some(BRANCH.to_string()),
        local: dir.to_str().expect("utf-8 path").to_string(),
        ignore: vec![],
        keep_bom: false,
        pretty: false,
        summary_only: false,
        status: None,
        backend: Backend::Rest,
        verbose: 0,
    }
}

// --- Fixture builders ------------------------------------------------------

fn fixture_10k_identical(with_attrs: bool) -> (TempDir, BenchGhClient) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let paths = build_paths(PATHS_10K);
    write_local_files(tmp.path(), &paths);
    if with_attrs {
        fs::write(
            tmp.path().join(".gitattributes"),
            build_gitattributes_content(RULES),
        )
        .expect("write .gitattributes");
    }
    let mut mock = BenchGhClient::new();
    mock.stub(tree_args(REPO, BRANCH), build_trees_body(&paths));
    (tmp, mock)
}

/// 100K Trees entries against an empty local directory — every path falls
/// to `Status::RemoteOnlyChanged` (`local_sha` is None, so the SHA-mismatch
/// filter excludes it), Commits API is skipped. Walker walks an empty dir,
/// so the dominant cost is Trees JSON parse + classify + serialize at scale.
fn fixture_100k_remote_only() -> (TempDir, BenchGhClient) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let paths = build_paths(PATHS_100K);
    let mut mock = BenchGhClient::new();
    mock.stub(tree_args(REPO, BRANCH), build_trees_body(&paths));
    (tmp, mock)
}

// --- Helper: run the full pipeline + serialize for end-to-end walltime -----

fn run_full(args: &ScanArgs, client: &BenchGhClient) {
    let (report, _failed) = build_report(args, client).expect("build_report");
    let json = serialize(&report, false).expect("serialize");
    black_box(json);
}

/// One-shot raw timing dump for the long-run 100K case. Criterion's default
/// sample size would run this many times; we only need a single number to
/// document the projection point alongside the regularly-sampled 10K data.
fn dump_one_shot(label: &str, args: &ScanArgs, client: &BenchGhClient) {
    let start = Instant::now();
    run_full(args, client);
    let elapsed = start.elapsed();
    eprintln!("scan_scale {label}: walltime = {} ms", elapsed.as_millis());
}

// --- Bench groups ----------------------------------------------------------

fn bench_10k_without_gitattributes(c: &mut Criterion) {
    let (tmp, mock) = fixture_10k_identical(false);
    let args = args_for(tmp.path());
    let mut group = c.benchmark_group("scan_scale/10k_without_gitattributes");
    group.sample_size(20);
    group.bench_function("build_report+serialize", |b| {
        b.iter(|| run_full(black_box(&args), black_box(&mock)));
    });
    group.finish();
}

fn bench_10k_with_100rule_gitattributes(c: &mut Criterion) {
    let (tmp, mock) = fixture_10k_identical(true);
    let args = args_for(tmp.path());
    let mut group = c.benchmark_group("scan_scale/10k_with_100rule_gitattributes");
    group.sample_size(20);
    group.bench_function("build_report+serialize", |b| {
        b.iter(|| run_full(black_box(&args), black_box(&mock)));
    });
    group.finish();
}

fn bench_100k_remote_only_no_gitattributes(c: &mut Criterion) {
    let (tmp, mock) = fixture_100k_remote_only();
    let args = args_for(tmp.path());
    dump_one_shot("100k_remote_only_no_gitattributes/oneshot", &args, &mock);
    let mut group = c.benchmark_group("scan_scale/100k_remote_only_no_gitattributes");
    group.sample_size(10);
    group.bench_function("build_report+serialize", |b| {
        b.iter(|| run_full(black_box(&args), black_box(&mock)));
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_10k_without_gitattributes,
    bench_10k_with_100rule_gitattributes,
    bench_100k_remote_only_no_gitattributes,
);
criterion_main!(benches);
