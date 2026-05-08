//! GraphQL query string construction for `fetch_last_commit_at_batch`.
//!
//! Pure functions: split a `owner/name` repo string, escape user-provided
//! paths for embedding in GraphQL string literals, and build the alias-
//! batched query for one chunk. No IO, no `gh` subprocess.

use std::fmt::Write as _;

use crate::shared::error::GitlessError;

pub(super) fn split_repo(repo: &str) -> Result<(&str, &str), GitlessError> {
    repo.split_once('/').ok_or_else(|| {
        GitlessError::Config(format!("invalid repo format: {repo} (expected owner/name)"))
    })
}

pub(super) fn escape_graphql_string(s: &str) -> String {
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

pub(super) fn build_query(owner: &str, name: &str, branch: &str, paths: &[String]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(q.contains(r#"path: "weird\"name.md""#), "got: {q}");
    }

    #[test]
    fn build_query_escapes_path_with_backslash() {
        let paths = vec![r"weird\name.md".to_string()];
        let q = build_query("owner", "repo", "main", &paths);
        assert!(q.contains(r#"path: "weird\\name.md""#), "got: {q}");
    }

    #[test]
    fn build_query_escapes_path_with_newline() {
        let paths = vec!["weird\nname.md".to_string()];
        let q = build_query("owner", "repo", "main", &paths);
        assert!(q.contains(r#"path: "weird\nname.md""#), "got: {q}");
    }

    // --- split_repo --------------------------------------------------------

    #[test]
    fn split_repo_returns_owner_and_name() {
        assert_eq!(split_repo("owner/repo").unwrap(), ("owner", "repo"));
    }

    #[test]
    fn split_repo_without_slash_returns_config_error() {
        let err = split_repo("no-slash").unwrap_err();
        match err {
            GitlessError::Config(msg) => {
                assert!(msg.contains("owner/name"), "got: {msg}");
            }
            other => panic!("expected Config, got {other:?}"),
        }
    }
}
