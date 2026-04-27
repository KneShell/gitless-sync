use chrono::{DateTime, Utc};

use crate::shared::error::GitlessError;

#[derive(Debug, Clone)]
pub struct RemoteFile {
    pub path: String,
    pub sha: String,
    pub mode: String,
    pub size: Option<u64>,
}

pub fn fetch_tree(repo: &str, branch: &str, token: &str) -> Result<Vec<RemoteFile>, GitlessError> {
    let _ = (repo, branch, token);
    todo!("GET /repos/{repo}/git/trees/{branch}?recursive=1 -- fail fast on truncated=true")
}

pub fn fetch_blob(repo: &str, sha: &str, token: &str) -> Result<Vec<u8>, GitlessError> {
    let _ = (repo, sha, token);
    todo!("GET /repos/{repo}/git/blobs/{sha} -- base64 decode")
}

pub fn fetch_last_commit_at(
    repo: &str,
    branch: &str,
    path: &str,
    token: &str,
) -> Result<DateTime<Utc>, GitlessError> {
    let _ = (repo, branch, path, token);
    todo!("GET /repos/{repo}/commits?sha={branch}&path={path}&per_page=1")
}
