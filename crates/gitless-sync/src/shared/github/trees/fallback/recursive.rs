//! Sub-tree fallback recursive descent (Phase 7).
//!
//! Walks `gh api repos/{repo}/git/trees/{sub_tree_sha}` non-recursive
//! responses depth-first per `spec-github-api.md` § sub-tree 재귀
//! 알고리즘. Cap checks against [`super::MAX_TREE_CALL_BUDGET`] +
//! [`super::MAX_TREE_ENTRIES`] gate every layer; either trip aborts
//! with [`GitlessError::TreesTruncated`] and discards partial state
//! (G-002 no-partial-result policy).
//!
//! `Descent` bundles the four args the recursion threads through every
//! frame so the per-call signature stays under the workspace
//! `clippy::too_many_arguments` cap (5). The two genuinely per-frame
//! args (`tree_sha`, `path_prefix`) ride alongside.
//!
//! The loop deviates from the spec pseudo-code in one place: only
//! `entry.entry_type == "tree"` short-circuits to recurse on the
//! sub-tree sha; every other entry routes through
//! [`classify_tree_entry`] so submodule (`commit` + `160000`),
//! symlink (`120000`), executable (`100755`), and the
//! unsupported-mode warning path stay aligned with task A/B/C
//! semantics rather than re-implementing classification here.

use super::super::classify::{RemoteFile, classify_tree_entry};
use super::super::parse::parse_tree_body;
use super::{Budget, MAX_TREE_CALL_BUDGET, MAX_TREE_ENTRIES};
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::github::map_gh_error;

/// Bundle of state threaded through every recursive frame:
/// the gh client + repo (immutable across the descent) plus the
/// shared accumulators (`entries`, `budget`). All four share one
/// lifetime so caller code can build a `Descent` once and reborrow
/// it on each recursive call.
#[allow(dead_code)]
pub(super) struct Descent<'a, C: GhClient> {
    pub(super) client: &'a C,
    pub(super) repo: &'a str,
    pub(super) entries: &'a mut Vec<RemoteFile>,
    pub(super) budget: &'a mut Budget,
}

