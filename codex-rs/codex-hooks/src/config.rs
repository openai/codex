//! JSON configuration types for hooks.
//!
//! Aligned with Claude Code's hooks.json format.
//!
//! ## File Locations
//!
//! - Project: `.codex/hooks.json`
//! - User: `~/.codex/hooks.json`
//!
//! ## Example Configuration
//!
//! ```json
//! {
//!   "disableAllHooks": false,
//!   "shellPrefix": "/optional/wrapper.sh",
//!   "hooks": {
//!     "PreToolUse": [
//!       {
//!         "matcher": "Bash|Write",
//!         "hooks": [
//!           {
//!             "type": "command",
//!             "command": "~/.codex/hooks/check.sh",
//!             "timeout": 30,
//!             "statusMessage": "Checking..."
//!           }
//!         ]
//!       }
//!     ]
//!   }
//! }
//! ```

use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

/// Root configuration for hooks.json file.
///
/// Aligned with Claude Code's settings.json hooks format.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HooksJsonConfig {
    /// Global kill switch for all hooks.
    #[serde(default)]
    pub disable_all_hooks: bool,

    /// Optional shell prefix that wraps all hook commands.
    /// Replaces the CODEX_CODE_SHELL_PREFIX environment variable.
    #[serde(default)]
    pub shell_prefix: Option<String>,

    /// Hook definitions by event type.
    /// Keys are event type names: PreToolUse, PostToolUse, etc.
    #[serde(default)]
    pub hooks: HashMap<String, Vec<HookMatcherJson>>,
}

/// Matcher configuration with nested hooks array.
///
/// Aligned with Claude Code's nested hooks structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookMatcherJson {
    /// Pattern to match against.
    /// - Empty or "*" matches all
    /// - Pipe-separated like "Bash|Write|Edit"
    /// - Regex pattern
    #[serde(default)]
    pub matcher: String,

    /// List of hooks to execute when pattern matches.
    /// Multiple hooks can be defined per matcher.
    pub hooks: Vec<HookConfigJson>,
}

/// Individual hook configuration.
///
/// Supports command hooks (future: prompt, agent).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookConfigJson {
    /// Hook type: "command" (future: "prompt", "agent").
    #[serde(rename = "type")]
    pub hook_type: String,

    /// Shell command to execute (for command hooks).
    #[serde(default)]
    pub command: Option<String>,

    /// Timeout in seconds (default: 60).
    #[serde(default = "default_timeout")]
    pub timeout: i32,

    /// Optional status message for UI display.
    #[serde(default)]
    pub status_message: Option<String>,
}

fn default_timeout() -> i32 {
    60
}

impl HooksJsonConfig {
    /// Check if hooks are globally disabled.
    pub fn is_disabled(&self) -> bool {
        self.disable_all_hooks
    }

    /// Get the shell prefix if configured.
    pub fn get_shell_prefix(&self) -> Option<&str> {
        self.shell_prefix.as_deref()
    }

    /// Get hooks for a specific event type.
    pub fn get_hooks(&self, event_type: &str) -> Option<&Vec<HookMatcherJson>> {
        self.hooks.get(event_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_config() {
        let json = "{}";
        let config: HooksJsonConfig = serde_json::from_str(json).unwrap();
        assert!(!config.disable_all_hooks);
        assert!(config.shell_prefix.is_none());
        assert!(config.hooks.is_empty());
    }

    #[test]
    fn test_parse_full_config() {
        let json = r#"{
            "disableAllHooks": false,
            "shellPrefix": "/usr/local/bin/wrapper.sh",
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash|Write",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "echo test",
                                "timeout": 30,
                                "statusMessage": "Testing..."
                            }
                        ]
                    }
                ]
            }
        }"#;

        let config: HooksJsonConfig = serde_json::from_str(json).unwrap();
        assert!(!config.disable_all_hooks);
        assert_eq!(
            config.shell_prefix,
            Some("/usr/local/bin/wrapper.sh".to_string())
        );

        let pre_tool_use = config.get_hooks("PreToolUse").unwrap();
        assert_eq!(pre_tool_use.len(), 1);
        assert_eq!(pre_tool_use[0].matcher, "Bash|Write");
        assert_eq!(pre_tool_use[0].hooks.len(), 1);
        assert_eq!(pre_tool_use[0].hooks[0].hook_type, "command");
        assert_eq!(
            pre_tool_use[0].hooks[0].command,
            Some("echo test".to_string())
        );
        assert_eq!(pre_tool_use[0].hooks[0].timeout, 30);
    }

    #[test]
    fn test_default_timeout() {
        let json = r#"{
            "hooks": {
                "SessionStart": [
                    {
                        "matcher": "*",
                        "hooks": [
                            {
                                "type": "command",
                                "command": "echo init"
                            }
                        ]
                    }
                ]
            }
        }"#;

        let config: HooksJsonConfig = serde_json::from_str(json).unwrap();
        let session_start = config.get_hooks("SessionStart").unwrap();
        assert_eq!(session_start[0].hooks[0].timeout, 60);
    }

    #[test]
    fn test_nested_hooks_array() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            {"type": "command", "command": "cmd1"},
                            {"type": "command", "command": "cmd2"},
                            {"type": "command", "command": "cmd3"}
                        ]
                    }
                ]
            }
        }"#;

        let config: HooksJsonConfig = serde_json::from_str(json).unwrap();
        let pre_tool_use = config.get_hooks("PreToolUse").unwrap();
        assert_eq!(pre_tool_use[0].hooks.len(), 3);
    }

    #[test]
    fn test_disable_all_hooks() {
        let json = r#"{"disableAllHooks": true}"#;
        let config: HooksJsonConfig = serde_json::from_str(json).unwrap();
        assert!(config.is_disabled());
    }
}
