//! Sub-tree fallback budget caps (Phase 7).
//!
//! Holds [`Budget`] + the two hard caps from `spec-github-api.md`
//! § Trees truncation handling § 한도 상수. Task D adds the recursive
//! descent (`fetch_subtree_recursive`) in this same file; the caps +
//! counter live here so the recursion reads/writes them inline.
//!
//! The `allow(dead_code)` markers fall away as tasks C/D/E wire this
//! module into `super::fetch_tree_with_fallback`.

/// Hard cap on `gh api ...trees/{sub_tree_sha}` calls during a single
/// sub-tree fallback recovery. Exceeding aborts with
/// [`crate::shared::error::GitlessError::TreesTruncated`] (G-002
/// no-partial-result policy).
#[allow(dead_code)]
pub(super) const MAX_TREE_CALL_BUDGET: u32 = 1000;

/// Hard cap on cumulative entries during sub-tree fallback. Compared
/// against `Vec::<RemoteFile>::len()`. Exceeding aborts with
/// [`crate::shared::error::GitlessError::TreesTruncated`] (memory
/// safety).
#[allow(dead_code)]
pub(super) const MAX_TREE_ENTRIES: usize = 500_000;

/// Mutable counter advanced by the recursive descent. Read by the call
/// budget check; incremented after each `gh api` call. Initial value
/// is `0`.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub(super) struct Budget {
    calls_used: u32,
}

#[allow(dead_code)]
impl Budget {
    pub(super) const fn new() -> Self {
        Self { calls_used: 0 }
    }
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
mod tests {
    use super::{Budget, MAX_TREE_CALL_BUDGET, MAX_TREE_ENTRIES};

    #[test]
    fn cap_constants_match_spec_values() {
        assert_eq!(MAX_TREE_CALL_BUDGET, 1000);
        assert_eq!(MAX_TREE_ENTRIES, 500_000);
    }

    #[test]
    fn budget_new_starts_at_zero_calls_used() {
        assert_eq!(Budget::new().calls_used, 0);
    }

    #[test]
    fn budget_default_matches_new() {
        assert_eq!(Budget::default().calls_used, Budget::new().calls_used);
    }
}
