//! Hook configuration parsing and validation.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::hooks::types::{HookError, HookExecutionMode, HookPriority, HookType, LifecycleEventType};

/// Main hooks configuration structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct HooksConfig {
    /// Global hooks settings.
    #[serde(default)]
    pub hooks: GlobalHooksConfig,
}

/// Global hooks configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlobalHooksConfig {
    /// Whether hooks are enabled globally.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Default timeout for hook execution.
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,

    /// Whether to execute hooks in parallel by default.
    #[serde(default = "default_parallel_execution")]
    pub parallel_execution: bool,

    /// Session lifecycle hooks.
    #[serde(default)]
    pub session: Vec<HookConfig>,

    /// Task lifecycle hooks.
    #[serde(default)]
    pub task: Vec<HookConfig>,

    /// Execution lifecycle hooks.
    #[serde(default)]
    pub exec: Vec<HookConfig>,

    /// Patch lifecycle hooks.
    #[serde(default)]
    pub patch: Vec<HookConfig>,

    /// MCP tool lifecycle hooks.
    #[serde(default)]
    pub mcp: Vec<HookConfig>,

    /// Agent interaction hooks.
    #[serde(default)]
    pub agent: Vec<HookConfig>,

    /// Error handling hooks.
    #[serde(default)]
    pub error: Vec<HookConfig>,

    /// Custom integration hooks.
    #[serde(default)]
    pub integration: Vec<HookConfig>,
}

impl Default for GlobalHooksConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            timeout_seconds: default_timeout_seconds(),
            parallel_execution: default_parallel_execution(),
            session: Vec::new(),
            task: Vec::new(),
            exec: Vec::new(),
            patch: Vec::new(),
            mcp: Vec::new(),
            agent: Vec::new(),
            error: Vec::new(),
            integration: Vec::new(),
        }
    }
}

/// Configuration for a single hook.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HookConfig {
    /// The lifecycle event that triggers this hook.
    pub event: LifecycleEventType,

    /// The type and configuration of the hook.
    #[serde(flatten)]
    pub hook_type: HookType,

    /// Execution mode for this hook.
    #[serde(default)]
    pub mode: HookExecutionMode,

    /// Priority for hook execution ordering.
    #[serde(default)]
    pub priority: HookPriority,

    /// Optional condition for conditional execution.
    pub condition: Option<String>,

    /// Whether this hook should block execution if it fails.
    #[serde(default)]
    pub blocking: bool,

    /// Whether this hook execution is required for the operation to succeed.
    #[serde(default)]
    pub required: bool,

    /// Tags for hook categorization and filtering.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Human-readable description of the hook.
    pub description: Option<String>,
}

impl HookConfig {
    /// Validate the hook configuration.
    pub fn validate(&self) -> Result<(), HookError> {
        // Validate hook type specific configuration
        match &self.hook_type {
            HookType::Script { command, .. } => {
                if command.is_empty() {
                    return Err(HookError::Configuration(
                        "Script hook must have a non-empty command".to_string(),
                    ));
                }
            }
            HookType::Webhook { url, .. } => {
                if url.is_empty() {
                    return Err(HookError::Configuration(
                        "Webhook hook must have a non-empty URL".to_string(),
                    ));
                }
                // Basic URL validation
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Err(HookError::Configuration(
                        "Webhook URL must start with http:// or https://".to_string(),
                    ));
                }
            }
            HookType::McpTool { server, tool, .. } => {
                if server.is_empty() || tool.is_empty() {
                    return Err(HookError::Configuration(
                        "MCP tool hook must have non-empty server and tool names".to_string(),
                    ));
                }
            }
            HookType::Executable { path, .. } => {
                if !path.exists() {
                    return Err(HookError::Configuration(format!(
                        "Executable path does not exist: {}",
                        path.display()
                    )));
                }
            }
        }

        // Validate condition syntax if present
        if let Some(condition) = &self.condition {
            self.validate_condition(condition)?;
        }

