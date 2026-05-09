#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Real-file NFD/NFC fixture for `gitless-sync scan` (Phase 5 task P1).
//!
//! Creates actual on-disk files via `tempfile`, then asserts that the
//! walker → compare → output pipeline collapses NFD ↔ NFC variants of the
//! same logical path to a single `Status::Identical` entry. Pairs with the
//! unit-level synthetic fixtures in `walker.rs::tests` (task P) by adding
//! the real-filesystem dimension — the unit tests only feed `Path` values
//! to the helper, never touch the disk.
//!
//! Coverage by code path (per `walker.rs::tests::nfd_and_nfc_synthetic_paths_collapse_to_same_key`):
//! - **Hangul (LV/LVT algorithmic)**: `\u{1100}\u{1161}` ≡ `\u{AC00}` — exercises
//!   `unicode-normalization`'s composition algorithm for Hangul Jamo.
//! - **Latin ñ (decomposition table)**: `n\u{0303}` ≡ `\u{00F1}` — exercises the
//!   table-driven canonical composition path.
//!
//! Symmetry: the walker calls `to_nfc` on local paths (`walker.rs:92`) and
//! `shared/github/trees.rs` calls `to_nfc` on remote tree entry paths
//! (`shared/github/trees.rs:63/75/87`). Test 3 below is the only place we
//! exercise the remote-side normalization at integration level —
//! `scan_path_normalization.rs` only feeds NFC remote paths.
//!
//! Platform note: NTFS (Windows, primary target) and ext4 (Linux) preserve
//! filename bytes verbatim, so creating with NFD codepoints yields an
//! NFD-named file on disk. macOS HFS+ canonicalizes-on-write to NFD; APFS
//! preserves bytes. Either way, both sides converge to NFC via the
//! `unicode-normalization` crate, so the assertions hold on every platform
//! — they just exercise slightly different boundary conditions.

mod common;

use std::fs;

use tempfile::TempDir;

use common::{TestGhClient, args_for, lf_blob_hash, ok_resp, run_to_json, tree_args};

/// NFD jamo sequence — Hangul `가` decomposed into Choseong `ㄱ` + Jungseong `ㅏ`.
/// Composes algorithmically (LV) to `\u{AC00}`.
const NFD_HANGUL: &str = "\u{1100}\u{1161}.txt";
const NFC_HANGUL: &str = "\u{AC00}.txt";

/// NFD via decomposition table — Latin small `n` + combining tilde `\u{0303}`.
/// Composes to precomposed `\u{00F1}` (`ñ`).
const NFD_LATIN: &str = "n\u{0303}ame.txt";
const NFC_LATIN: &str = "\u{00F1}ame.txt";

#[test]
fn local_nfd_hangul_real_file_matches_remote_nfc_blob() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(NFD_HANGUL), "alpha\n").unwrap();
    let sha = lf_blob_hash("alpha\n");

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"{NFC_HANGUL}","mode":"100644","type":"blob","sha":"{sha}","size":6}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));
    // Identical entries skip the Commits API (G-003) — no commits stub.

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["identical"], 1);
    assert_eq!(json["summary"]["local_only_changed"], 0);
    assert_eq!(json["summary"]["remote_only_changed"], 0);
    assert_eq!(json["summary"]["drift"], 0);
    assert_eq!(json["summary"]["failed"], 0);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    // Path key is NFC-normalized regardless of which form is on disk.
    assert_eq!(files[0]["path"], NFC_HANGUL);
    assert_eq!(files[0]["status"], "identical");
}

#[test]
fn local_nfd_latin_n_tilde_real_file_matches_remote_nfc_blob() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(NFD_LATIN), "alpha\n").unwrap();
    let sha = lf_blob_hash("alpha\n");

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"{NFC_LATIN}","mode":"100644","type":"blob","sha":"{sha}","size":6}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["identical"], 1);
    assert_eq!(json["summary"]["failed"], 0);
    assert_eq!(json["summary"]["drift"], 0);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], NFC_LATIN);
    assert_eq!(files[0]["status"], "identical");
}

#[test]
fn local_nfc_real_file_matches_remote_nfd_blob() {
    // Symmetric direction: local NFC on disk, remote tree carries NFD bytes.
    // Exercises `shared/github/trees.rs::to_nfc` at integration level —
    // existing `scan_path_normalization.rs` only feeds NFC remote paths.
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(NFC_HANGUL), "alpha\n").unwrap();
    let sha = lf_blob_hash("alpha\n");

    let mut mock = TestGhClient::new();
    let trees_body = format!(
        r#"{{"sha":"x","tree":[{{"path":"{NFD_HANGUL}","mode":"100644","type":"blob","sha":"{sha}","size":6}}],"truncated":false}}"#
    );
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));

    let json = run_to_json(&args_for(dir.path(), "o/r"), &mock);
    assert_eq!(json["summary"]["identical"], 1);
    assert_eq!(json["summary"]["failed"], 0);
    assert_eq!(json["summary"]["drift"], 0);

    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], NFC_HANGUL);
    assert_eq!(files[0]["status"], "identical");
}
