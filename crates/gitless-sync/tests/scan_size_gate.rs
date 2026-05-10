#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Phase 7.2 task O — end-to-end size gate scenarios via `build_report`.
//!
//! Pins the full-pipeline JSON output (`run_to_json` → `pre_entry_to_file`
//! → `serialize`) for the 4 scenarios in `spec-hash-and-normalize.md`
//! § Phase 7 — 큰 파일 처리 § 단위 테스트 시나리오. `try_size_gate`
//! boundary semantics are unit-tested in
//! `commands/scan/hash_local.rs::tests::size_gate_*`; this file pins the
//! integration plumbing on top.
//!
//! Scenarios:
//! 1. 49 MB local (below 50 MB) → normal hash + `Status::Identical`.
//! 2. 51 MB local (above 50 MB) → `Status::Failed` + `memory_exceeded` +
//!    `size_bytes` `53_477_376`.
//! 3. 101 MB local (above 100 MB) → `Status::Failed` + `file_too_large` +
//!    `size_bytes` `105_906_176`.
//! 4. 30 MB LFS-tracked path → `Status::Failed` + `lfs_pointer` +
//!    placeholder `{oid: "?", size: 0}`. LFS short-circuit (cascade
//!    priority 6) outranks the size gate (priority 8), so the body is
//!    never measured and `size_bytes` is omitted.
//!
//! Sparse files via `File::set_len` keep CI cheap — only scenario 1
//! actually allocates + SHA-1s the 49 MB body (size gate returns None
//! below threshold). Scenarios 2/3 short-circuit on `fs::metadata().len()`
//! before any read; scenario 4 short-circuits on path pattern. The plan's
//! `tests/fixtures/large-files/` hint is honored as runtime sparse files
//! in `TempDir` — checking in 100 MB+ binaries would bloat the repo for
//! data that's trivially regenerated from the size constants.

mod common;

use std::fs;

use serde_json::Value;
use tempfile::TempDir;

use common::{TestGhClient, args_for, ok_resp, run_to_json, tree_args};
use gitless_sync::shared::hash::blob_hash;

const EMPTY_TREES: &[u8] = br#"{"sha":"x","tree":[],"truncated":false}"#;

fn make_sparse(dir: &TempDir, name: &str, size: u64) {
    let path = dir.path().join(name);
    let f = fs::File::create(&path).unwrap();
    f.set_len(size).unwrap();
}

fn entry_at<'a>(json: &'a Value, path: &str) -> &'a Value {
    json["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"].as_str() == Some(path))
        .expect("entry not found in scan output")
}

#[test]
fn local_49mb_below_threshold_passes_through_normal_hash_to_status_identical() {
    // Sparse 49 MiB → fs::read 49 MiB of zeros → blob_hash. Matching remote
    // sha triggers the SHA-equality skip (G-003), so no Commits API stub
    // is registered (a stray Commits call would surface as `TestGhClient:
    // no stub registered` and fail the test).
    let dir = TempDir::new().unwrap();
    let n: u64 = 49 * 1024 * 1024;
    make_sparse(&dir, "data.bin", n);

    let zeros = vec![0u8; usize::try_from(n).unwrap()];
    let expected = blob_hash(&zeros);

    let mut mock = TestGhClient::new();
    let trees = format!(
        r#"{{"sha":"x","tree":[{{"path":"data.bin","mode":"100644","type":"blob","sha":"{expected}","size":{n}}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees.as_bytes()));

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    let e = entry_at(&json, "data.bin");

    assert_eq!(e["status"], "identical");
    assert_eq!(e["local_sha"], expected);
    assert_eq!(e["remote_sha"], expected);
    let obj = e.as_object().unwrap();
    assert!(
        !obj.contains_key("size_bytes"),
        "below-threshold path must omit size_bytes"
    );
    assert!(!obj.contains_key("failed_reason"));
    assert_eq!(json["summary"]["identical"], 1);
    assert_eq!(json["summary"]["failed"], 0);
}

#[test]
fn local_51mb_above_50mb_classifies_as_failed_with_memory_exceeded_reason() {
    // 53,477,376 bytes (51 MiB) — above 50 MB, below 100 MB. Local-only
    // path (no remote entry) so `try_remote_size_gate` returns None and
    // the local arm's metadata pre-flight fires `MemoryExceeded` BEFORE
    // any fs::read.
    let dir = TempDir::new().unwrap();
    let n: u64 = 51 * 1024 * 1024;
    assert_eq!(n, 53_477_376);
    make_sparse(&dir, "big.bin", n);

    let mut mock = TestGhClient::new();
    mock.stub(tree_args("o/r", "main"), ok_resp(EMPTY_TREES));

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    let e = entry_at(&json, "big.bin");

    assert_eq!(e["status"], "failed");
    assert_eq!(e["failed_reason"], "memory_exceeded");
    assert_eq!(e["size_bytes"], n);
    let obj = e.as_object().unwrap();
    assert!(!obj.contains_key("local_sha"));
    assert!(!obj.contains_key("lfs_pointer"));
    assert_eq!(json["summary"]["failed"], 1);
}

#[test]
fn local_101mb_above_100mb_classifies_as_failed_with_file_too_large_reason() {
    // 105,906,176 bytes (101 MiB) — above 100 MB, so `FileTooLarge` wins
    // the `try_size_gate` cascade over `MemoryExceeded`.
    let dir = TempDir::new().unwrap();
    let n: u64 = 101 * 1024 * 1024;
    assert_eq!(n, 105_906_176);
    make_sparse(&dir, "huge.bin", n);

    let mut mock = TestGhClient::new();
    mock.stub(tree_args("o/r", "main"), ok_resp(EMPTY_TREES));

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    let e = entry_at(&json, "huge.bin");

    assert_eq!(e["status"], "failed");
    assert_eq!(e["failed_reason"], "file_too_large");
    assert_eq!(e["size_bytes"], n);
    let obj = e.as_object().unwrap();
    assert!(!obj.contains_key("local_sha"));
    assert!(!obj.contains_key("lfs_pointer"));
    assert_eq!(json["summary"]["failed"], 1);
}

#[test]
fn local_lfs_tracked_path_outranks_size_gate_with_lfs_pointer_reason() {
    // 30 MiB sparse file matching `*.psd filter=lfs`. LFS classification
    // (`short_circuit` priority 6) fires before the size gate (priority
    // 8), so the body is never measured — `size_bytes` is omitted and
    // the entry surfaces with the `{oid: "?", size: 0}` placeholder.
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join(".gitattributes"),
        "*.psd filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    make_sparse(&dir, "cover.psd", 30 * 1024 * 1024);

    let mut mock = TestGhClient::new();
    mock.stub(tree_args("o/r", "main"), ok_resp(EMPTY_TREES));

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    let e = entry_at(&json, "cover.psd");

    assert_eq!(e["status"], "failed");
    assert_eq!(e["failed_reason"], "lfs_pointer");
    let obj = e.as_object().unwrap();
    assert!(
        !obj.contains_key("size_bytes"),
        "LFS short-circuit fires before size measurement"
    );
    assert_eq!(e["lfs_pointer"]["oid"], "?");
    assert_eq!(e["lfs_pointer"]["size"], 0);
}
