//! Library surface for `gitless-sync`.
//!
//! Exists so integration tests under `tests/` can call command entry points
//! directly with a stubbed [`shared::gh::GhClient`] implementation. The binary
//! at `src/main.rs` re-uses these modules through `gitless_sync::*`.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod commands;
pub mod shared;
