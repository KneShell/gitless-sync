//! Tree-response entry iteration (pure).
//!
//! [`process_entries`] visits each entry from a parsed Trees response
//! and yields an [`Outcome`] enum stream — `Outcome::Blob` for
//! supported blob/symlink/submodule rows (delegated to
//! [`super::super::super::classify::classify_tree_entry`]) and
//! `Outcome::Subtree` for `type == "tree"` rows the orchestrator must
//! recurse into. The orchestrator ([`super::walk`]) consumes the
//! stream — push timing + cap checks live there.
//!
//! Splits the body iteration off `walk.rs` per `spec-architecture.md`
//! § Module 폴더 정책. Pure: no `GhClient`, no [`super::Descent`]
//! mutation, no `Result`. The iterator is `impl Iterator` so a wide
//! root tree (e.g. 500 001 blobs) does not allocate a parallel
//! `Vec<Outcome>` — the orchestrator advances one outcome at a time
//! and trips the post-push cap synchronously.

use super::super::super::classify::{RemoteFile, classify_tree_entry};
use super::super::super::parse::TreeResponse;

/// One tree-response entry classified into either a supported blob
/// ([`Outcome::Blob`]) the orchestrator should push, or a sub-tree
/// reference ([`Outcome::Subtree`]) the orchestrator should recurse
/// on. Unsupported entries (unknown mode bits, drop-warning rows from
/// [`classify_tree_entry`]) skip without producing an outcome.
pub(super) enum Outcome {
    Blob(RemoteFile),
    Subtree { sha: String, path: String },
}

/// Iterate `body.tree` and yield one [`Outcome`] per supported entry.
///
/// `path_prefix` joins with `/` (forward slash, per G-004) when
/// non-empty; root invocation passes `""` and entry paths come through
/// untouched.
///
/// Returns `impl Iterator` rather than `Vec<Outcome>` so the
/// orchestrator can short-circuit on the post-push cap without first
/// allocating an outcome per entry — important for the entries-cap
/// trip case where one root response carries 500 001 blobs.
pub(super) fn process_entries(
    body: TreeResponse,
    path_prefix: &str,
) -> impl Iterator<Item = Outcome> + '_ {
    body.tree.into_iter().filter_map(move |mut entry| {
        let full_path = if path_prefix.is_empty() {
            entry.path.clone()
        } else {
            format!("{path_prefix}/{}", entry.path)
        };
        if entry.entry_type == "tree" {
            Some(Outcome::Subtree {
                sha: entry.sha.clone(),
                path: full_path,
            })
        } else {
            entry.path = full_path;
            classify_tree_entry(entry).map(Outcome::Blob)
        }
    })
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
mod tests {
    use super::super::super::super::parse::parse_tree_body;
    use super::*;

    fn parse(body: &[u8]) -> TreeResponse {
        parse_tree_body(body).unwrap()
    }

    #[test]
    fn empty_tree_yields_no_outcomes() {
        let body = parse(br#"{"sha":"x","tree":[],"truncated":false}"#);
        assert_eq!(process_entries(body, "").count(), 0);
    }

    #[test]
    fn root_blob_yields_blob_outcome_with_unprefixed_path() {
        let body = parse(
            br#"{"sha":"r","tree":[{"path":"a.md","mode":"100644","type":"blob","sha":"b1"}],"truncated":false}"#,
        );
        let outcomes: Vec<_> = process_entries(body, "").collect();
        assert_eq!(outcomes.len(), 1);
        match &outcomes[0] {
            Outcome::Blob(rf) => {
                assert_eq!(rf.path, "a.md");
                assert_eq!(rf.sha, "b1");
            }
            Outcome::Subtree { .. } => panic!("expected Blob outcome"),
        }
    }

    #[test]
    fn tree_entry_yields_subtree_outcome_joined_with_prefix() {
        let body = parse(
            br#"{"sha":"r","tree":[{"path":"docs","mode":"040000","type":"tree","sha":"t1"}],"truncated":false}"#,
        );
        let outcomes: Vec<_> = process_entries(body, "src").collect();
        assert_eq!(outcomes.len(), 1);
        match &outcomes[0] {
            Outcome::Subtree { sha, path } => {
                assert_eq!(sha, "t1");
                assert_eq!(path, "src/docs");
            }
            Outcome::Blob(_) => panic!("expected Subtree outcome"),
        }
    }

    #[test]
    fn mixed_entries_preserve_input_order() {
        let body = parse(
            br#"{"sha":"r","tree":[
                {"path":"a.md","mode":"100644","type":"blob","sha":"b1"},
                {"path":"sub","mode":"040000","type":"tree","sha":"t1"},
                {"path":"z.md","mode":"100644","type":"blob","sha":"b2"}
            ],"truncated":false}"#,
        );
        let outcomes: Vec<_> = process_entries(body, "").collect();
        assert_eq!(outcomes.len(), 3);
        assert!(matches!(outcomes[0], Outcome::Blob(_)));
        assert!(matches!(outcomes[1], Outcome::Subtree { .. }));
        assert!(matches!(outcomes[2], Outcome::Blob(_)));
    }

    #[test]
    fn unsupported_blob_mode_drops_without_outcome() {
        let body = parse(
            br#"{"sha":"r","tree":[{"path":"weird","mode":"100664","type":"blob","sha":"b1"}],"truncated":false}"#,
        );
        assert_eq!(process_entries(body, "").count(), 0);
    }
}
