//! Sub-tree fallback recursive descent (Phase 7).
//!
//! Module-folder split per `spec-architecture.md` § Module 폴더 정책 — task
//! F's two new cap-trip unit tests pushed the previous single-file
//! `recursive.rs` past the 300-LOC gate, so the production logic split
//! into:
//! - [`walk::fetch_subtree_recursive`] — orchestrator (pre-call cap
//!   checks → `gh api ...trees/{sub_tree_sha}` → parse → consume the
//!   [`iter::Outcome`] stream → push blobs + recurse on sub-trees).
//! - [`iter::process_entries`] — pure, allocation-free iterator that
//!   classifies each tree-response entry into an [`iter::Outcome`].
//!
//! [`Descent`] lives in this `mod.rs` so both sub-modules share the
//! per-frame state without re-declaring the struct.

mod iter;
mod walk;

use super::super::classify::RemoteFile;
use super::Budget;
use crate::shared::gh::GhClient;

pub(in super::super) use walk::fetch_subtree_recursive;

/// Bundle of state threaded through every recursive frame:
/// the gh client + repo (immutable across the descent) plus the
/// shared accumulators (`entries`, `budget`). All four share one
/// lifetime so caller code can build a `Descent` once and reborrow
/// it on each recursive call.
pub(in super::super) struct Descent<'a, C: GhClient> {
    pub(in super::super) client: &'a C,
    pub(in super::super) repo: &'a str,
    pub(in super::super) entries: &'a mut Vec<RemoteFile>,
    pub(in super::super) budget: &'a mut Budget,
}
