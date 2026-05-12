#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Task L — Finding 2 v1.6 integration test: `--summary-only` + `hash_io`
//! wire shape lock. Local file IO failure (chmod 0o000) drives
//! `try_hash_local` → `Err` → `build_local_state`'s Err arm lands
//! `PreState::Failed { failed_reason: Some(FailedReason::HashIo), .. }`,
//! the `--summary-only` projection emits `path` + `presence` +
//! `failed_reason: "hash_io"` 3-field minimal row per
//! `spec-output-schema.md` § v1.6.
//!
//! `LocalOnly` fixture — remote tree is empty so the path lands only on
//! the local arm; `try_short_circuit_failed` returns `None` (no priority-1
//! through priority-7 reason matches a plain `ghost.md`), and `hash_pass`
//! invokes `try_hash_local` against the chmod-0 file. `RemoteOnly` would
//! never reach `hash_local`; `Both` keeps the test surface portable for
//! the assertion but enlarges the gh stub set (blob fetch path), so the
//! minimal fixture stays on `LocalOnly` for a single trees stub.
//!
//! Unix-only file: the chmod-0 read-denial fixture assumes POSIX
//! permission semantics. Windows production parity for the `hash_io`
//! wire emit is locked at the unit-test layer
//! (`pipeline/hash_pass/local.rs::build_local_state_marks_unreadable_local_as_failed_with_hash_io_reason`).
//! Single `#![cfg(unix)]` file gate keeps Linux CI runner happy without
//! per-line cfg fences (G-018 사례 A pattern).

use std::fs;
use std::os::unix::fs::PermissionsExt;

use tempfile::TempDir;

mod common;

use common::{TestGhClient, args_for, ok_resp, run_to_json, tree_args};

#[test]
fn summary_only_emits_minimal_failed_entry_for_hash_io_pitfall() {
    let dir = TempDir::new().unwrap();
    let local_path = dir.path().join("ghost.md");
    fs::write(&local_path, "payload that will never be read").unwrap();
    // chmod 0o000 — `fs::metadata` still succeeds (parent dir at 0o755
    // gives stat access), but `fs::read` returns EACCES → `try_hash_local`
    // returns `Err` → `build_local_state` emits `FailedReason::HashIo`.
    fs::set_permissions(&local_path, fs::Permissions::from_mode(0o000)).unwrap();

    // Empty remote tree → `Presence::LocalOnly` for `ghost.md`. No blob /
    // commits stub needed: short-circuit pre-hash, no Trees match, no
    // hash_remote work; the local arm is the only producer of the entry.
    let mut mock = TestGhClient::new();
    let trees_body = r#"{"sha":"x","tree":[],"truncated":false}"#;
    mock.stub(tree_args("o/r", "main"), ok_resp(trees_body.as_bytes()));

    let mut args = args_for(dir.path(), "o/r");
    args.summary_only = true;

    let json = run_to_json(&args, &mock);

    // `chmod 0o000` may leave the file undeletable by `TempDir::drop` if
    // unlink were blocked, but POSIX unlink is gated by parent-dir
    // permission, not file mode — restore anyway for hygiene (and so a
    // root-runner accidental pass surfaces obviously by writing the file
    // out again before this point).
    fs::set_permissions(&local_path, fs::Permissions::from_mode(0o644)).unwrap();

    assert_eq!(json["summary"]["failed"], 1);
    let files = json["files"]
        .as_array()
        .expect("files[] present when failed > 0 in summary-only mode");
    assert_eq!(files.len(), 1);

    // v1.6 minimal entry shape — `path` + `presence` + `failed_reason`
    // (hash_io explicit). v1.5's 2-field special case (presence + path
    // only when `failed_reason` was the `None` sentinel) is gone.
    let entry = files[0].as_object().expect("entry object");
    assert_eq!(
        entry.len(),
        3,
        "v1.6 hash_io entry must emit exactly 3 keys"
    );
    assert_eq!(entry["path"].as_str().unwrap(), "ghost.md");
    assert_eq!(entry["failed_reason"], "hash_io");
    assert!(
        entry.contains_key("presence"),
        "presence key required for caller G2 (presence/status orthogonal) branching"
    );

    for stripped in [
        "status",
        "local_sha",
        "remote_sha",
        "local_mtime",
        "remote_last_commit_at",
        "is_binary",
        "mode",
        "diff_meaningful",
        "lfs_pointer",
        "size_bytes",
    ] {
        assert!(
            !entry.contains_key(stripped),
            "detail key {stripped} must not leak in summary-only mode"
        );
    }
}
