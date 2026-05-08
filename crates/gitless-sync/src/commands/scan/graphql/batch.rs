//! Alias batching: chunked GraphQL queries for `fetch_last_commit_at_batch`.
//!
//! This module owns the entry point ([`fetch_last_commit_at_batch`]) — query
//! construction lives in `query`, response decoding lives in `parse`.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::GRAPHQL_BATCH_SIZE;
use super::parse::parse_chunk;
use super::query::{build_query, split_repo};
use crate::shared::error::GitlessError;
use crate::shared::gh::GhClient;
use crate::shared::github::map_gh_error;

/// Fetch the timestamp of the most recent commit per path on `branch`.
///
/// Calls `gh api graphql -f query=<query>` once per chunk of
/// [`GRAPHQL_BATCH_SIZE`] paths. Returns a `path → committedDate` map
/// covering all input paths.
///
/// # Errors
/// - [`GitlessError::Config`] when `repo` is not in `owner/name` form.
/// - [`GitlessError::AuthFailed`] / [`GitlessError::RateLimitExceeded`] /
///   [`GitlessError::Http`] when the `gh` subprocess fails (REST stderr
///   substring mapping per `spec-error-contracts.md`).
/// - [`GitlessError::AuthFailed`] / [`GitlessError::RateLimitExceeded`] /
///   [`GitlessError::Http`] when the GraphQL response carries a non-empty
///   `errors[]` array (mapped via
///   [`crate::shared::error::map_graphql_error`]).
/// - [`GitlessError::Http`] for JSON decode failures, missing
///   `data.repository.ref.target` envelope, missing alias entries, empty
///   `nodes` lists (no commits found for a path), or unparseable
///   `committedDate` values.
pub(crate) fn fetch_last_commit_at_batch(
    client: &impl GhClient,
    repo: &str,
    branch: &str,
    paths: &[String],
) -> Result<HashMap<String, DateTime<Utc>>, GitlessError> {
    if paths.is_empty() {
        return Ok(HashMap::new());
    }

    let (owner, name) = split_repo(repo)?;
    let mut out = HashMap::with_capacity(paths.len());

    for chunk in paths.chunks(GRAPHQL_BATCH_SIZE) {
        let query = build_query(owner, name, branch, chunk);
        let args = vec![
            "api".to_string(),
            "graphql".to_string(),
            "-f".to_string(),
            format!("query={query}"),
        ];
        let resp = client.api(&args)?;
        if resp.exit_code != 0 {
            return Err(map_gh_error(&resp.stderr));
        }
        parse_chunk(&resp.stdout, chunk, branch, &mut out)?;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::scan::graphql::test_helpers::{
        err_resp, graphql_args, ok_resp, ok_response_for,
    };
    use crate::shared::gh::MockGhClient;

    // --- happy path --------------------------------------------------------

    #[test]
    fn returns_committed_date_for_single_path() {
        let owner = "owner";
        let name = "repo";
        let branch = "main";
        let paths = vec!["a.md".to_string()];

        let query = build_query(owner, name, branch, &paths);
        let body = ok_response_for(&[("a.md", "2026-05-07T10:00:00Z")]);

        let mut mock = MockGhClient::new();
        mock.stub(graphql_args(&query), ok_resp(body.as_bytes()));

        let map = fetch_last_commit_at_batch(&mock, "owner/repo", branch, &paths).unwrap();
        assert_eq!(map.len(), 1);
        let dt = map.get("a.md").unwrap();
        assert_eq!(dt.to_rfc3339(), "2026-05-07T10:00:00+00:00");
    }

    #[test]
    fn empty_paths_returns_empty_map_without_calling_gh() {
        // No stubs registered. MockGhClient would return Http error if called.
        let mock = MockGhClient::new();
        let result = fetch_last_commit_at_batch(&mock, "owner/repo", "main", &[]).unwrap();
        assert!(result.is_empty());
    }

    // --- repo split error propagation through entry -----------------------

    #[test]
    fn invalid_repo_format_returns_config_error() {
        let mock = MockGhClient::new();
        let paths = vec!["a.md".to_string()];
        let err = fetch_last_commit_at_batch(&mock, "no-slash", "main", &paths).unwrap_err();
        match err {
            GitlessError::Config(msg) => {
                assert!(msg.contains("owner/name"), "got: {msg}");
            }
            other => panic!("expected Config, got {other:?}"),
        }
    }

    // --- gh exit != 0 stderr routing --------------------------------------

    #[test]
    fn gh_exit_nonzero_with_bad_credentials_maps_to_auth_failed() {
        let paths = vec!["a.md".to_string()];
        let query = build_query("owner", "repo", "main", &paths);
        let mut mock = MockGhClient::new();
        mock.stub(
            graphql_args(&query),
            err_resp("gh: Bad credentials (HTTP 401)"),
        );

        let err = fetch_last_commit_at_batch(&mock, "owner/repo", "main", &paths).unwrap_err();
        assert!(matches!(err, GitlessError::AuthFailed));
    }

    // --- chunking ----------------------------------------------------------

    #[test]
    fn chunks_paths_above_batch_size() {
        // 300 paths → two chunks: 200 (first BATCH_SIZE) + 100. The function
        // must perform two `gh api graphql` calls and merge the responses.
        let total = GRAPHQL_BATCH_SIZE + 100;
        let paths: Vec<String> = (0..total).map(|i| format!("p{i}.md")).collect();
        let date = "2026-05-07T10:00:00Z";

        let chunk1: Vec<&str> = paths[..GRAPHQL_BATCH_SIZE]
            .iter()
            .map(String::as_str)
            .collect();
        let chunk2: Vec<&str> = paths[GRAPHQL_BATCH_SIZE..]
            .iter()
            .map(String::as_str)
            .collect();

        assert_eq!(chunk1.len(), GRAPHQL_BATCH_SIZE, "first chunk = BATCH_SIZE");
        assert_eq!(chunk2.len(), 100, "second chunk = remainder");

        let chunk1_owned: Vec<String> = chunk1.iter().map(|s| (*s).to_string()).collect();
        let chunk2_owned: Vec<String> = chunk2.iter().map(|s| (*s).to_string()).collect();

        let query1 = build_query("owner", "repo", "main", &chunk1_owned);
        let query2 = build_query("owner", "repo", "main", &chunk2_owned);

        let entries1: Vec<(&str, &str)> = chunk1.iter().map(|p| (*p, date)).collect();
        let entries2: Vec<(&str, &str)> = chunk2.iter().map(|p| (*p, date)).collect();

        let mut mock = MockGhClient::new();
        mock.stub(
            graphql_args(&query1),
            ok_resp(ok_response_for(&entries1).as_bytes()),
        );
        mock.stub(
            graphql_args(&query2),
            ok_resp(ok_response_for(&entries2).as_bytes()),
        );

        let map = fetch_last_commit_at_batch(&mock, "owner/repo", "main", &paths).unwrap();
        assert_eq!(map.len(), total);
        for p in &paths {
            assert!(map.contains_key(p), "missing path {p}");
        }
    }

    // --- happy path: multi-path -------------------------------------------

    #[test]
    fn returns_committed_dates_for_ten_paths() {
        let paths: Vec<String> = (0..10).map(|i| format!("p{i}.md")).collect();
        let dates: Vec<String> = (0..10)
            .map(|i| format!("2026-05-07T10:{i:02}:00Z"))
            .collect();
        let entries: Vec<(&str, &str)> = paths
            .iter()
            .zip(dates.iter())
            .map(|(p, d)| (p.as_str(), d.as_str()))
            .collect();

        let query = build_query("owner", "repo", "main", &paths);
        let body = ok_response_for(&entries);

        let mut mock = MockGhClient::new();
        mock.stub(graphql_args(&query), ok_resp(body.as_bytes()));

        let map = fetch_last_commit_at_batch(&mock, "owner/repo", "main", &paths).unwrap();
        assert_eq!(map.len(), 10);
        for (i, p) in paths.iter().enumerate() {
            let expected = format!("2026-05-07T10:{i:02}:00+00:00");
            let dt = map.get(p).unwrap_or_else(|| panic!("missing {p}"));
            assert_eq!(dt.to_rfc3339(), expected, "path {p} index {i}");
        }
    }

    // --- alias mangling round-trip: full batch with unique per-path dates -

    #[test]
    fn alias_mangling_full_batch_round_trips_correctly() {
        // Exactly GRAPHQL_BATCH_SIZE paths in a single chunk, each with a
        // unique committedDate. Unique dates make this assertion failable if
        // the alias→path index ever drifts (every path's date is checked
        // against its expected slot, not just presence).
        let total = GRAPHQL_BATCH_SIZE;
        let paths: Vec<String> = (0..total).map(|i| format!("p{i}.md")).collect();
        let dates: Vec<String> = (0..total)
            .map(|i| {
                let hour = 10 + i / 60;
                let minute = i % 60;
                format!("2026-05-07T{hour:02}:{minute:02}:00Z")
            })
            .collect();
        let entries: Vec<(&str, &str)> = paths
            .iter()
            .zip(dates.iter())
            .map(|(p, d)| (p.as_str(), d.as_str()))
            .collect();

        let query = build_query("owner", "repo", "main", &paths);
        let body = ok_response_for(&entries);

        let mut mock = MockGhClient::new();
        mock.stub(graphql_args(&query), ok_resp(body.as_bytes()));

        let map = fetch_last_commit_at_batch(&mock, "owner/repo", "main", &paths).unwrap();
        assert_eq!(map.len(), total);
        for (i, p) in paths.iter().enumerate() {
            let hour = 10 + i / 60;
            let minute = i % 60;
            let expected = format!("2026-05-07T{hour:02}:{minute:02}:00+00:00");
            let dt = map.get(p).unwrap_or_else(|| panic!("missing path {p}"));
            assert_eq!(dt.to_rfc3339(), expected, "alias a{i} for path {p}");
        }
    }

    // --- escape end-to-end: argv match through MockGhClient ---------------

    #[test]
    fn fetch_with_escape_chars_in_path_routes_through_escaped_query() {
        // MockGhClient keys on exact argv. If escape ever drifted, the stub
        // wouldn't match and the call would fail with `no stub registered`.
        let raw_path = "weird\"name\\here.md";
        let paths = vec![raw_path.to_string()];
        let query = build_query("owner", "repo", "main", &paths);
        let body = ok_response_for(&[(raw_path, "2026-05-07T10:00:00Z")]);

        let mut mock = MockGhClient::new();
        mock.stub(graphql_args(&query), ok_resp(body.as_bytes()));

        let map = fetch_last_commit_at_batch(&mock, "owner/repo", "main", &paths).unwrap();
        assert_eq!(map.len(), 1);
        assert!(map.contains_key(raw_path));
    }
}
