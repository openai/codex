use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CustomPrompt {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
}
