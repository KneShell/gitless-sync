//! Sub-tree fallback orchestrator: cap pre-checks → `gh api` →
//! parse → consume the [`super::iter::Outcome`] stream → push blobs +
//! recurse on sub-trees.
//!
//! See `spec-github-api.md` § sub-tree 재귀 알고리즘 for the trace.
//! Cap checks against [`super::super::MAX_TREE_CALL_BUDGET`] +
//! [`super::super::MAX_TREE_ENTRIES`] gate every layer; either trip
//! aborts with [`GitlessError::TreesTruncated`] and discards partial
//! state (G-002 no-partial-result policy).

use super::super::super::parse::parse_tree_body;
use super::super::{MAX_TREE_CALL_BUDGET, MAX_TREE_ENTRIES};
use super::Descent;
use super::iter::{Outcome, process_entries};
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::github::map_gh_error;

/// Walk the sub-tree rooted at `tree_sha`, appending discovered blobs
/// to `descent.entries` and recursing on tree entries.
///
/// `path_prefix` is empty at the root invocation; each recursive
/// descent passes the path joined so far. The two cap checks are
/// evaluated **before** the next `gh api` call so a saturated state
/// short-circuits without further IO. The post-push entries cap also
/// fires inline so a wide root tree cannot push past
/// [`MAX_TREE_ENTRIES`] before the next sub-tree call's pre-check.
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
pub(in super::super::super) fn fetch_subtree_recursive<C: GhClient>(
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
    for outcome in process_entries(body, path_prefix) {
        match outcome {
            Outcome::Blob(rf) => {
                descent.entries.push(rf);
                if descent.entries.len() >= MAX_TREE_ENTRIES {
                    return Err(GitlessError::TreesTruncated);
                }
            }
            Outcome::Subtree { sha, path } => {
                fetch_subtree_recursive(descent, &sha, &path)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
mod tests {
    use std::fmt::Write as _;

    use super::super::super::super::classify::RemoteFile;
    use super::super::super::Budget;
    use super::super::super::{MAX_TREE_CALL_BUDGET, MAX_TREE_ENTRIES};
    use super::super::Descent;
    use super::*;
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

    /// Test harness that builds a `Descent` against the mock, runs the
    /// fallback descent rooted at `sha`, and returns `(result, entries,
    /// budget)`. Wraps the borrow-checker dance every test would
    /// otherwise repeat: `entries` and `budget` outlive the inner
    /// `Descent` so the test can assert on them after `result` is
    /// observed.
    fn run_descent(
        mock: &MockGhClient,
        sha: &str,
        budget: Budget,
        entries: Vec<RemoteFile>,
    ) -> (Result<(), GitlessError>, Vec<RemoteFile>, Budget) {
        let mut e = entries;
        let mut b = budget;
        let r = {
            let mut d = Descent {
                client: mock,
                repo: "o/r",
                entries: &mut e,
                budget: &mut b,
            };
            fetch_subtree_recursive(&mut d, sha, "")
        };
        (r, e, b)
    }

    #[test]
    fn empty_tree_yields_zero_entries_and_one_call() {
        let body = br#"{"sha":"x","tree":[],"truncated":false}"#;
        let mut mock = MockGhClient::new();
        mock.stub(tree_args("o/r", "root"), ok_resp(body));
        let (r, entries, budget) = run_descent(&mock, "root", Budget::new(), vec![]);
        r.unwrap();
        assert_eq!(entries.len(), 0);
        assert_eq!(budget.calls_used, 1);
    }

    #[test]
    fn root_blob_lands_with_unprefixed_full_path() {
        let body = br#"{"sha":"root","tree":[{"path":"README.md","mode":"100644","type":"blob","sha":"b1","size":42}],"truncated":false}"#;
        let mut mock = MockGhClient::new();
        mock.stub(tree_args("o/r", "root"), ok_resp(body));
        let (r, entries, budget) = run_descent(&mock, "root", Budget::new(), vec![]);
        r.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "README.md");
        assert_eq!(entries[0].mode, "100644");
        assert_eq!(entries[0].sha, "b1");
        assert_eq!(budget.calls_used, 1);
    }

    #[test]
    fn nested_tree_descends_and_joins_path_with_forward_slash() {
        let root_body = br#"{"sha":"root","tree":[{"path":"docs","mode":"040000","type":"tree","sha":"t_docs"}],"truncated":false}"#;
        let docs_body = br#"{"sha":"t_docs","tree":[{"path":"intro.md","mode":"100644","type":"blob","sha":"b_intro","size":12}],"truncated":false}"#;
        let mut mock = MockGhClient::new();
        mock.stub(tree_args("o/r", "root"), ok_resp(root_body));
        mock.stub(tree_args("o/r", "t_docs"), ok_resp(docs_body));
        let (r, entries, budget) = run_descent(&mock, "root", Budget::new(), vec![]);
        r.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "docs/intro.md");
        assert_eq!(entries[0].sha, "b_intro");
        assert_eq!(budget.calls_used, 2);
    }

    #[test]
    fn call_budget_saturated_aborts_before_any_api_call() {
        let mock = MockGhClient::new();
        let saturated = Budget {
            calls_used: MAX_TREE_CALL_BUDGET,
        };
        let (r, _, budget) = run_descent(&mock, "root", saturated, vec![]);
        assert!(matches!(r.unwrap_err(), GitlessError::TreesTruncated));
        assert_eq!(budget.calls_used, MAX_TREE_CALL_BUDGET);
    }

    #[test]
    fn entries_cap_saturated_aborts_before_any_api_call() {
        let mock = MockGhClient::new();
        let initial: Vec<RemoteFile> = (0..MAX_TREE_ENTRIES)
            .map(|_| RemoteFile {
                path: String::new(),
                sha: String::new(),
                mode: String::new(),
            })
            .collect();
        let (r, _, budget) = run_descent(&mock, "root", Budget::new(), initial);
        assert!(matches!(r.unwrap_err(), GitlessError::TreesTruncated));
        assert_eq!(budget.calls_used, 0);
    }

    #[test]
    fn gh_exit_nonzero_propagates_via_map_gh_error() {
        let mut mock = MockGhClient::new();
        mock.stub(
            tree_args("o/r", "root"),
            err_resp("gh: Bad credentials (HTTP 401)"),
        );
        let (r, _, _) = run_descent(&mock, "root", Budget::new(), vec![]);
        assert!(matches!(r.unwrap_err(), GitlessError::AuthFailed));
    }

    #[test]
    fn call_budget_caps_at_1001st_recursive_call() {
        // Root references 1000 sibling sub-trees; root call + 999 successful
        // recursive calls saturate `budget.calls_used` at MAX_TREE_CALL_BUDGET
        // (== 1000). The 1001st recursion entry fails the pre-call budget
        // check and aborts with TreesTruncated. The fixture also stubs the
        // 1000th sub-tree (`t999`) so an algorithm bug that *did* recurse
        // past the cap would surface as a panic on the unstubbed-args
        // `t999` rather than as a silent off-by-one.
        let mut mock = MockGhClient::new();
        let mut tree = String::new();
        for i in 0..1000u32 {
            if i > 0 {
                tree.push(',');
            }
            write!(
                tree,
                r#"{{"path":"d{i}","mode":"040000","type":"tree","sha":"t{i}"}}"#
            )
            .unwrap();
        }
        let root_body = format!(r#"{{"sha":"r","tree":[{tree}],"truncated":false}}"#);
        mock.stub(tree_args("o/r", "root"), ok_resp(root_body.as_bytes()));
        let empty = br#"{"sha":"x","tree":[],"truncated":false}"#;
        for i in 0..1000u32 {
            mock.stub(tree_args("o/r", &format!("t{i}")), ok_resp(empty));
        }
        let (r, _, budget) = run_descent(&mock, "root", Budget::new(), vec![]);
        assert!(matches!(r.unwrap_err(), GitlessError::TreesTruncated));
        assert_eq!(budget.calls_used, MAX_TREE_CALL_BUDGET);
    }

    #[test]
    fn entries_cap_caps_at_500_001st_blob_input() {
        // Root response carries 500 001 blob entries. The post-push check
        // fires when the 500 000th push lands `entries.len()` at
        // MAX_TREE_ENTRIES; the 500 001st entry never gets pushed.
        let mut tree = String::with_capacity(500_001 * 70);
        for i in 0..500_001u32 {
            if i > 0 {
                tree.push(',');
            }
            write!(
                tree,
                r#"{{"path":"f{i}","mode":"100644","type":"blob","sha":"b{i}"}}"#
            )
            .unwrap();
        }
        let body = format!(r#"{{"sha":"r","tree":[{tree}],"truncated":false}}"#);
        let mut mock = MockGhClient::new();
        mock.stub(tree_args("o/r", "root"), ok_resp(body.as_bytes()));
        let (r, entries, _) = run_descent(&mock, "root", Budget::new(), vec![]);
        assert!(matches!(r.unwrap_err(), GitlessError::TreesTruncated));
        assert_eq!(entries.len(), MAX_TREE_ENTRIES);
    }
}
