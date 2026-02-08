//! Shell execution type configuration.

use serde::Deserialize;
use serde::Serialize;
use strum::Display;
use strum::EnumIter;

/// Shell execution capability for a model.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, Display, EnumIter,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ConfigShellToolType {
    /// Shell command mode — single string command, executed via `bash -c`.
    /// Used by BashTool and most modern models (GPT-5.x, codex series).
    #[default]
    ShellCommand,

    /// Basic shell mode — array-based command format (e.g. `["bash", "-lc", "ls"]`).
    /// Used by legacy models (o3, gpt-4.x) in codex-rs.
    Shell,

    /// Shell execution disabled — no shell tool sent to model.
    Disabled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        assert_eq!(
            ConfigShellToolType::default(),
            ConfigShellToolType::ShellCommand
        );
    }

    #[test]
    fn test_serde() {
        let shell_type = ConfigShellToolType::Shell;
        let json = serde_json::to_string(&shell_type).expect("serialize");
        assert_eq!(json, "\"shell\"");

        let parsed: ConfigShellToolType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, ConfigShellToolType::Shell);
    }
}