        Ok(())
    }

    /// Validate condition syntax (basic validation for now).
    fn validate_condition(&self, condition: &str) -> Result<(), HookError> {
        // Basic validation - ensure condition is not empty
        if condition.trim().is_empty() {
            return Err(HookError::Configuration(
                "Hook condition cannot be empty".to_string(),
            ));
        }

        // TODO: Implement more sophisticated condition parsing and validation
        // For now, we just check for basic syntax

        Ok(())
    }

    /// Get the timeout for this hook, falling back to the provided default.
    pub fn get_timeout(&self, default_timeout: Duration) -> Duration {
        match &self.hook_type {
            HookType::Script { timeout, .. }
            | HookType::Webhook { timeout, .. }
            | HookType::McpTool { timeout, .. }
            | HookType::Executable { timeout, .. } => {
                timeout.unwrap_or(default_timeout)
            }
        }
    }
}

/// Load hooks configuration from a TOML file.
pub fn load_hooks_config(path: &PathBuf) -> Result<HooksConfig, HookError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| HookError::Configuration(format!("Failed to read hooks config: {}", e)))?;

    let config: HooksConfig = toml::from_str(&content)
        .map_err(|e| HookError::Configuration(format!("Failed to parse hooks config: {}", e)))?;

    // Validate all hook configurations
    validate_hooks_config(&config)?;

    Ok(config)
}

/// Validate the entire hooks configuration.
pub fn validate_hooks_config(config: &HooksConfig) -> Result<(), HookError> {
    let all_hooks = [
        &config.hooks.session,
        &config.hooks.task,
        &config.hooks.exec,
        &config.hooks.patch,
        &config.hooks.mcp,
        &config.hooks.agent,
        &config.hooks.error,
        &config.hooks.integration,
    ];

    for hook_group in all_hooks {
        for hook in hook_group {
            hook.validate()?;
        }
    }

    Ok(())
}

/// Get all hooks for a specific event type from the configuration.
pub fn get_hooks_for_event(
    config: &HooksConfig,
    event_type: LifecycleEventType,
) -> Vec<&HookConfig> {
    let all_hooks = [
        &config.hooks.session,
        &config.hooks.task,
        &config.hooks.exec,
        &config.hooks.patch,
        &config.hooks.mcp,
        &config.hooks.agent,
        &config.hooks.error,
        &config.hooks.integration,
    ];

    let mut matching_hooks = Vec::new();

    for hook_group in all_hooks {
        for hook in hook_group {
            if hook.event == event_type {
                matching_hooks.push(hook);
            }
        }
    }

    // Sort by priority (lower numbers = higher priority)
    matching_hooks.sort_by_key(|hook| hook.priority);

    matching_hooks
}

// Default value functions for serde
fn default_enabled() -> bool {
    true
}

fn default_timeout_seconds() -> u64 {
    30
}

fn default_parallel_execution() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_hooks_config_parsing() {
        let config_content = r#"
[hooks]
enabled = true
timeout_seconds = 60

[[hooks.task]]
event = "task_start"
type = "script"
command = ["echo", "Task started"]
environment = {}
mode = "async"
priority = 100
"#;

        let config: HooksConfig = toml::from_str(config_content).unwrap();
        assert!(config.hooks.enabled);
        assert_eq!(config.hooks.timeout_seconds, 60);
        assert_eq!(config.hooks.task.len(), 1);

        let hook = &config.hooks.task[0];
        assert_eq!(hook.event, LifecycleEventType::TaskStart);
        assert_eq!(hook.mode, HookExecutionMode::Async);
    }

    #[test]
    fn test_hook_validation() {
        let hook = HookConfig {
            event: LifecycleEventType::TaskStart,
            hook_type: HookType::Script {
                command: vec!["echo".to_string(), "test".to_string()],
                cwd: None,
                environment: HashMap::new(),
                timeout: None,
            },
            mode: HookExecutionMode::Async,
            priority: HookPriority::NORMAL,
            condition: None,
            blocking: false,
            required: false,
            tags: Vec::new(),
            description: None,
        };

        assert!(hook.validate().is_ok());
    }

    #[test]
    fn test_invalid_hook_validation() {
        let hook = HookConfig {
            event: LifecycleEventType::TaskStart,
            hook_type: HookType::Script {
                command: vec![], // Empty command should fail validation
                cwd: None,
                environment: HashMap::new(),
                timeout: None,
            },
            mode: HookExecutionMode::Async,
            priority: HookPriority::NORMAL,
            condition: None,
            blocking: false,
            required: false,
            tags: Vec::new(),
            description: None,
        };

        assert!(hook.validate().is_err());
    }
}
