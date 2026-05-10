//! Case-collision detection for the comparison stage.
//!
//! Splits the cross-set check (path appearing on one side, case-folded
//! counterpart on the other) out of `pipeline.rs` to keep the orchestrator
//! under the 300-LOC gate. See `spec-domain-pitfalls.md` § Windows NTFS
//! local-side case detection and `spec-classification.md` § Path 정규화.
//!
//! Detection is symmetric: a path is a collision if it appears on exactly
//! one side AND a different-case sibling exists on the other side. Covers
//! the canonical scenario (case-insensitive volume swallows one of two
//! remote variants) and the diagonal scenario (local has only `foo.txt`,
//! remote has only `Foo.txt`) without extra wiring.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::shared::github::RemoteFile;

use super::walker::LocalFile;

/// Identify paths whose status should be promoted to `Failed` with reason
/// `case_collision`.
///
/// A path P is flagged when it appears on exactly one side AND the other
/// side has at least one path Q with `Q.lower() == P.lower()` and
/// `Q != P` (case-sensitive). Paths matched on both sides are returned to
/// the normal classifier.
pub(super) fn detect<'a>(
    all_paths: &BTreeSet<&'a str>,
    local_map: &HashMap<&'a str, &LocalFile>,
    remote_map: &HashMap<&'a str, &RemoteFile>,
) -> HashSet<String> {
    let local_ci: HashSet<String> = local_map.keys().map(|k| k.to_lowercase()).collect();
    let remote_ci: HashSet<String> = remote_map.keys().map(|k| k.to_lowercase()).collect();

    all_paths
        .iter()
        .filter_map(|p| {
            let in_local = local_map.contains_key(*p);
            let in_remote = remote_map.contains_key(*p);
            if in_local && in_remote {
                return None;
            }
            let folded = p.to_lowercase();
            let collides = (!in_local && local_ci.contains(&folded))
                || (!in_remote && remote_ci.contains(&folded));
            collides.then(|| (*p).to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::{TimeZone, Utc};

    use super::*;

    fn lf(name: &str) -> LocalFile {
        LocalFile {
            relative_path: name.to_string(),
            absolute_path: PathBuf::from(name),
            mtime: Utc.timestamp_opt(0, 0).unwrap(),
            is_symlink: false,
        }
    }

    fn rf(name: &str) -> RemoteFile {
        RemoteFile {
            path: name.to_string(),
            sha: format!("sha-{name}"),
            mode: "100644".to_string(),
            size: None,
        }
    }

    fn run(locals: &[LocalFile], remotes: &[RemoteFile]) -> HashSet<String> {
        let local_map: HashMap<&str, &LocalFile> = locals
            .iter()
            .map(|f| (f.relative_path.as_str(), f))
            .collect();
        let remote_map: HashMap<&str, &RemoteFile> =
            remotes.iter().map(|f| (f.path.as_str(), f)).collect();
        let mut all_paths: BTreeSet<&str> = BTreeSet::new();
        all_paths.extend(local_map.keys().copied());
        all_paths.extend(remote_map.keys().copied());
        detect(&all_paths, &local_map, &remote_map)
    }

    #[test]
    fn canonical_case_insensitive_volume_flags_remote_only_variant() {
        // Local volume swallowed `Foo.txt`; only `foo.txt` survived.
        let locals = [lf("foo.txt")];
        let remotes = [rf("Foo.txt"), rf("foo.txt")];
        let collisions = run(&locals, &remotes);
        assert_eq!(collisions.len(), 1);
        assert!(collisions.contains("Foo.txt"));
        // The matched path `foo.txt` is NOT flagged.
        assert!(!collisions.contains("foo.txt"));
    }

    #[test]
    fn local_has_both_cases_remote_has_one_flags_unmatched_local() {
        // Case-preserving (NTFS) local sees both; remote has only `Foo.txt`.
        let locals = [lf("Foo.txt"), lf("foo.txt")];
        let remotes = [rf("Foo.txt")];
        let collisions = run(&locals, &remotes);
        assert_eq!(collisions.len(), 1);
        assert!(collisions.contains("foo.txt"));
        assert!(!collisions.contains("Foo.txt"));
    }

    #[test]
    fn diagonal_case_mismatch_flags_both_paths() {
        // Local has only `foo.txt`, remote has only `Foo.txt` — neither side
        // has a case-sensitive match, but each side has a case-folded
        // sibling on the other.
        let locals = [lf("foo.txt")];
        let remotes = [rf("Foo.txt")];
        let collisions = run(&locals, &remotes);
        assert_eq!(collisions.len(), 2);
        assert!(collisions.contains("foo.txt"));
        assert!(collisions.contains("Foo.txt"));
    }

    #[test]
    fn matched_path_with_no_siblings_is_not_flagged() {
        let locals = [lf("foo.txt")];
        let remotes = [rf("foo.txt")];
        let collisions = run(&locals, &remotes);
        assert!(collisions.is_empty());
    }

    #[test]
    fn unique_paths_with_distinct_case_folded_forms_are_not_flagged() {
        let locals = [lf("alpha.md"), lf("Beta.md")];
        let remotes = [rf("gamma.md")];
        let collisions = run(&locals, &remotes);
        assert!(collisions.is_empty());
    }

    #[test]
    fn directory_segment_case_difference_is_flagged() {
        // Same logical file, different case in directory segment.
        let locals = [lf("Docs/note.md")];
        let remotes = [rf("docs/note.md")];
        let collisions = run(&locals, &remotes);
        assert_eq!(collisions.len(), 2);
        assert!(collisions.contains("Docs/note.md"));
        assert!(collisions.contains("docs/note.md"));
    }
}
