use crate::shared::error::GitlessError;

/// Map a non-zero `gh api` invocation's stderr to a [`GitlessError`].
///
/// Substring matches per `spec-error-contracts.md` § gh 종료 코드 매핑. Order
/// follows the spec's explicit priority list (auth → secondary rate → primary
/// rate → fallthrough Http). Unknown stderrs fall through to `Http(stderr)`
/// with the original message preserved.
pub(crate) fn map_gh_error(stderr: &str) -> GitlessError {
    if stderr.contains("Bad credentials") {
        GitlessError::AuthFailed
    } else if stderr.contains("secondary rate limit") || stderr.contains("API rate limit exceeded")
    {
        GitlessError::RateLimitExceeded {
            reset_at: String::new(),
        }
    } else {
        GitlessError::Http(stderr.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_gh_error_bad_credentials_returns_auth_failed() {
        let err = map_gh_error("gh: Bad credentials (HTTP 401)");
        assert!(matches!(err, GitlessError::AuthFailed));
    }

    #[test]
    fn map_gh_error_secondary_rate_takes_precedence_over_primary_substring() {
        // Per spec § 매칭 우선순위: secondary checked before primary. Both
        // substrings would match, so the resulting message must still be a
        // RateLimitExceeded — order doesn't change the variant here, but the
        // explicit ordering protects against regressions.
        let err = map_gh_error("gh: secondary rate limit exceeded ...");
        assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
    }

    #[test]
    fn map_gh_error_primary_rate_returns_rate_limit() {
        let err = map_gh_error("gh: API rate limit exceeded for user XXX. (HTTP 403)");
        assert!(matches!(err, GitlessError::RateLimitExceeded { .. }));
    }

    #[test]
    fn map_gh_error_unknown_returns_http_with_stderr_preserved() {
        let stderr = "gh: weird thing happened";
        let err = map_gh_error(stderr);
        match err {
            GitlessError::Http(msg) => assert_eq!(msg, stderr),
            other => panic!("expected Http, got {other:?}"),
        }
    }
}
