use serde::Deserialize;
use serde::Serialize;

/// Extra configuration fields for an internally hosted thread.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtraConfig {
    /// User message that should start the next turn when the thread becomes idle.
    #[serde(default)]
    pub persistent_mode_message: Option<String>,
}
