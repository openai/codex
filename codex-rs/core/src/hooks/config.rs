use std::time::Duration;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

use super::executor::command_hook;
use super::types::Hook;

/// Single hook entry from configuration.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct HookEntryToml {
    /// The command to execute as argv (program + args).
    pub command: Vec<String>,

    /// Optional timeout in seconds (default: 30).
    #[serde(default = "default_timeout_secs")]
    pub timeout: u64,

    /// Optional matcher pattern for tool-use hooks.
    ///
    /// Supported patterns:
    /// - `"*"` matches any tool name
    /// - `"prefix*"` matches tool names starting with `prefix`
    /// - `"exact"` matches only that exact tool name
    ///
    /// Note: suffix patterns like `"*shell"` and infix patterns like
    /// `"read_*_file"` are **not** supported.
    #[serde(default)]
    pub matcher: Option<String>,
}

fn default_timeout_secs() -> u64 {
    30
}

/// All hook entries grouped by event type.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct HooksConfigToml {
    #[serde(default)]
    pub after_agent: Vec<HookEntryToml>,

    #[serde(default)]
    pub pre_tool_use: Vec<HookEntryToml>,

    #[serde(default)]
    pub post_tool_use: Vec<HookEntryToml>,

    #[serde(default)]
    pub notification: Vec<HookEntryToml>,

    #[serde(default)]
    pub stop: Vec<HookEntryToml>,

    #[serde(default)]
    pub user_prompt_submit: Vec<HookEntryToml>,
}

/// Convert a single HookEntryToml into a Hook via the command executor.
///
/// If the entry has a matcher pattern, the hook will only execute for events
/// whose tool name matches the pattern. Non-tool events always match.
pub(super) fn hook_from_entry(entry: &HookEntryToml) -> Hook {
    let timeout = Duration::from_secs(entry.timeout);
    let inner = command_hook(entry.command.clone(), timeout);
    match &entry.matcher {
        None => inner,
        Some(pattern) => {
            let pattern = pattern.clone();
            Hook {
                func: std::sync::Arc::new(move |payload| {
                    let tool_name = match &payload.hook_event {
                        super::types::HookEvent::PreToolUse { event } => Some(&event.tool_name),
                        super::types::HookEvent::PostToolUse { event } => Some(&event.tool_name),
                        _ => None, // Non-tool events always match
                    };

                    if let Some(name) = tool_name
                        && !matches_pattern(&pattern, name)
                    {
                        return Box::pin(async { super::types::HookOutcome::Proceed });
                    }

                    inner.func.clone()(payload)
                }),
            }
        }
    }
}

