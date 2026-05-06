//! Library surface for `gitless-sync`.
//!
//! Exists so integration tests under `tests/` can call command entry points
//! directly with a stubbed [`shared::gh::GhClient`] implementation. The binary
//! at `src/main.rs` re-uses these modules through `gitless_sync::*`.

pub mod commands;
pub mod shared;
