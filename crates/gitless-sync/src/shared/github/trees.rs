use serde::Deserialize;

use super::error_map::map_gh_error;
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::path::to_nfc;

#[derive(Debug, Clone)]
pub struct RemoteFile {
    pub path: String,
    pub sha: String,
    /// Tree mode bit: `100644` regular, `100755` executable, `160000`
    /// submodule, `120000` symlink. Carried through to v1.1 JSON
    /// `files[].mode`. Phase 5 task G adds submodule (`160000`); H/J extend
    /// the others (`spec-output-schema.md` § v1.1, `spec-domain-pitfalls.md`).
    pub mode: String,
}

/// Fetch the recursive tree of `branch` via `gh api` subprocess.
///
/// Calls `gh api repos/{repo}/git/trees/{branch}?recursive=1` and converts the
/// response per `spec-github-api.md`. Authentication, rate limiting, and
/// truncation are observed through `GhResponse.exit_code` + `stderr`
/// substring matching per `spec-error-contracts.md`.
///
/// # Errors
/// - [`GitlessError::TreesTruncated`] when `truncated == true` (G-002).
/// - [`GitlessError::AuthFailed`] when stderr contains `"Bad credentials"`.
/// - [`GitlessError::RateLimitExceeded`] when stderr contains
///   `"secondary rate limit"` or `"API rate limit exceeded"`.
/// - [`GitlessError::Http`] for parse failures or any other gh failure mode.
pub(crate) fn fetch_tree(
    client: &impl GhClient,
    repo: &str,
    branch: &str,
) -> Result<Vec<RemoteFile>, GitlessError> {
    let args = vec![
        "api".to_string(),
        format!("repos/{repo}/git/trees/{branch}?recursive=1"),
    ];
    let resp = client.api(&args)?;
    if resp.exit_code != 0 {
        return Err(map_gh_error(&resp.stderr));
    }

    let body: TreeResponse = serde_json::from_slice(&resp.stdout)
        .map_err(|e| GitlessError::Http(format!("decode trees response: {e}")))?;

    if body.truncated {
        return Err(GitlessError::TreesTruncated);
    }

    let mut files = Vec::with_capacity(body.tree.len());
    for entry in body.tree {
        match (entry.entry_type.as_str(), entry.mode.as_str()) {
            ("blob", "100644") => {
                files.push(RemoteFile {
                    path: to_nfc(&entry.path),
                    sha: entry.sha,
                    mode: entry.mode,
                });
            }
            ("blob", "120000") => {
                // Symlink — carry through to compare.rs which promotes the
                // path to `Status::Failed` + `failed_reason: "symlink"`
                // (Phase 5 task H, spec-domain-pitfalls.md § Symlink). The
                // `sha` here points to a blob whose contents are the link
                // target path; we do not follow or compare it.
                files.push(RemoteFile {
                    path: to_nfc(&entry.path),
                    sha: entry.sha,
                    mode: entry.mode,
                });
            }
            ("commit", "160000") => {
                // Submodule — carry through to compare.rs which promotes the
                // path to `Status::Failed` + `failed_reason: "submodule"`
                // (Phase 5 task G, spec-domain-pitfalls.md § Submodule). The
                // `sha` here is the submodule pointer commit, useful info for
                // the caller deciding what to do with it.
                files.push(RemoteFile {
                    path: to_nfc(&entry.path),
                    sha: entry.sha,
                    mode: entry.mode,
                });
            }
            (other_type, other_mode) => {
                if other_type == "blob" {
                    eprintln!(
                        "warning: skipping {} (mode {} unsupported in v0.1)",
                        entry.path, other_mode
                    );
                }
            }
        }
    }

    Ok(files)
}

#[derive(Debug, Deserialize)]
struct TreeResponse {
    tree: Vec<TreeEntry>,
    truncated: bool,
}

#[derive(Debug, Deserialize)]
struct TreeEntry {
    path: String,
    mode: String,
    #[serde(rename = "type")]
    entry_type: String,
    sha: String,
}

#[cfg(test)]
#[path = "trees_tests.rs"]
mod tests;
