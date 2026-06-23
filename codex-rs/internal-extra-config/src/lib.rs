use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// Extra configuration fields for an internally hosted thread.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtraConfig {
    /// User message that should start the next turn when the thread becomes idle.
    #[serde(default)]
    pub persistent_mode_message: Option<String>,
}

/// Extra app-server data for a thread.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[ts(type = "Record<string, never>", export_to = "v2/")]
pub struct ThreadExtra {
    #[schemars(skip)]
    #[ts(skip)]
    #[serde(default, rename = "persistentMode")]
    pub persistent_mode: bool,
}
