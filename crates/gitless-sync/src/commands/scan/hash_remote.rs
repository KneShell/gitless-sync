//! Remote-side size gate — short-circuits 50 MB+ blobs using only the
//! Trees response size field, BEFORE any local read or blob fetch.
//!
//! `scan` calls `fetch_blob` only in `pipeline::normalize_pass` for
//! sha-mismatch Hashed entries (cosmetic drift verification per
//! `spec-hash-and-normalize.md` § 원격 측 비교, Phase 8 task I).
//! This module's role is the size pre-flight: mark oversize remote
//! entries `Status::Failed` and skip the local hash pass entirely.
//! `diff` (future) will reuse `fetch_blob_with_size_gate` at its own
//! caller plumbing point.
//!
//! Cascade priority (`spec-hash-and-normalize.md` § 우선순위):
//!  1–7. `pipeline::short_circuit` (path/mode-only, no size).
//!  8.  `file_too_large` / `memory_exceeded` — this module (remote
//!      pre-flight) and `super::hash_local::try_size_gate` (local
//!      pre-flight). Remote runs first in `pipeline::hash_pass` since it
//!      is cheaper (in-memory check, no `fs::metadata` syscall).
//!  9.  `Encoding` — post-read in `try_hash_local`.
//!
//! 50 / 100 MB constants mirror `super::hash_local`; `shared/` could host
//! a single source of truth, but a `shared/limits.rs` extraction is
//! deferred until a third caller appears (yagni).
//!
//! Called from `pipeline::hash_pass::build_one_pre_entry` after the
//! `try_short_circuit_failed` cascade returns `None` and before any local
//! arm runs (Phase 7.2 task N).

use super::compare::FailedReason;
use crate::shared::github::RemoteFile;

/// 100 MB — GitHub Blobs API hard limit. Mirror of
/// `super::hash_local::FILE_TOO_LARGE_BYTES`.
pub(super) const FILE_TOO_LARGE_BYTES: u64 = 100 * 1024 * 1024;
/// 50 MB — tool memory safety threshold. Mirror of
/// `super::hash_local::MEMORY_EXCEEDED_BYTES`.
pub(super) const MEMORY_EXCEEDED_BYTES: u64 = 50 * 1024 * 1024;

/// Inspect a remote entry's Trees-response size; promote 50 MB+ blobs to
/// `Status::Failed` BEFORE any local read or blob fetch.
///
/// Returns `None` (proceed to local hash pass) for any of:
/// - `remote` is `None` (local-only path)
/// - `remote.size` is `None` (submodule `type=commit` carries no size)
/// - size within the safe range (≤ 50 MB)
///
/// Returns `Some((reason, size))` with `FileTooLarge` (> 100 MB) winning
/// over `MemoryExceeded` (> 50 MB) per cascade priority.
pub(super) fn try_remote_size_gate(remote: Option<&RemoteFile>) -> Option<(FailedReason, u64)> {
    let size = remote?.size?;
    if size > FILE_TOO_LARGE_BYTES {
        Some((FailedReason::FileTooLarge, size))
    } else if size > MEMORY_EXCEEDED_BYTES {
        Some((FailedReason::MemoryExceeded, size))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rf(size: Option<u64>) -> RemoteFile {
        RemoteFile {
            path: "x".to_string(),
            sha: "x".to_string(),
            mode: "100644".to_string(),
            size,
        }
    }

    #[test]
    fn try_remote_size_gate_returns_none_for_missing_remote() {
        assert_eq!(try_remote_size_gate(None), None);
    }

    #[test]
    fn try_remote_size_gate_returns_none_for_remote_without_size_field() {
        // Submodule case — `type=commit` entries carry no size in Trees
        // response, so `RemoteFile.size == None`. Gate must let them
        // through (the submodule arm of `short_circuit` already promoted
        // these before the gate runs, but the gate stays defensive).
        let r = rf(None);
        assert_eq!(try_remote_size_gate(Some(&r)), None);
    }

    #[test]
    fn try_remote_size_gate_passes_at_50mb_exact() {
        // Strict `>` — 50 MB exact passes (only over-50 fails).
        let r = rf(Some(MEMORY_EXCEEDED_BYTES));
        assert_eq!(try_remote_size_gate(Some(&r)), None);
    }

    #[test]
    fn try_remote_size_gate_promotes_memory_exceeded_just_over_50mb() {
        let n = MEMORY_EXCEEDED_BYTES + 1;
        let r = rf(Some(n));
        assert_eq!(
            try_remote_size_gate(Some(&r)),
            Some((FailedReason::MemoryExceeded, n))
        );
    }

    #[test]
    fn try_remote_size_gate_returns_memory_exceeded_at_100mb_exact() {
        // 100 MB exact is over 50 (memory_exceeded fires), not over 100
        // (file_too_large does not fire) — strict `>` boundary mirrors
        // `super::hash_local::try_size_gate`.
        let r = rf(Some(FILE_TOO_LARGE_BYTES));
        assert_eq!(
            try_remote_size_gate(Some(&r)),
            Some((FailedReason::MemoryExceeded, FILE_TOO_LARGE_BYTES))
        );
    }

    #[test]
    fn try_remote_size_gate_promotes_file_too_large_just_over_100mb() {
        let n = FILE_TOO_LARGE_BYTES + 1;
        let r = rf(Some(n));
        assert_eq!(
            try_remote_size_gate(Some(&r)),
            Some((FailedReason::FileTooLarge, n))
        );
    }

    #[test]
    fn try_remote_size_gate_prefers_file_too_large_over_memory_exceeded() {
        // 200 MB is over both thresholds — `FileTooLarge` arm wins per
        // cascade priority (mirror of `super::hash_local::try_size_gate`).
        let n = 200 * 1024 * 1024;
        let r = rf(Some(n));
        assert_eq!(
            try_remote_size_gate(Some(&r)),
            Some((FailedReason::FileTooLarge, n))
        );
    }
}
