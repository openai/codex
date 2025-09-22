use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
pub struct CustomPrompt {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
    /// Optional short description (from front matter) to show in the UI popup.
    pub description: Option<String>,
}
