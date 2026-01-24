//! Hook injection.

use crate::error::Result;
use crate::loader::PluginHook;

/// Injected hook ready for HookRegistry.
#[derive(Debug, Clone)]
pub struct InjectedHook {
    /// Hook event type (PreToolUse, PostToolUse, etc.).
    pub event_type: String,
    /// Matcher pattern.
    pub matcher: String,
    /// Hook type (command, script, http).
    pub hook_type: HookType,
    /// Source plugin ID.
    pub source_plugin: String,
    /// Timeout in milliseconds.
    pub timeout: Option<i32>,
}

impl InjectedHook {
    /// Convert to codex-hooks types for registration.
    ///
    /// Returns `(HookEventType, HookMatcher)` suitable for `HookRegistry::register_global`.
    pub fn to_hook_config(&self) -> Result<(codex_hooks::HookEventType, codex_hooks::HookMatcher)> {
        use codex_hooks::HookConfig;
        use codex_hooks::HookMatcher;
        use codex_hooks::HookType as CoreHookType;

        let event_type = parse_event_type(&self.event_type)?;

        // Convert timeout from ms to seconds (default 60s)
        let timeout_secs = self.timeout.map(|ms| (ms / 1000) as u32).unwrap_or(60);

        let hook_type = match &self.hook_type {
            HookType::Command { command } => CoreHookType::Command {
                command: command.clone(),
                timeout_secs,
                status_message: None,
            },
            HookType::Script { path } => {
                // Scripts are executed via shell
                CoreHookType::Command {
                    command: format!("sh {path}"),
                    timeout_secs,
                    status_message: None,
                }
            }
            HookType::Http { url } => {
                // HTTP hooks are executed via curl
                CoreHookType::Command {
                    command: format!("curl -sX POST {url}"),
                    timeout_secs: 30,
                    status_message: None,
                }
            }
        };

        let config = HookConfig {
            hook_type,
            on_success: None,
        };

        let matcher = HookMatcher {
            matcher: self.matcher.clone(),
            hooks: vec![config],
        };

        Ok((event_type, matcher))
    }
}

/// Parse event type string to HookEventType.
fn parse_event_type(name: &str) -> Result<codex_hooks::HookEventType> {
    use codex_hooks::HookEventType;

    match name {
        "PreToolUse" => Ok(HookEventType::PreToolUse),
        "PostToolUse" => Ok(HookEventType::PostToolUse),
        "PostToolUseFailure" => Ok(HookEventType::PostToolUseFailure),
        "SessionStart" => Ok(HookEventType::SessionStart),
        "SessionEnd" => Ok(HookEventType::SessionEnd),
        "Stop" => Ok(HookEventType::Stop),
        "SubagentStart" => Ok(HookEventType::SubagentStart),
        "SubagentStop" => Ok(HookEventType::SubagentStop),
        "UserPromptSubmit" => Ok(HookEventType::UserPromptSubmit),
        "Notification" => Ok(HookEventType::Notification),
        "PreCompact" => Ok(HookEventType::PreCompact),
        "PermissionRequest" => Ok(HookEventType::PermissionRequest),
        _ => Err(crate::error::PluginError::InvalidManifest {
            path: "hook".into(),
            reason: format!("Unknown hook event type: {name}"),
        }),
    }
}

/// Hook type.
#[derive(Debug, Clone)]
pub enum HookType {
    /// Shell command execution.
    Command { command: String },
    /// Script execution.
    Script { path: String },
    /// HTTP request.
    Http { url: String },
}

/// Convert a plugin hook to injectable format.
pub fn convert_hook(hook: &PluginHook) -> Result<InjectedHook> {
    let hook_type = if let Some(ref cmd) = hook.config.command {
        HookType::Command {
            command: cmd.clone(),
        }
    } else if let Some(ref script) = hook.config.script {
        HookType::Script {
            path: script.clone(),
        }
    } else if let Some(ref url) = hook.config.url {
        HookType::Http { url: url.clone() }
    } else {
        // Default to empty command
        HookType::Command {
            command: String::new(),
        }
    };

    Ok(InjectedHook {
        event_type: hook.event_type.clone(),
        matcher: hook.matcher.clone(),
        hook_type,
        source_plugin: hook.source_plugin.clone(),
        timeout: hook.config.timeout,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::PluginHookConfig;

    #[test]
    fn test_convert_hook_command() {
        let plugin_hook = PluginHook {
            event_type: "PreToolUse".to_string(),
            matcher: "*".to_string(),
            config: PluginHookConfig {
                hook_type: "command".to_string(),
                command: Some("echo hello".to_string()),
                script: None,
                url: None,
                timeout: Some(5000),
            },
            source_plugin: "test-plugin".to_string(),
        };

        let injected = convert_hook(&plugin_hook).unwrap();
        assert_eq!(injected.event_type, "PreToolUse");
        assert!(matches!(injected.hook_type, HookType::Command { .. }));
        assert_eq!(injected.timeout, Some(5000));
    }

    #[test]
    fn test_convert_hook_http() {
        let plugin_hook = PluginHook {
            event_type: "PostToolUse".to_string(),
            matcher: "shell".to_string(),
            config: PluginHookConfig {
                hook_type: "http".to_string(),
                command: None,
                script: None,
                url: Some("https://example.com/hook".to_string()),
                timeout: None,
            },
            source_plugin: "test-plugin".to_string(),
        };

        let injected = convert_hook(&plugin_hook).unwrap();
        assert!(matches!(injected.hook_type, HookType::Http { .. }));
    }
}
