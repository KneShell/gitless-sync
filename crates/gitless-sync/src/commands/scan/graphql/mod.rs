//! GraphQL backend for `fetch_last_commit_at` (Phase 4, ADR 0005/0006).
//!
//! Replaces the REST commits endpoint's N×round-trip pattern with a single
//! `gh api graphql` invocation per chunk of [`GRAPHQL_BATCH_SIZE`] paths.
//! Each chunk uses GraphQL alias batching: one alias = one
//! `history(first: 1, path: ...)` node, so all paths in a chunk are resolved
//! in a single round-trip and evaluated in parallel server-side. Per ADR
//! 0005, this backend deliberately does not use rayon — the alias batching
//! itself is the parallelism. Authentication, rate limiting, and transport
//! errors stay delegated to the `gh` subprocess (ADR 0001 + ADR 0002).
//!
//! Sub-modules: [`batch`] owns the entry point + query construction;
//! [`parse`] decodes the response envelope and maps `errors[]` codes.
//!
//! Error classification follows `spec-error-contracts.md` § GraphQL error
//! mapping: gh subprocess exit ≠ 0 routes through the same REST stderr
//! substring table (via [`crate::shared::github::map_gh_error`]); exit == 0
//! with a non-empty `errors[]` array routes through
//! [`crate::shared::error::map_graphql_error`] keyed off
//! `errors[0].extensions.code`.

mod batch;
mod parse;
mod query;

#[cfg(test)]
mod test_helpers;

pub(crate) use batch::fetch_last_commit_at_batch;

/// Number of alias entries packed into a single `gh api graphql` request.
///
/// Default 200 per `roadmap.md` § Phase 4 GraphQL batching, confirmed by
/// ADR 0007 (P6a raw data, 2026-05-07): at 13-path scale batch 100 vs 200
/// resolve to a single chunk and are functionally equivalent — measurement
/// noise dominated. yagni keeps the recommended ceiling. Any change requires
/// a coordinated update of this constant + `spec-github-api.md` § GraphQL
/// backend + ADR 0007.
pub(crate) const GRAPHQL_BATCH_SIZE: usize = 200;
