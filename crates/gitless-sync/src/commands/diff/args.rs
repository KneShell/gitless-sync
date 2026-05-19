//! User-facing argument types for the `diff` command.
//!
//! Lives in its own file so `compute.rs` can import [`DiffArgs`] without
//! going through `mod.rs` (avoids a `mod.rs ↔ compute.rs` cycle).

#[derive(Debug)]
pub struct DiffArgs {
    /// `GitHub` repository (`owner/name`). `None` falls back to env / config.
    pub repo: Option<String>,
    /// Branch name to compare against (defaults to `main` upstream).
    pub branch: String,
    /// Local directory the file is read from.
    pub local: String,
    /// Preserve `UTF-8` BOM when reading text files.
    pub keep_bom: bool,
    /// Relative path (forward slash) of the file to diff.
    pub path: String,
    /// Optional remote-side path override for cross-path comparison
    /// (rename/move scenarios). `None` falls back to `path`.
    pub remote_path: Option<String>,
    /// Emit `JSON` output instead of unified text (opt-in).
    pub json: bool,
}
