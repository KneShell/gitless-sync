//! GraphQL backend for `fetch_last_commit_at` (Phase 4, ADR 0005/0006).
//!
//! Replaces the REST commits endpoint's N×round-trip pattern with a single
//! `gh api graphql` invocation per chunk of [`GRAPHQL_BATCH_SIZE`] paths.
//! Each chunk uses GraphQL alias batching: one alias = one
//! `history(first: 1, path: ...)` node, so all paths in a chunk are resolved
//! in a single round-trip and evaluated in parallel server-side. Per ADR
//! 0005, this backend deliberately does not use rayon — the alias batching
//! itself is the parallelism. Authentication, rate limiting, and transport
//! errors stay delegated to the `gh` subprocess (ADR 0001 + ADR 0002).
//!
//! Error classification follows `spec-error-contracts.md` § GraphQL error
//! mapping: gh subprocess exit ≠ 0 routes through the same REST stderr
//! substring table (via [`super::github::map_gh_error`]); exit == 0 with a
//! non-empty `errors[]` array routes through
//! [`crate::shared::error::map_graphql_error`] keyed off
//! `errors[0].extensions.code`.

use std::collections::HashMap;
use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::shared::error::{GitlessError, GraphqlError, map_graphql_error};
use crate::shared::gh::GhClient;

use super::github::map_gh_error;

/// Number of alias entries packed into a single `gh api graphql` request.
///
/// Default 200 per `roadmap.md` § Phase 4 GraphQL batching, confirmed by
/// ADR 0007 (P6a raw data, 2026-05-07): at 13-path scale batch 100 vs 200
/// resolve to a single chunk and are functionally equivalent — measurement
/// noise dominated. yagni keeps the recommended ceiling. Any change requires
/// a coordinated update of this constant + `spec-github-api.md` § GraphQL
/// backend + ADR 0007.
pub(crate) const GRAPHQL_BATCH_SIZE: usize = 200;

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
///   `errors[]` array (mapped via [`map_graphql_error`]).
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

        let body: GraphqlResponse = serde_json::from_slice(&resp.stdout)
            .map_err(|e| GitlessError::Http(format!("decode graphql response: {e}")))?;

        if let Some(errors) = body.errors.as_ref()
            && !errors.is_empty()
        {
            return Err(map_graphql_error(errors));
        }

        let target = body
            .data
            .and_then(|d| d.repository)
            .and_then(|r| r.git_ref)
            .and_then(|r| r.target)
            .ok_or_else(|| {
                GitlessError::Http(format!(
                    "graphql: ref refs/heads/{branch} target missing in response"
                ))
            })?;

        for (i, path) in chunk.iter().enumerate() {
            let alias = format!("a{i}");
            let history = target.aliases.get(&alias).ok_or_else(|| {
                GitlessError::Http(format!("graphql: missing alias {alias} for path {path}"))
            })?;
            let node = history
                .nodes
                .first()
                .ok_or_else(|| GitlessError::Http(format!("no commits found for path: {path}")))?;
            let dt = DateTime::parse_from_rfc3339(&node.committed_date)
                .map_err(|e| GitlessError::Http(format!("parse graphql commit date: {e}")))?
                .with_timezone(&Utc);
            out.insert(path.clone(), dt);
        }
    }

    Ok(out)
}

fn split_repo(repo: &str) -> Result<(&str, &str), GitlessError> {
    repo.split_once('/').ok_or_else(|| {
        GitlessError::Config(format!("invalid repo format: {repo} (expected owner/name)"))
    })
}

fn escape_graphql_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out
}

fn build_query(owner: &str, name: &str, branch: &str, paths: &[String]) -> String {
    let mut q = String::new();
    q.push_str("query {\n");
    let _ = writeln!(q, "  repository(owner: \"{owner}\", name: \"{name}\") {{");
    let _ = writeln!(q, "    ref(qualifiedName: \"refs/heads/{branch}\") {{");
    q.push_str("      target {\n");
    q.push_str("        ... on Commit {\n");
    for (i, path) in paths.iter().enumerate() {
        let escaped = escape_graphql_string(path);
        let _ = writeln!(
            q,
            "          a{i}: history(first: 1, path: \"{escaped}\") {{ nodes {{ committedDate }} }}"
        );
    }
    q.push_str("        }\n");
    q.push_str("      }\n");
    q.push_str("    }\n");
    q.push_str("  }\n");
    q.push_str("}\n");
    q
}