/// Walk the sub-tree rooted at `tree_sha`, appending discovered
/// [`RemoteFile`] entries to `descent.entries`.
///
/// `path_prefix` is empty at the root invocation; each recursive
/// descent passes the path joined so far. The two cap checks are
/// evaluated **before** the next `gh api` call so a saturated state
/// short-circuits without further IO.
///
/// # Errors
/// - [`GitlessError::TreesTruncated`] when either cap is reached
///   (G-002 no-partial-result policy applies — caller must discard
///   `descent.entries`).
/// - [`GitlessError::AuthFailed`] / [`GitlessError::RateLimitExceeded`]
///   / [`GitlessError::Http`] per `spec-error-contracts.md`
///   (mapping delegated to [`map_gh_error`]).
/// - [`GitlessError::Http`] for JSON decode failures or `truncated:true`
///   on a sub-tree response (parse propagates `TreesTruncated`).
#[allow(dead_code)]
pub(super) fn fetch_subtree_recursive<C: GhClient>(
    descent: &mut Descent<'_, C>,
    tree_sha: &str,
    path_prefix: &str,
) -> Result<(), GitlessError> {
    if descent.budget.calls_used >= MAX_TREE_CALL_BUDGET {
        return Err(GitlessError::TreesTruncated);
    }
    if descent.entries.len() >= MAX_TREE_ENTRIES {
        return Err(GitlessError::TreesTruncated);
    }

    let args = vec![
        "api".to_string(),
        format!("repos/{}/git/trees/{tree_sha}", descent.repo),
    ];
    let resp = descent.client.api(&args)?;
    if resp.exit_code != 0 {
        return Err(map_gh_error(&resp.stderr));
    }
    descent.budget.calls_used += 1;

    let body = parse_tree_body(&resp.stdout)?;

    for mut entry in body.tree {
        let full_path = if path_prefix.is_empty() {
            entry.path.clone()
        } else {
            format!("{path_prefix}/{}", entry.path)
        };

        if entry.entry_type == "tree" {
            let sub_sha = entry.sha.clone();
            fetch_subtree_recursive(descent, &sub_sha, &full_path)?;
        } else {
            entry.path = full_path;
            if let Some(rf) = classify_tree_entry(entry) {
                descent.entries.push(rf);
                if descent.entries.len() >= MAX_TREE_ENTRIES {
                    return Err(GitlessError::TreesTruncated);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
mod tests {
    use super::super::super::classify::RemoteFile;
    use super::{
        Budget, Descent, MAX_TREE_CALL_BUDGET, MAX_TREE_ENTRIES, fetch_subtree_recursive,
    };
    use crate::shared::error::GitlessError;
    use crate::shared::gh::{GhResponse, MockGhClient};

    fn ok_resp(stdout: &[u8]) -> GhResponse {
        GhResponse {
            stdout: stdout.to_vec(),
            stderr: String::new(),
            exit_code: 0,
        }
    }

    fn err_resp(stderr: &str) -> GhResponse {
        GhResponse {
            stdout: Vec::new(),
            stderr: stderr.to_string(),
            exit_code: 1,
        }
    }

    fn tree_args(repo: &str, sha: &str) -> Vec<String> {
        vec!["api".to_string(), format!("repos/{repo}/git/trees/{sha}")]
    }

    #[test]
    fn empty_tree_yields_zero_entries_and_one_call() {
        let body = br#"{"sha":"x","tree":[],"truncated":false}"#;
        let mut mock = MockGhClient::new();
        mock.stub(tree_args("o/r", "root"), ok_resp(body));

        let mut entries = vec![];
        let mut budget = Budget::new();
        {
            let mut descent = Descent {
                client: &mock,
                repo: "o/r",
                entries: &mut entries,
                budget: &mut budget,
            };
            fetch_subtree_recursive(&mut descent, "root", "").unwrap();
        }
        assert_eq!(entries.len(), 0);
        assert_eq!(budget.calls_used, 1);
    }

    #[test]
    fn root_blob_lands_with_unprefixed_full_path() {
        let body = br#"{
            "sha":"root",
            "tree":[{"path":"README.md","mode":"100644","type":"blob","sha":"b1","size":42}],
            "truncated":false
        }"#;
        let mut mock = MockGhClient::new();
        mock.stub(tree_args("o/r", "root"), ok_resp(body));

        let mut entries = vec![];
        let mut budget = Budget::new();
        {
            let mut descent = Descent {
                client: &mock,
                repo: "o/r",
                entries: &mut entries,
                budget: &mut budget,
            };
            fetch_subtree_recursive(&mut descent, "root", "").unwrap();
        }
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "README.md");
        assert_eq!(entries[0].mode, "100644");
        assert_eq!(entries[0].sha, "b1");
        assert_eq!(budget.calls_used, 1);
    }

    #[test]
    fn nested_tree_descends_and_joins_path_with_forward_slash() {
        let root_body = br#"{
            "sha":"root",
            "tree":[{"path":"docs","mode":"040000","type":"tree","sha":"t_docs"}],
            "truncated":false
        }"#;
        let docs_body = br#"{
            "sha":"t_docs",
            "tree":[{"path":"intro.md","mode":"100644","type":"blob","sha":"b_intro","size":12}],
            "truncated":false
        }"#;
        let mut mock = MockGhClient::new();
        mock.stub(tree_args("o/r", "root"), ok_resp(root_body));
        mock.stub(tree_args("o/r", "t_docs"), ok_resp(docs_body));

        let mut entries = vec![];
        let mut budget = Budget::new();
        {
            let mut descent = Descent {
                client: &mock,
                repo: "o/r",
                entries: &mut entries,
                budget: &mut budget,
            };
            fetch_subtree_recursive(&mut descent, "root", "").unwrap();
        }
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "docs/intro.md");
        assert_eq!(entries[0].sha, "b_intro");
        assert_eq!(budget.calls_used, 2);
    }

    #[test]
    fn call_budget_saturated_aborts_before_any_api_call() {
        // Mock has zero stubs — if the function reached `client.api()`
        // it would surface the unstubbed-args error instead of
        // TreesTruncated.
        let mock = MockGhClient::new();

        let mut entries = vec![];
        let mut budget = Budget {
            calls_used: MAX_TREE_CALL_BUDGET,
        };
        let err = {
            let mut descent = Descent {
                client: &mock,
                repo: "o/r",
                entries: &mut entries,
                budget: &mut budget,
            };
            fetch_subtree_recursive(&mut descent, "root", "").unwrap_err()
        };
        assert!(matches!(err, GitlessError::TreesTruncated));
        assert_eq!(budget.calls_used, MAX_TREE_CALL_BUDGET);
    }

    #[test]
    fn entries_cap_saturated_aborts_before_any_api_call() {
        let mock = MockGhClient::new();

        let mut entries: Vec<RemoteFile> = (0..MAX_TREE_ENTRIES)
            .map(|_| RemoteFile {
                path: String::new(),
                sha: String::new(),
                mode: String::new(),
            })
            .collect();
        let mut budget = Budget::new();
        let err = {
            let mut descent = Descent {
                client: &mock,
                repo: "o/r",
                entries: &mut entries,
                budget: &mut budget,
            };
            fetch_subtree_recursive(&mut descent, "root", "").unwrap_err()
        };
        assert!(matches!(err, GitlessError::TreesTruncated));
        assert_eq!(budget.calls_used, 0);
    }

    #[test]
    fn gh_exit_nonzero_propagates_via_map_gh_error() {
        let mut mock = MockGhClient::new();
        mock.stub(
            tree_args("o/r", "root"),
            err_resp("gh: Bad credentials (HTTP 401)"),
        );

        let mut entries = vec![];
        let mut budget = Budget::new();
        let err = {
            let mut descent = Descent {
                client: &mock,
                repo: "o/r",
                entries: &mut entries,
                budget: &mut budget,
            };
            fetch_subtree_recursive(&mut descent, "root", "").unwrap_err()
        };
        assert!(matches!(err, GitlessError::AuthFailed));
    }
}
