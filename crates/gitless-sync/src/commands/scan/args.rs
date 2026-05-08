//! User-facing argument types for the `scan` command.
//!
//! Lives in its own file so `pipeline.rs` and `commits.rs` can import [`Backend`]
//! without going through `mod.rs` (avoids a `mod.rs ↔ pipeline.rs` cycle).

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "lowercase")]
pub enum Backend {
    #[default]
    Rest,
    Graphql,
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
    pub status: Option<String>,
    pub backend: Backend,
    pub verbose: u8,
}
