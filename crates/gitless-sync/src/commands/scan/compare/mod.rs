//! 4-state classification + `presence` / `diff_meaningful` decision logic.
//!
//! Phase 8 task H split per `spec-architecture.md` § Module 폴더 단위 정책:
//! - [`types`] — `Status`, `Presence`, `FailedReason`, `LfsPointer`,
//!   `FileEntry` + their serde round-trip tests.
//! - [`decisions`] — `classify` (4-state status) + `compare` (presence +
//!   `diff_meaningful`) decision functions and their unit tests.
//!
//! Caller imports stay `crate::commands::scan::compare::{Status, ...}` via
//! the re-exports below.

mod decisions;
mod types;

pub use decisions::{classify, compare};
pub use types::{FailedReason, FileEntry, LfsPointer, Presence, Status};
