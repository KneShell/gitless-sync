//! User-facing argument types for the `scan` command.
//!
//! Lives in its own file so `pipeline.rs` and `commits.rs` can import [`Backend`]
//! without going through `mod.rs` (avoids a `mod.rs ↔ pipeline.rs` cycle).

use super::compare::Status;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "lowercase")]
pub enum Backend {
    #[default]
    Rest,
    Graphql,
}

/// `clap`-facing companion to `Status` for the `--status` filter.
///
/// Defined separately so domain `Status` stays free of `clap` derives and
/// the CLI surface picks up `--help [possible values: ...]` plus
/// "possible values" echo on invalid input automatically. The variants
/// mirror the 5 `Status` cases 1:1; conversion lives in `mod.rs` so the
/// `From` impl doesn't drag a `compare::types -> args` module edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "snake_case")]
pub enum StatusFilter {
    Identical,
    LocalOnlyChanged,
    RemoteOnlyChanged,
    Drift,
    Failed,
}

/// Map clap-parsed `Vec<StatusFilter>` (empty when `--status` absent) to the
/// `Option<Vec<StatusFilter>>` that [`ScanArgs`] consumes — `None` = no filter.
#[must_use]
pub fn collect_status_filter(filters: Vec<StatusFilter>) -> Option<Vec<StatusFilter>> {
    if filters.is_empty() {
        None
    } else {
        Some(filters)
    }
}

/// Free-function conversion from CLI surface [`StatusFilter`] to the domain
/// [`Status`] enum. Defined as a plain `fn` (not `From` impl) so the
/// module-graph edge stays uni-directional `args -> compare::types` —
/// trait impls trigger an implicit reverse edge that breaks
/// `cargo xtask check-cycles`.
#[must_use]
pub fn to_status(filter: StatusFilter) -> Status {
    match filter {
        StatusFilter::Identical => Status::Identical,
        StatusFilter::LocalOnlyChanged => Status::LocalOnlyChanged,
        StatusFilter::RemoteOnlyChanged => Status::RemoteOnlyChanged,
        StatusFilter::Drift => Status::Drift,
        StatusFilter::Failed => Status::Failed,
    }
}

#[derive(Debug)]
pub struct ScanArgs {
    pub repo: Option<String>,
    pub branch: String,
    pub local: String,
    pub ignore: Vec<String>,
    pub keep_bom: bool,
    pub pretty: bool,
    pub summary_only: bool,
    pub status: Option<Vec<StatusFilter>>,
    pub backend: Backend,
    pub verbose: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_status_filter_returns_none_when_empty() {
        assert!(collect_status_filter(vec![]).is_none());
    }

    #[test]
    fn collect_status_filter_returns_some_when_non_empty() {
        assert_eq!(
            collect_status_filter(vec![StatusFilter::Drift]),
            Some(vec![StatusFilter::Drift])
        );
    }

    #[test]
    fn to_status_maps_each_variant_to_corresponding_status() {
        assert_eq!(to_status(StatusFilter::Identical), Status::Identical);
        assert_eq!(
            to_status(StatusFilter::LocalOnlyChanged),
            Status::LocalOnlyChanged
        );
        assert_eq!(
            to_status(StatusFilter::RemoteOnlyChanged),
            Status::RemoteOnlyChanged
        );
        assert_eq!(to_status(StatusFilter::Drift), Status::Drift);
        assert_eq!(to_status(StatusFilter::Failed), Status::Failed);
    }
}
