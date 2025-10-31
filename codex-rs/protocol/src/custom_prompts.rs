use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;
use ts_rs::TS;

/// Base namespace for custom prompt slash commands (without trailing colon).
/// Example usage forms constructed in code:
/// - Command token after '/': `"{PROMPTS_CMD_PREFIX}:name"`
/// - Full slash prefix: `"/{PROMPTS_CMD_PREFIX}:"`
pub const PROMPTS_CMD_PREFIX: &str = "prompts";

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema, TS)]
pub struct CustomPrompt {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
    pub description: Option<String>,
    pub argument_hint: Option<String>,
}