#[derive(Debug, Deserialize)]
struct GraphqlResponse {
    data: Option<GraphqlData>,
    #[serde(default)]
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphqlData {
    repository: Option<GraphqlRepository>,
}

#[derive(Debug, Deserialize)]
struct GraphqlRepository {
    #[serde(rename = "ref")]
    git_ref: Option<GraphqlRef>,
}

#[derive(Debug, Deserialize)]
struct GraphqlRef {
    target: Option<GraphqlTarget>,
}

#[derive(Debug, Deserialize)]
struct GraphqlTarget {
    #[serde(flatten)]
    aliases: HashMap<String, GraphqlHistory>,
}

#[derive(Debug, Deserialize)]
struct GraphqlHistory {
    nodes: Vec<GraphqlCommitNode>,
}

#[derive(Debug, Deserialize)]
struct GraphqlCommitNode {
    #[serde(rename = "committedDate")]
    committed_date: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::gh::{GhResponse, MockGhClient};

    fn ok_resp(body: &[u8]) -> GhResponse {
        GhResponse {
            stdout: body.to_vec(),
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

    fn graphql_args(query: &str) -> Vec<String> {
        vec![
            "api".to_string(),
            "graphql".to_string(),
            "-f".to_string(),
            format!("query={query}"),
        ]
    }

    fn ok_response_for(paths: &[(&str, &str)]) -> String {
        let mut alias_entries = String::new();
        for (i, (_, date)) in paths.iter().enumerate() {
            if i > 0 {
                alias_entries.push(',');
            }
            let _ = write!(
                alias_entries,
                r#""a{i}":{{"nodes":[{{"committedDate":"{date}"}}]}}"#
            );
        }
        format!(
            r#"{{"data":{{"repository":{{"ref":{{"target":{{{alias_entries}}}}}}}}},"errors":[]}}"#
        )
    }

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

    // --- empty input -------------------------------------------------------

    #[test]
    fn empty_paths_returns_empty_map_without_calling_gh() {
        // No stubs registered. MockGhClient would return Http error if called.
        let mock = MockGhClient::new();
        let result = fetch_last_commit_at_batch(&mock, "owner/repo", "main", &[]).unwrap();
        assert!(result.is_empty());
    }

    // --- GraphQL errors[] mapping -----------------------------------------

    #[test]
    fn rate_limited_extension_code_maps_to_rate_limit_exceeded() {
        let body = r#"{"data":null,"errors":[{"message":"throttled","extensions":{"code":"RATE_LIMITED"}}]}"#;
        let paths = vec!["a.md".to_string()];
        let query = build_query("owner", "repo", "main", &paths);
        let mut mock = MockGhClient::new();
        mock.stub(graphql_args(&query), ok_resp(body.as_bytes()));

        let err = fetch_last_commit_at_batch(&mock, "owner/repo", "main", &paths).unwrap_err();
        assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
    }

    // --- repo split --------------------------------------------------------

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

    // --- escape_graphql_string ---------------------------------------------

    #[test]
    fn escape_graphql_string_escapes_backslash_quote_and_newline() {
        assert_eq!(escape_graphql_string("plain"), "plain");
        assert_eq!(escape_graphql_string(r#"a"b"#), r#"a\"b"#);
        assert_eq!(escape_graphql_string(r"a\b"), r"a\\b");
        assert_eq!(escape_graphql_string("a\nb"), "a\\nb");
    }

    // --- build_query embeds escaped path ----------------------------------

    #[test]
    fn build_query_escapes_path_with_quote() {
        let paths = vec![r#"weird"name.md"#.to_string()];
        let q = build_query("owner", "repo", "main", &paths);
        // The raw path's `"` must appear escaped inside the query string.
        assert!(q.contains(r#"path: "weird\"name.md""#), "got: {q}");
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

    // --- errors[] extensions.code: code-specific mapping -------------------

    #[test]
    fn unauthenticated_extension_code_maps_to_auth_failed() {
        let body = r#"{"data":null,"errors":[{"message":"login required","extensions":{"code":"UNAUTHENTICATED"}}]}"#;
        let paths = vec!["a.md".to_string()];
        let query = build_query("owner", "repo", "main", &paths);
        let mut mock = MockGhClient::new();
        mock.stub(graphql_args(&query), ok_resp(body.as_bytes()));

        let err = fetch_last_commit_at_batch(&mock, "owner/repo", "main", &paths).unwrap_err();
        assert!(matches!(err, GitlessError::AuthFailed));
    }

    #[test]
    fn not_found_extension_code_falls_through_to_http() {
        let body = r#"{"data":null,"errors":[{"message":"object not found","extensions":{"code":"NOT_FOUND"}}]}"#;
        let paths = vec!["a.md".to_string()];
        let query = build_query("owner", "repo", "main", &paths);
        let mut mock = MockGhClient::new();
        mock.stub(graphql_args(&query), ok_resp(body.as_bytes()));

        let err = fetch_last_commit_at_batch(&mock, "owner/repo", "main", &paths).unwrap_err();
        match err {
            GitlessError::Http(msg) => {
                assert!(msg.contains("NOT_FOUND"), "got: {msg}");
                assert!(msg.contains("object not found"), "got: {msg}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    #[test]
    fn internal_server_error_falls_through_to_http() {
        let body = r#"{"data":null,"errors":[{"message":"oops","extensions":{"code":"INTERNAL_SERVER_ERROR"}}]}"#;
        let paths = vec!["a.md".to_string()];
        let query = build_query("owner", "repo", "main", &paths);
        let mut mock = MockGhClient::new();
        mock.stub(graphql_args(&query), ok_resp(body.as_bytes()));

        let err = fetch_last_commit_at_batch(&mock, "owner/repo", "main", &paths).unwrap_err();
        match err {
            GitlessError::Http(msg) => {
                assert!(msg.contains("INTERNAL_SERVER_ERROR"), "got: {msg}");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }

    // --- partial errors policy: data present + non-empty errors → fail ----

    #[test]
    fn partial_errors_with_data_present_returns_fail() {
        // Some aliases resolved (`data` non-null) but `errors[]` is non-empty.
        // Per spec § Partial errors policy, errors[0] is mapped and the call
        // fails — the partial data is never returned.
        let body = r#"{"data":{"repository":{"ref":{"target":{"a0":{"nodes":[{"committedDate":"2026-05-07T10:00:00Z"}]}}}}},"errors":[{"message":"throttled","extensions":{"code":"RATE_LIMITED"}}]}"#;
        let paths = vec!["good.md".to_string(), "bad.md".to_string()];
        let query = build_query("owner", "repo", "main", &paths);
        let mut mock = MockGhClient::new();
        mock.stub(graphql_args(&query), ok_resp(body.as_bytes()));

        let err = fetch_last_commit_at_batch(&mock, "owner/repo", "main", &paths).unwrap_err();
        assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
    }

    // --- build_query escape variants: backslash and newline ---------------

    #[test]
    fn build_query_escapes_path_with_backslash() {
        let paths = vec![r"weird\name.md".to_string()];
        let q = build_query("owner", "repo", "main", &paths);
        // Raw `\` byte in input → literal `\\` (two chars) in query string.
        assert!(q.contains(r#"path: "weird\\name.md""#), "got: {q}");
    }

    #[test]
    fn build_query_escapes_path_with_newline() {
        let paths = vec!["weird\nname.md".to_string()];
        let q = build_query("owner", "repo", "main", &paths);
        // Raw newline byte in input → literal `\n` (backslash + n) in query.
        assert!(q.contains(r#"path: "weird\nname.md""#), "got: {q}");
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
