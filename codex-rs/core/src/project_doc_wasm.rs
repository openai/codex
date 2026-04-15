use std::io;
use std::path::PathBuf;

use crate::config::Config;

pub(crate) const HIERARCHICAL_AGENTS_MESSAGE: &str = "";
pub const DEFAULT_PROJECT_DOC_FILENAME: &str = "AGENTS.md";
pub const LOCAL_PROJECT_DOC_FILENAME: &str = "AGENTS.override.md";

pub(crate) async fn get_user_instructions(_config: &Config) -> Option<String> {
    None
}

pub async fn read_project_docs(_config: &Config) -> io::Result<Option<String>> {
    Ok(None)
}

pub fn discover_project_doc_paths(_config: &Config) -> io::Result<Vec<PathBuf>> {
    Ok(Vec::new())
}
