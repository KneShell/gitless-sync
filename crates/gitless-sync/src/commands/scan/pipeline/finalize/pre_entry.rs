//! Pass 3 — `pre_entry_to_file` + `finalize_entries`.
//!
//! Each `PreEntry` resolves to a `FileEntry` via `classify` (4-state status
//! decision) + `compare` (presence + `diff_meaningful`, Phase 8 task H/I).
//! `presence` flows straight from `PreEntry` (covers `Failed` entries where
//! the SHA-derived presence in `compare()` would lose information). The
//! `Hashed` arm consults the upstream `normalize_eq_map` to feed `compare`'s
//! third arg per `spec-output-schema.md` § v1.3.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::super::hash_pass::{PreEntry, PreState};
use crate::commands::scan::compare::{FileEntry, Status, classify, compare};
use crate::commands::scan::lfs;
use crate::commands::scan::output::Summary;

struct CompareCtx<'a> {
    commit_map: &'a HashMap<String, DateTime<Utc>>,
    normalize_eq_map: &'a HashMap<String, bool>,
}

pub(in super::super) fn finalize_entries(
    pending: Vec<PreEntry>,
    commit_map: &HashMap<String, DateTime<Utc>>,
    normalize_eq_map: &HashMap<String, bool>,
) -> (Vec<FileEntry>, Summary, usize) {
    let mut entries: Vec<FileEntry> = Vec::with_capacity(pending.len());
    let mut summary = Summary::default();
    let mut failed_count = 0usize;
    let ctx = CompareCtx {
        commit_map,
        normalize_eq_map,
    };

    for pre in pending {
        let entry = pre_entry_to_file(pre, &ctx, &mut summary, &mut failed_count);
        entries.push(entry);
    }

    (entries, summary, failed_count)
}

fn pre_entry_to_file(
    pre: PreEntry,
    ctx: &CompareCtx<'_>,
    summary: &mut Summary,
    failed_count: &mut usize,
) -> FileEntry {
    match &pre.state {
        PreState::Failed { .. } => {
            summary.failed += 1;
            *failed_count += 1;
            failed_to_file_entry(pre)
        }
        PreState::Hashed { .. } => hashed_to_file_entry(pre, ctx, summary),
    }
}

fn failed_to_file_entry(pre: PreEntry) -> FileEntry {
    let PreEntry {
        path,
        mode,
        presence,
        state,
    } = pre;
    let PreState::Failed {
        remote_sha,
        local_mtime,
        failed_reason,
        is_binary,
        size_bytes,
    } = state
    else {
        unreachable!("failed_to_file_entry called with non-Failed state");
    };
    FileEntry {
        path,
        status: Status::Failed,
        presence,
        local_sha: None,
        remote_sha,
        local_mtime,
        remote_last_commit_at: None,
        is_binary,
        mode,
        diff_meaningful: None,
        lfs_pointer: lfs::placeholder_pointer_for(failed_reason),
        failed_reason,
        size_bytes,
    }
}

