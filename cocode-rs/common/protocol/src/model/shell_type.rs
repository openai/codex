//! Shell execution type configuration.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};

/// Shell execution capability for a model.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Display, EnumIter,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ConfigShellToolType {
    /// Default shell execution.
    #[default]
    Default,
    /// Local shell execution.
    Local,
    /// Unified exec mode.
    UnifiedExec,
    /// Shell execution disabled.
    Disabled,
    /// Shell command mode.
    ShellCommand,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        assert_eq!(ConfigShellToolType::default(), ConfigShellToolType::Default);
    }

    #[test]
    fn test_serde() {
        let shell_type = ConfigShellToolType::ShellCommand;
        let json = serde_json::to_string(&shell_type).expect("serialize");
        assert_eq!(json, "\"shell_command\"");

        let parsed: ConfigShellToolType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, ConfigShellToolType::ShellCommand);
    }
}
