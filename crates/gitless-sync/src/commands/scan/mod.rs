pub mod compare;
pub mod github;
pub mod output;
pub mod walker;

use crate::shared::error::GitlessError;

#[derive(Debug)]
pub struct ScanArgs {
    pub repo: Option<String>,
    pub branch: String,
    pub local: String,
    pub ignore: Vec<String>,
    pub token: Option<String>,
    pub keep_bom: bool,
    pub pretty: bool,
    pub summary_only: bool,
    pub status: Option<String>,
}

pub fn run(args: ScanArgs) -> Result<(), GitlessError> {
    let _ = args;
    todo!("scan: orchestrate github::fetch_tree + walker::walk + compare + output")
}