/// Check if a tool name matches a simple pattern.
///
/// Supports three forms:
/// - `"*"` matches any tool name.
/// - `"prefix*"` (trailing wildcard) matches tool names starting with `prefix`.
/// - Any other string is compared as an exact match.
///
/// Full glob semantics (suffix, infix wildcards) are intentionally not
/// supported to keep the matching logic trivial and predictable.
fn matches_pattern(pattern: &str, tool_name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return tool_name.starts_with(prefix);
    }
    pattern == tool_name
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    /// Check if a tool name matches a hook entry's matcher pattern.
    /// If matcher is None, the hook matches all tools.
    fn matches_tool(entry: &HookEntryToml, tool_name: &str) -> bool {
        match &entry.matcher {
            None => true,
            Some(pattern) => matches_pattern(pattern, tool_name),
        }
    }

    #[test]
    fn test_hook_entry_deserialize_minimal() {
        let toml_str = r#"
            command = ["./hook.sh"]
        "#;
        let entry: HookEntryToml = toml::from_str(toml_str).unwrap();
        assert_eq!(entry.command, vec!["./hook.sh"]);
        assert_eq!(entry.timeout, 30); // default
        assert_eq!(entry.matcher, None); // default
    }

    #[test]
    fn test_hook_entry_deserialize_full() {
        let toml_str = r#"
            command = ["./pre-tool.sh", "--verbose"]
            timeout = 60
            matcher = "shell*"
        "#;
        let entry: HookEntryToml = toml::from_str(toml_str).unwrap();
        assert_eq!(entry.command, vec!["./pre-tool.sh", "--verbose"]);
        assert_eq!(entry.timeout, 60);
        assert_eq!(entry.matcher, Some("shell*".to_string()));
    }

    #[test]
    fn test_hooks_config_deserialize_empty() {
        let toml_str = "";
        let config: HooksConfigToml = toml::from_str(toml_str).unwrap();
        assert!(config.after_agent.is_empty());
        assert!(config.pre_tool_use.is_empty());
        assert!(config.post_tool_use.is_empty());
        assert!(config.notification.is_empty());
        assert!(config.stop.is_empty());
        assert!(config.user_prompt_submit.is_empty());
    }

    #[test]
    fn test_hooks_config_deserialize_after_agent() {
        let toml_str = r#"
            [[after_agent]]
            command = ["./hook1.sh"]
            timeout = 45

            [[after_agent]]
            command = ["./hook2.sh"]
        "#;
        let config: HooksConfigToml = toml::from_str(toml_str).unwrap();
        assert_eq!(config.after_agent.len(), 2);
        assert_eq!(config.after_agent[0].command, vec!["./hook1.sh"]);
        assert_eq!(config.after_agent[0].timeout, 45);
        assert_eq!(config.after_agent[1].command, vec!["./hook2.sh"]);
        assert_eq!(config.after_agent[1].timeout, 30); // default
    }

    #[test]
    fn test_matches_tool_none_matches_all() {
        let entry = HookEntryToml {
            command: vec!["./hook.sh".to_string()],
            timeout: 30,
            matcher: None,
        };
        assert!(matches_tool(&entry, "shell"));
        assert!(matches_tool(&entry, "read"));
        assert!(matches_tool(&entry, "write"));
    }

    #[test]
    fn test_matches_tool_exact() {
        let entry = HookEntryToml {
            command: vec!["./hook.sh".to_string()],
            timeout: 30,
            matcher: Some("shell".to_string()),
        };
        assert!(matches_tool(&entry, "shell"));
        assert!(!matches_tool(&entry, "shell_exec"));
        assert!(!matches_tool(&entry, "read"));
    }

    #[test]
    fn test_matches_tool_glob_prefix() {
        let entry = HookEntryToml {
            command: vec!["./hook.sh".to_string()],
            timeout: 30,
            matcher: Some("shell*".to_string()),
        };
        assert!(matches_tool(&entry, "shell"));
        assert!(matches_tool(&entry, "shell_exec"));
        assert!(matches_tool(&entry, "shell_command"));
        assert!(!matches_tool(&entry, "read"));
    }

    #[test]
    fn test_matches_tool_wildcard() {
        let entry = HookEntryToml {
            command: vec!["./hook.sh".to_string()],
            timeout: 30,
            matcher: Some("*".to_string()),
        };
        assert!(matches_tool(&entry, "shell"));
        assert!(matches_tool(&entry, "read"));
        assert!(matches_tool(&entry, "write"));
        assert!(matches_tool(&entry, "anything"));
    }

    #[test]
    fn test_matches_tool_no_match() {
        let entry = HookEntryToml {
            command: vec!["./hook.sh".to_string()],
            timeout: 30,
            matcher: Some("read".to_string()),
        };
        assert!(matches_tool(&entry, "read"));
        assert!(!matches_tool(&entry, "write"));
        assert!(!matches_tool(&entry, "read_file"));
    }

    #[test]
    fn test_hooks_config_deserialize_pre_tool_use() {
        let toml_str = r#"
            [[pre_tool_use]]
            command = ["./validate-tool.sh"]
            timeout = 10
            matcher = "bash*"

            [[pre_tool_use]]
            command = ["./log-tool.sh", "--verbose"]
            matcher = "*"
        "#;
        let config: HooksConfigToml = toml::from_str(toml_str).unwrap();
        assert_eq!(config.pre_tool_use.len(), 2);
        assert_eq!(config.pre_tool_use[0].command, vec!["./validate-tool.sh"]);
        assert_eq!(config.pre_tool_use[0].timeout, 10);
        assert_eq!(config.pre_tool_use[0].matcher, Some("bash*".to_string()));
        assert_eq!(
            config.pre_tool_use[1].command,
            vec!["./log-tool.sh", "--verbose"]
        );
        assert_eq!(config.pre_tool_use[1].matcher, Some("*".to_string()));
    }

    #[test]
    fn test_hooks_config_full_deserialize() {
        let toml_str = r#"
            [[after_agent]]
            command = ["./notify.sh"]

            [[pre_tool_use]]
            command = ["./pre-tool.sh"]
            matcher = "bash"

            [[post_tool_use]]
            command = ["./post-tool.sh"]

            [[notification]]
            command = ["./notify-desktop.sh"]

            [[stop]]
            command = ["./cleanup.sh"]

            [[user_prompt_submit]]
            command = ["./log-prompt.sh"]
        "#;
        let config: HooksConfigToml = toml::from_str(toml_str).unwrap();
        assert_eq!(config.after_agent.len(), 1);
        assert_eq!(config.pre_tool_use.len(), 1);
        assert_eq!(config.post_tool_use.len(), 1);
        assert_eq!(config.notification.len(), 1);
        assert_eq!(config.stop.len(), 1);
        assert_eq!(config.user_prompt_submit.len(), 1);
    }

    #[tokio::test]
    async fn test_hook_from_entry_creates_working_hook() {
        let entry = HookEntryToml {
            command: vec!["echo".to_string(), "test".to_string()],
            timeout: 5,
            matcher: None,
        };

        let hook = hook_from_entry(&entry);

        // Create a minimal payload to test hook execution
        use super::super::types::HookEvent;
        use super::super::types::HookEventAfterAgent;
        use super::super::types::HookPayload;
        use chrono::TimeZone;
        use chrono::Utc;
        use codex_protocol::ThreadId;
        use std::path::PathBuf;

        let payload = HookPayload {
            session_id: ThreadId::new(),
            cwd: PathBuf::from("/tmp"),
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::AfterAgent {
                event: HookEventAfterAgent {
                    thread_id: ThreadId::new(),
                    turn_id: "test".to_string(),
                    input_messages: vec!["test".to_string()],
                    last_assistant_message: None,
                },
            },
        };

        // Hook should execute without panicking
        let outcome = hook.execute(&payload).await;

        // command_hook returns Proceed on success
        use super::super::types::HookOutcome;
        assert_eq!(outcome, HookOutcome::Proceed);
    }
}
