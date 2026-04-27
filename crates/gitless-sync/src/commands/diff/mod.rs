use crate::shared::error::GitlessError;

#[derive(Debug)]
pub struct DiffArgs {
    pub repo: Option<String>,
    pub branch: String,
    pub local: String,
    pub token: Option<String>,
    pub keep_bom: bool,
    pub path: String,
}

pub fn run(args: DiffArgs) -> Result<(), GitlessError> {
    let _ = args;
    todo!("diff: fetch remote blob, normalize both sides, print unified diff to stdout")
}
