//! Leaf data types for the `hash_pass` module. Holding [`PreState`] /
//! [`PreEntry`] in their own module keeps `cargo xtask check-cycles`
//! acyclic — both `super::mod` and `super::local` depend on this leaf
//! and never on each other in the import graph.

use chrono::{DateTime, Utc};

use crate::commands::scan::compare::FailedReason;

/// Per-path intermediate state — either short-circuited (no hash, fail
/// reason already locked in) or hashed (caller will classify against
/// remote sha + commit timestamp in [`super::super::finalize`]).
///
/// `is_binary` semantics (spec-output-schema.md § null 정책 + EE):
/// reflects the local-bytes NUL heuristic when a local read occurred. For
/// `Failed`, that means only the `Encoding` arm — every other Failed
/// reason short-circuits before any local read, so `is_binary: false` is
/// the only honest value (no measurement). `Hashed` always has a measured
/// value; the `local: None` arm (remote-only path) carries `false` for
/// the same "no measurement" reason.
pub(in crate::commands::scan::pipeline) enum PreState {
    Failed {
        remote_sha: Option<String>,
        local_mtime: Option<DateTime<Utc>>,
        failed_reason: Option<FailedReason>,
        is_binary: bool,
        size_bytes: Option<u64>,
    },
    Hashed {
        local_sha: Option<String>,
        remote_sha: Option<String>,
        local_mtime: Option<DateTime<Utc>>,
        is_binary: bool,
    },
}

pub(in crate::commands::scan::pipeline) struct PreEntry {
    pub(in crate::commands::scan::pipeline) path: String,
    pub(in crate::commands::scan::pipeline) mode: String,
    pub(in crate::commands::scan::pipeline) state: PreState,
}