fn hashed_to_file_entry(pre: PreEntry, ctx: &CompareCtx<'_>, summary: &mut Summary) -> FileEntry {
    let PreEntry {
        path,
        mode,
        presence,
        state,
    } = pre;
    let PreState::Hashed {
        local_sha,
        remote_sha,
        local_mtime,
        is_binary,
    } = state
    else {
        unreachable!("hashed_to_file_entry called with non-Hashed state");
    };
    let remote_last_commit_at = ctx.commit_map.get(path.as_str()).copied();
    let status = classify(
        local_sha.as_deref(),
        remote_sha.as_deref(),
        local_mtime,
        remote_last_commit_at,
    );
    let normalize_equal = ctx.normalize_eq_map.get(path.as_str()).copied();
    let (_, diff_meaningful) =
        compare(local_sha.as_deref(), remote_sha.as_deref(), normalize_equal);
    summary.tally(status);
    FileEntry {
        path,
        status,
        presence,
        local_sha,
        remote_sha,
        local_mtime,
        remote_last_commit_at,
        is_binary,
        mode,
        diff_meaningful,
        failed_reason: None,
        lfs_pointer: None,
        size_bytes: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::scan::compare::{FailedReason, Presence};
    use crate::commands::scan::test_helpers::mtime;

    fn hashed(path: &str, local: Option<&str>, remote: Option<&str>) -> PreEntry {
        let presence = match (local.is_some(), remote.is_some()) {
            (true, true) => Presence::Both,
            (true, false) => Presence::LocalOnly,
            (false, true) => Presence::RemoteOnly,
            _ => unreachable!(),
        };
        PreEntry {
            path: path.to_string(),
            mode: "100644".to_string(),
            presence,
            state: PreState::Hashed {
                local_sha: local.map(str::to_string),
                remote_sha: remote.map(str::to_string),
                local_mtime: Some(mtime(1_700_000_000)),
                is_binary: false,
            },
        }
    }

    fn failed_pre(reason: Option<FailedReason>, is_binary: bool) -> PreEntry {
        PreEntry {
            path: "x".to_string(),
            mode: "100644".to_string(),
            presence: Presence::Both,
            state: PreState::Failed {
                remote_sha: Some("rs".to_string()),
                local_mtime: Some(mtime(1_700_000_000)),
                failed_reason: reason,
                is_binary,
                size_bytes: None,
            },
        }
    }

    fn run_with(
        pre: PreEntry,
        cm: &HashMap<String, DateTime<Utc>>,
        nm: &HashMap<String, bool>,
    ) -> (FileEntry, Summary, usize) {
        let mut s = Summary::default();
        let mut f = 0usize;
        let ctx = CompareCtx {
            commit_map: cm,
            normalize_eq_map: nm,
        };
        let entry = pre_entry_to_file(pre, &ctx, &mut s, &mut f);
        (entry, s, f)
    }

    fn run(pre: PreEntry) -> (FileEntry, Summary, usize) {
        run_with(pre, &HashMap::new(), &HashMap::new())
    }

    // === Phase 8 task J — 6-scenario v1.3 entry-level lock ============
    // spec-output-schema.md § v1.3 Acceptance Criteria — (Status, Presence,
    // diff_meaningful) shape per side combination. Regression pin against
    // eval F1/F2 friction.

    #[test]
    fn scenario_identical_presence_both_dm_some_false() {
        let (entry, summary, _) = run(hashed("a.md", Some("x"), Some("x")));
        assert_eq!(entry.status, Status::Identical);
        assert_eq!(entry.presence, Presence::Both);
        assert_eq!(entry.diff_meaningful, Some(false));
        assert_eq!(summary.identical, 1);
    }

    #[test]
    fn scenario_local_only_changed_both_presence_both_dm_some_true() {
        // Both shas exist, local newer than remote → Status::LocalOnlyChanged
        // with presence=both (eval F2 case ii). normalize-diff → dm=Some(true).
        let cm = HashMap::from([("a.md".to_string(), mtime(1_000_000_000))]);
        let nm = HashMap::from([("a.md".to_string(), false)]);
        let (entry, ..) = run_with(hashed("a.md", Some("l"), Some("r")), &cm, &nm);
        assert_eq!(entry.status, Status::LocalOnlyChanged);
        assert_eq!(entry.presence, Presence::Both);
        assert_eq!(entry.diff_meaningful, Some(true));
    }

    #[test]
    fn scenario_local_only_presence_local_only_dm_none() {
        let (entry, ..) = run(hashed("a.md", Some("l"), None));
        assert_eq!(entry.status, Status::LocalOnlyChanged);
        assert_eq!(entry.presence, Presence::LocalOnly);
        assert_eq!(entry.diff_meaningful, None);
    }

    #[test]
    fn scenario_remote_only_presence_remote_only_dm_none() {
        let (entry, ..) = run(hashed("a.md", None, Some("r")));
        assert_eq!(entry.status, Status::RemoteOnlyChanged);
        assert_eq!(entry.presence, Presence::RemoteOnly);
        assert_eq!(entry.diff_meaningful, None);
    }

    #[test]
    fn scenario_drift_presence_both_dm_some_true() {
        // sha differ + remote_last_commit_at absent → Status::Drift fallthrough.
        // normalize-diff (eq=false) → dm=Some(true).
        let nm = HashMap::from([("a.md".to_string(), false)]);
        let (entry, ..) = run_with(hashed("a.md", Some("l"), Some("r")), &HashMap::new(), &nm);
        assert_eq!(entry.status, Status::Drift);
        assert_eq!(entry.presence, Presence::Both);
        assert_eq!(entry.diff_meaningful, Some(true));
    }

    #[test]
    fn scenario_failed_presence_both_dm_none() {
        // scenario 6 — Failed: presence=both, dm=None. Submodule (short-circuit),
        // Encoding (EE is_binary preservation), LfsPointer (placeholder companion)
        // arms covered. `lfs::placeholder_pointer_for` itself in lfs.rs.
        let (entry, _, failed) = run(failed_pre(Some(FailedReason::Submodule), false));
        assert_eq!(entry.status, Status::Failed);
        assert_eq!(entry.presence, Presence::Both);
        assert_eq!(entry.diff_meaningful, None);
        assert!(entry.lfs_pointer.is_none());
        assert_eq!(failed, 1);
        let (entry, ..) = run(failed_pre(Some(FailedReason::Encoding), true));
        assert!(entry.is_binary);
        let (entry, ..) = run(failed_pre(Some(FailedReason::LfsPointer), false));
        let pointer = entry.lfs_pointer.expect("lfs_pointer placeholder");
        assert_eq!(pointer.oid, "?");
        assert_eq!(pointer.size, 0);
    }

    #[test]
    fn drift_normalize_equal_emits_dm_false_unknown_emits_none() {
        // F1 BOM/encoding-only drift: shas differ but normalize-equal → dm=Some(false).
        // Map absent (compute failure / single-side) → dm=None (don't guess).
        let nm = HashMap::from([("a.md".to_string(), true)]);
        let (entry, ..) = run_with(hashed("a.md", Some("l"), Some("r")), &HashMap::new(), &nm);
        assert_eq!(entry.diff_meaningful, Some(false));
        let (entry, ..) = run(hashed("b.md", Some("l"), Some("r")));
        assert_eq!(entry.diff_meaningful, None);
    }

    #[test]
    fn finalize_entries_aggregates_summary_and_carries_presence_through() {
        // Multi-entry rollup — summary tally + per-entry presence flow.
        let pending = vec![
            hashed("same.md", Some("abc"), Some("abc")),
            failed_pre(Some(FailedReason::Symlink), false),
            hashed("local-only.md", Some("l"), None),
        ];
        let (entries, summary, failed) =
            finalize_entries(pending, &HashMap::new(), &HashMap::new());
        assert_eq!(entries.len(), 3);
        assert_eq!(summary.identical, 1);
        assert_eq!(summary.local_only_changed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(failed, 1);
        assert_eq!(entries[2].presence, Presence::LocalOnly);
    }
}
