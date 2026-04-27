use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::shared::error::GitlessError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub repo: Option<String>,
    pub branch: Option<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
}

pub fn load(path: Option<&Path>) -> Result<Config, GitlessError> {
    let _ = path;
    todo!("load gitless-sync.toml if present; return default if absent")
}
