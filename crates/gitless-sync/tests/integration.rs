//! End-to-end integration tests for `gitless-sync`.
//!
//! **2026-05-06 (M2c)**: The previous mockito + `GITLESS_API_BASE`-based
//! scaffolding was removed when the `mockito` dev-dep was dropped (ADR 0002).
//! M4a/M4b will rewrite this file to call the library entry
//! `commands::scan::run_with_client(args, &MockGhClient)` directly, covering
//! PRD scenarios 1-4, 9 (M4a) and 10-15 (M4b).
