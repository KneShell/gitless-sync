//! GitHub Trees API client (recursive=1).
//!
//! Phase 5.13 task CC split — three sub-modules per
//! `spec-architecture.md` § Module 폴더 단위 정책:
//! - [`parse`] — wire-level deserialization + truncation guard.
//! - [`classify`] — tree-entry → [`RemoteFile`] mode-bit dispatch (pure).
//! - [`fetch`] — orchestrator (`GhClient` → error map → parse → classify).
//!
//! Caller imports stay `crate::shared::github::{RemoteFile, fetch_tree}`
//! via the parent's existing re-exports in `shared/github/mod.rs`.

mod classify;
mod fallback;
mod fetch;
mod parse;

pub use classify::RemoteFile;
pub(crate) use fetch::fetch_tree;
