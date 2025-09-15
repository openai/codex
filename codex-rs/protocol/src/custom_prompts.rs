use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
pub struct CustomPrompt {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
    // Optional short description shown in the slash popup, typically provided
    // via frontmatter in the prompt file.
    pub description: Option<String>,
    // Optional argument hint (e.g., "[file] [flags]") shown alongside the
    // description in the popup when available.
    pub argument_hint: Option<String>,
}
