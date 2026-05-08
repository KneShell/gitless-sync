//! Crate-wide error type, split into domain sub-modules.
//!
//! - `core` — `GitlessError` enum + exit code / error code / stderr payload.
//! - `network` — GraphQL response error shape and mapping.
//!
//! Callers import from `crate::shared::error::*` directly; the re-exports
//! below preserve the flat path regardless of which sub-module owns the
//! item.

pub mod core;
pub mod network;

pub use core::{GitlessError, StderrPayload};
pub use network::{GraphqlError, GraphqlErrorExtensions, map_graphql_error};
