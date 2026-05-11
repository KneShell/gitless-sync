use chrono::{DateTime, Utc};

use super::types::Presence;
use super::types::Status;

/// Classify a single path into one of the 4-state categories.
///
/// `normalize_equal == Some(true)` promotes a sha-differ entry to
/// [`Status::Identical`] — F1 cosmetic drift case (`spec-output-schema.md`
/// § v1.4): raw SHAs differ (Trees-API blob hash) but `prepare_for_hash`
/// (BOM strip + LF normalize + `.gitattributes` policy) yields identical
/// self-defined hashes. byte-identical files with cosmetic SHA mismatch
/// (UTF-8 BOM, CRLF/LF) classify as [`Status::Identical`] per
/// `spec-hash-and-normalize.md` § 목적.
///
/// # Panics
/// Panics when both `local_sha` and `remote_sha` are `None` — the caller
/// guarantees at least one side is present (`spec-classification.md`).
#[must_use]
pub fn classify(
    local_sha: Option<&str>,
    remote_sha: Option<&str>,
    local_mtime: Option<DateTime<Utc>>,
    remote_last_commit_at: Option<DateTime<Utc>>,
    normalize_equal: Option<bool>,
) -> Status {
    match (local_sha, remote_sha) {
        (Some(a), Some(b)) if a == b => Status::Identical,
        (Some(_), Some(_)) if normalize_equal == Some(true) => Status::Identical,
        (Some(_), None) => Status::LocalOnlyChanged,
        (None, Some(_)) => Status::RemoteOnlyChanged,
        (Some(_), Some(_)) => match (local_mtime, remote_last_commit_at) {
            (Some(l), Some(r)) if r < l => Status::LocalOnlyChanged,
            (Some(l), Some(r)) if l < r => Status::RemoteOnlyChanged,
            _ => Status::Drift,
        },
        (None, None) => unreachable!("classify must not be called with both SHAs absent"),
    }
}

