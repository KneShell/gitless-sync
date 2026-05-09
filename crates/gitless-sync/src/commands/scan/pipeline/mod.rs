//! Three-pass `assemble_entries` (hash → fetch commits → classify).
//!
//! Phase 5.13 task DD split — four sub-modules per
//! `spec-architecture.md` § Module 폴더 단위 정책:
//! - [`short_circuit`] — pre-hash failure cascade + [`ClassifyContext`].
//! - [`hash_pass`] — Pass 1: hash local + build [`PreEntry`].
//! - [`finalize`] — Pass 2/3: extract commit paths + classify into `FileEntry`.
//! - [`orchestrator`] — entry: stitches the three passes + owns
//!   [`GitHubContext`] / [`assemble_entries`].
//!
//! Caller imports stay `crate::commands::scan::pipeline::{GitHubContext,
//! assemble_entries}` via the re-exports below.

mod finalize;
mod hash_pass;
mod orchestrator;
mod short_circuit;

pub(crate) use orchestrator::{GitHubContext, assemble_entries};
