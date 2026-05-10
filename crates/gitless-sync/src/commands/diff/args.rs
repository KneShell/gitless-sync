//! User-facing argument types for the `diff` command.
//!
//! Lives in its own file so `compute.rs` can import [`DiffArgs`] without
//! going through `mod.rs` (avoids a `mod.rs ↔ compute.rs` cycle).

#[derive(Debug)]
pub struct DiffArgs {
    pub repo: Option<String>,
    pub branch: String,
    pub local: String,
    pub keep_bom: bool,
    pub path: String,
    pub json: bool,
}