/// Determine `(Presence, diff_meaningful)` for a Hashed-side path.
///
/// `normalize_equal` is consulted only when both shas exist AND differ;
/// `Some(true)` → no semantic diff (F1 BOM/encoding-only drift case),
/// `Some(false)` → real diff. For `Failed` entries the caller must bypass
/// this function and set `presence` directly + `diff_meaningful = None`
/// (`Failed` has no comparable bytes available).
///
/// Truth table (`spec-output-schema.md` § v1.3):
///
/// | `local_sha` | `remote_sha` | shas eq | `normalize_equal` | `presence`   | `diff_meaningful` |
/// |-------------|--------------|---------|-------------------|--------------|-------------------|
/// | `Some`      | `Some`       | yes     | `_`               | `Both`       | `Some(false)`     |
/// | `Some`      | `Some`       | no      | `Some(true)`      | `Both`       | `Some(false)`     |
/// | `Some`      | `Some`       | no      | `Some(false)`     | `Both`       | `Some(true)`      |
/// | `Some`      | `Some`       | no      | `None`            | `Both`       | `None`            |
/// | `Some`      | `None`       | `_`     | `_`               | `LocalOnly`  | `None`            |
/// | `None`      | `Some`       | `_`     | `_`               | `RemoteOnly` | `None`            |
///
/// # Panics
/// Panics when both `local_sha` and `remote_sha` are `None` — same
/// invariant as [`classify`].
#[must_use]
pub fn compare(
    local_sha: Option<&str>,
    remote_sha: Option<&str>,
    normalize_equal: Option<bool>,
) -> (Presence, Option<bool>) {
    match (local_sha, remote_sha) {
        (Some(l), Some(r)) => {
            let dm = if l == r {
                Some(false)
            } else {
                normalize_equal.map(|eq| !eq)
            };
            (Presence::Both, dm)
        }
        (Some(_), None) => (Presence::LocalOnly, None),
        (None, Some(_)) => (Presence::RemoteOnly, None),
        (None, None) => unreachable!("compare must not be called with both SHAs absent"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    // --- classify -------------------------------------------------------

    #[test]
    fn identical_when_shas_match() {
        let s = classify(Some("abc"), Some("abc"), Some(ts(100)), Some(ts(200)), None);
        assert_eq!(s, Status::Identical);
    }

    #[test]
    fn identical_ignores_timestamps() {
        let s = classify(Some("deadbeef"), Some("deadbeef"), None, None, None);
        assert_eq!(s, Status::Identical);
    }

    #[test]
    fn local_only_when_remote_missing() {
        let s = classify(Some("abc"), None, Some(ts(100)), None, None);
        assert_eq!(s, Status::LocalOnlyChanged);
    }

    #[test]
    fn remote_only_when_local_missing() {
        let s = classify(None, Some("abc"), None, Some(ts(100)), None);
        assert_eq!(s, Status::RemoteOnlyChanged);
    }

    #[test]
    fn local_only_when_remote_older() {
        let s = classify(Some("a"), Some("b"), Some(ts(200)), Some(ts(100)), None);
        assert_eq!(s, Status::LocalOnlyChanged);
    }

    #[test]
    fn remote_only_when_local_older() {
        let s = classify(Some("a"), Some("b"), Some(ts(100)), Some(ts(200)), None);
        assert_eq!(s, Status::RemoteOnlyChanged);
    }

    #[test]
    fn drift_on_equal_timestamps() {
        let s = classify(Some("a"), Some("b"), Some(ts(100)), Some(ts(100)), None);
        assert_eq!(s, Status::Drift);
    }

    #[test]
    fn drift_when_local_mtime_missing() {
        let s = classify(Some("a"), Some("b"), None, Some(ts(100)), None);
        assert_eq!(s, Status::Drift);
    }

    #[test]
    fn drift_when_remote_commit_missing() {
        let s = classify(Some("a"), Some("b"), Some(ts(100)), None, None);
        assert_eq!(s, Status::Drift);
    }

    #[test]
    fn drift_when_both_times_missing() {
        let s = classify(Some("a"), Some("b"), None, None, None);
        assert_eq!(s, Status::Drift);
    }

    #[test]
    fn identical_when_normalize_equal_despite_sha_differ() {
        // F1 cosmetic drift case (issue #1): raw SHAs differ (Trees-API blob
        // hash) but `prepare_for_hash` yields identical self-defined hashes
        // (UTF-8 BOM strip / LF normalize). spec-hash-and-normalize.md § 목적
        // 정합 — byte-identical files must classify as Identical regardless
        // of cosmetic SHA drift, overriding the timestamp arm.
        let s = classify(
            Some("a"),
            Some("b"),
            Some(ts(200)),
            Some(ts(100)),
            Some(true),
        );
        assert_eq!(s, Status::Identical);
    }

    #[test]
    fn normalize_equal_some_false_falls_through_to_timestamp_arm() {
        // Real semantic drift: shas differ AND normalize-equal=false → fall
        // through to the timestamp arm (LocalOnlyChanged here, local newer).
        // Confirms the new arm only fires for `Some(true)`, not `Some(false)`.
        let s = classify(
            Some("a"),
            Some("b"),
            Some(ts(200)),
            Some(ts(100)),
            Some(false),
        );
        assert_eq!(s, Status::LocalOnlyChanged);
    }

    #[test]
    #[should_panic(expected = "classify must not be called with both SHAs absent")]
    fn panics_when_both_shas_missing() {
        let _ = classify(None, None, Some(ts(100)), Some(ts(100)), None);
    }

    // --- compare --------------------------------------------------------

    #[test]
    fn compare_identical_returns_both_and_diff_meaningful_false() {
        // Identical entry (presence=both, sha same) → diff_meaningful=Some(false).
        assert_eq!(
            compare(Some("a"), Some("a"), None),
            (Presence::Both, Some(false))
        );
    }

    #[test]
    fn compare_sha_differ_normalize_equal_returns_diff_meaningful_false() {
        // F1 evidence — BOM/encoding-only drift: shas differ but normalize-equal
        // → diff stdout 0 bytes. caller treats as "no semantic diff".
        assert_eq!(
            compare(Some("a"), Some("b"), Some(true)),
            (Presence::Both, Some(false))
        );
    }

    #[test]
    fn compare_sha_differ_normalize_diff_returns_diff_meaningful_true() {
        // Real semantic drift — diff would emit unified text.
        assert_eq!(
            compare(Some("a"), Some("b"), Some(false)),
            (Presence::Both, Some(true))
        );
    }

    #[test]
    fn compare_sha_differ_normalize_equal_unknown_returns_none() {
        // Defensive: caller could not compute normalize-equal hint → omit
        // diff_meaningful rather than guessing.
        assert_eq!(compare(Some("a"), Some("b"), None), (Presence::Both, None));
    }

    #[test]
    fn compare_local_only_returns_local_only_presence_and_diff_meaningful_none() {
        assert_eq!(compare(Some("a"), None, None), (Presence::LocalOnly, None));
    }

    #[test]
    fn compare_remote_only_returns_remote_only_presence_and_diff_meaningful_none() {
        assert_eq!(compare(None, Some("b"), None), (Presence::RemoteOnly, None));
    }

    #[test]
    fn compare_ignores_normalize_equal_for_single_side() {
        // normalize_equal is consulted only when both shas exist AND differ.
        // For single-side it must be ignored — diff_meaningful stays None.
        assert_eq!(
            compare(Some("a"), None, Some(true)),
            (Presence::LocalOnly, None)
        );
        assert_eq!(
            compare(None, Some("b"), Some(false)),
            (Presence::RemoteOnly, None)
        );
    }

    #[test]
    #[should_panic(expected = "compare must not be called with both SHAs absent")]
    fn compare_panics_when_both_shas_missing() {
        let _ = compare(None, None, Some(true));
    }
}
