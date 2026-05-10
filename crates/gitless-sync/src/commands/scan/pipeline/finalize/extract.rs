//! Pass 2 — paths that still need a Commits API call.
//!
//! Filters `Hashed` pre-entries where both shas exist AND differ; the
//! Commits API call is skipped for everything else (G-003 contract:
//! identical / single-side / Failed entries don't need a timestamp).

use super::super::hash_pass::{PreEntry, PreState};

pub(in super::super) fn extract_commit_paths(pending: &[PreEntry]) -> Vec<String> {
    pending
        .iter()
        .filter_map(|p| match &p.state {
            PreState::Hashed {
                local_sha: Some(l),
                remote_sha: Some(r),
                ..
            } if l != r => Some(p.path.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::scan::compare::{FailedReason, Presence};
    use crate::commands::scan::pipeline::hash_pass::{PreEntry, PreState};
    use crate::commands::scan::test_helpers::mtime;

    fn hashed(path: &str, local: Option<&str>, remote: Option<&str>) -> PreEntry {
        let presence = match (local.is_some(), remote.is_some()) {
            (true, true) => Presence::Both,
            (true, false) => Presence::LocalOnly,
            (false, true) => Presence::RemoteOnly,
            (false, false) => unreachable!(),
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

    fn failed(path: &str, reason: FailedReason) -> PreEntry {
        PreEntry {
            path: path.to_string(),
            mode: "120000".to_string(),
            presence: Presence::Both,
            state: PreState::Failed {
                remote_sha: Some("rs".to_string()),
                local_mtime: Some(mtime(1_700_000_000)),
                failed_reason: Some(reason),
                is_binary: false,
                size_bytes: None,
            },
        }
    }

    #[test]
    fn extract_commit_paths_returns_only_hashed_entries_with_differing_shas() {
        // Pass 2 contract — identical, local-only, remote-only, and Failed
        // entries all skip the Commits API call. Only the (Some, Some, neq)
        // case needs the timestamp.
        let pending = vec![
            hashed("identical.md", Some("abc"), Some("abc")),
            hashed("changed.md", Some("local-sha"), Some("remote-sha")),
            hashed("local-only.md", Some("local-sha"), None),
            hashed("remote-only.md", None, Some("remote-sha")),
            failed("failed.md", FailedReason::Symlink),
        ];
        let paths = extract_commit_paths(&pending);
        assert_eq!(paths, vec!["changed.md".to_string()]);
    }
}
