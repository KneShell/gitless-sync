//! Benchmarks for the `.gitattributes` parser per-file glob match path.
//!
//! Task X (Phase 5) — measure `GitAttributes::match_path` over a 10K-path /
//! 100-rule simulated vault, dump P50/P95/P99 to stderr for the baseline doc
//! at `docs/research/phase5-gitattributes-bench.md`. R3 owns the regression
//! gate; this bench is record-only.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fmt::Write as _;
use std::fs;
use std::hint::black_box;
use std::time::Instant;

use criterion::{Criterion, criterion_group, criterion_main};
use tempfile::TempDir;

use gitless_sync::shared::gitattributes::GitAttributes;

const RULES: usize = 100;
const PATHS: usize = 10_000;

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

fn build_paths(count: usize) -> Vec<String> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        match i % 4 {
            0 => out.push(format!("dir{i:03}/file{i:03}.ext{:03}", i % RULES)),
            1 => out.push(format!(
                "dir{:03}/sub{:03}/file{i:03}.bin",
                i / 100,
                i % 100
            )),
            2 => out.push(format!("path{i:03}/file.txt")),
            _ => out.push(format!("misc/dir{:03}/file{:03}.dat", i % 100, i)),
        }
    }
    out
}

fn fixture() -> (TempDir, GitAttributes, Vec<String>) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let content = build_gitattributes_content(RULES);
    fs::write(tmp.path().join(".gitattributes"), content).expect("write .gitattributes");
    let sub = tmp.path().join("sub");
    fs::create_dir(&sub).expect("create sub");
    fs::write(sub.join(".gitattributes"), "*.bin binary\n").expect("write sub .gitattributes");
    let attrs = GitAttributes::load(tmp.path()).expect("load .gitattributes");
    let paths = build_paths(PATHS);
    (tmp, attrs, paths)
}

fn dump_p95(attrs: &GitAttributes, paths: &[String]) {
    let mut samples = Vec::with_capacity(paths.len());
    for path in paths {
        let start = Instant::now();
        let _ = black_box(attrs.match_path(black_box(path)));
        samples.push(start.elapsed().as_nanos());
    }
    samples.sort_unstable();
    let n = samples.len();
    let p50 = samples[n / 2];
    let p95 = samples[(n * 95) / 100];
    let p99 = samples[(n * 99) / 100];
    let max = *samples.last().unwrap_or(&0);
    let mean = samples.iter().sum::<u128>() / (n as u128);
    eprintln!(
        "match_path raw nanos: P50={p50} MEAN={mean} P95={p95} P99={p99} MAX={max} N={n} \
         RULES={RULES} PATHS={PATHS}"
    );
}

fn bench_match_path(c: &mut Criterion) {
    let (_tmp, attrs, paths) = fixture();
    dump_p95(&attrs, &paths);
    let mut idx = 0_usize;
    c.bench_function("match_path/100rules/10K_paths_round_robin", |b| {
        b.iter(|| {
            let path = &paths[idx % paths.len()];
            idx = idx.wrapping_add(1);
            black_box(attrs.match_path(black_box(path)));
        });
    });
}

criterion_group!(benches, bench_match_path);
criterion_main!(benches);
