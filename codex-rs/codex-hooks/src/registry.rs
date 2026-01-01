//! Hook registry for storing and retrieving hooks.
//!
//! Aligned with Claude Code's hook registry pattern, supporting:
//! - Global hooks from configuration
//! - Session-scoped hooks for programmatic registration
//! - Thread-safe access via DashMap

use std::sync::Arc;

use dashmap::DashMap;
use tracing::debug;

use crate::matcher::matches_pattern;
use crate::types::HookConfig;
use crate::types::HookEventType;
use crate::types::HookMatcher;

/// Thread-safe hook registry.
///
/// Stores hooks organized by event type with support for:
/// - Global hooks (from configuration, persistent)
/// - Session hooks (programmatic, cleared on session end)
#[derive(Debug, Default)]
pub struct HookRegistry {
    /// Global hooks by event type.
    global_hooks: DashMap<HookEventType, Vec<HookMatcher>>,

    /// Session-scoped hooks by session ID and event type.
    session_hooks: DashMap<String, DashMap<HookEventType, Vec<HookMatcher>>>,
}

impl HookRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register global hooks for an event type.
    pub fn register_global(&self, event_type: HookEventType, matchers: Vec<HookMatcher>) {
        debug!(event = %event_type, count = matchers.len(), "Registering global hooks");
        self.global_hooks
            .entry(event_type)
            .or_default()
            .extend(matchers);
    }

    /// Register a single global hook with a matcher pattern.
    pub fn register_global_hook(
        &self,
        event_type: HookEventType,
        matcher_pattern: &str,
        hook: HookConfig,
    ) {
        debug!(
            event = %event_type,
            matcher = matcher_pattern,
            "Registering single global hook"
        );
        self.global_hooks
            .entry(event_type)
            .or_default()
            .push(HookMatcher {
                matcher: matcher_pattern.to_string(),
                hooks: vec![hook],
            });
    }

    /// Register session-scoped hooks.
    ///
    /// Session hooks are cleared when `clear_session_hooks` is called.
    pub fn register_session_hook(
        &self,
        session_id: &str,
        event_type: HookEventType,
        matcher: HookMatcher,
    ) {
        debug!(
            session = session_id,
            event = %event_type,
            "Registering session hook"
        );
        self.session_hooks
            .entry(session_id.to_string())
            .or_default()
            .entry(event_type)
            .or_default()
            .push(matcher);
    }

    /// Clear all session-scoped hooks for a session.
    pub fn clear_session_hooks(&self, session_id: &str) {
        debug!(session = session_id, "Clearing session hooks");
        self.session_hooks.remove(session_id);
    }

    /// Get all matching hooks for an event.
    ///
    /// Returns hooks from both global and session-scoped registrations
    /// that match the given value.
    ///
    /// # Arguments
    ///
    /// * `event_type` - The type of hook event
    /// * `match_value` - The value to match against (tool_name, source, etc.)
    /// * `session_id` - Optional session ID for session-scoped hooks
    pub fn get_matching_hooks(
        &self,
        event_type: HookEventType,
        match_value: Option<&str>,
        session_id: Option<&str>,
    ) -> Vec<HookConfig> {
        let mut result = Vec::new();

        // Collect from global hooks
        if let Some(matchers) = self.global_hooks.get(&event_type) {
            for matcher in matchers.iter() {
                if self.matches(&matcher.matcher, match_value) {
                    result.extend(matcher.hooks.iter().cloned());
                }
            }
        }

        // Collect from session hooks
        if let Some(session_id) = session_id {
            if let Some(session_map) = self.session_hooks.get(session_id) {
                if let Some(matchers) = session_map.get(&event_type) {
                    for matcher in matchers.iter() {
                        if self.matches(&matcher.matcher, match_value) {
                            result.extend(matcher.hooks.iter().cloned());
                        }
                    }
                }
            }
        }

        result
    }

    /// Check if a pattern matches a value.
    ///
    /// For events without matchers (Stop, SubagentStop, UserPromptSubmit),
    /// all hooks run regardless of the matcher pattern.
    fn matches(&self, pattern: &str, match_value: Option<&str>) -> bool {
        match match_value {
            Some(value) => matches_pattern(pattern, value),
            // No match value means all hooks should run (e.g., Stop event)
            None => true,
        }
    }

    /// Get the number of global hooks for an event type.
    pub fn global_hook_count(&self, event_type: HookEventType) -> usize {
        self.global_hooks
            .get(&event_type)
            .map(|m| m.iter().map(|h| h.hooks.len()).sum())
            .unwrap_or(0)
    }

    /// Get the number of session hooks for a session and event type.
    pub fn session_hook_count(&self, session_id: &str, event_type: HookEventType) -> usize {
        self.session_hooks
            .get(session_id)
            .map(|session_map| {
                session_map
                    .get(&event_type)
                    .map(|m| m.iter().map(|h| h.hooks.len()).sum())
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }

    /// Check if any hooks are registered for an event type.
    pub fn has_hooks(&self, event_type: HookEventType, session_id: Option<&str>) -> bool {
        // Check global hooks
        if self
            .global_hooks
            .get(&event_type)
            .is_some_and(|m| !m.is_empty())
        {
            return true;
        }

        // Check session hooks
        if let Some(session_id) = session_id {
            if self
                .session_hooks
                .get(session_id)
                .is_some_and(|s| s.get(&event_type).is_some_and(|m| !m.is_empty()))
            {
                return true;
            }
        }

        false
    }

    /// Get deduplication keys for command hooks.
    ///
    /// Returns command strings for deduplication (callbacks are never deduplicated).
    pub fn get_dedupe_keys(&self, hooks: &[HookConfig]) -> Vec<Option<String>> {
        hooks
            .iter()
            .map(|h| match &h.hook_type {
                crate::types::HookType::Command { command, .. } => Some(command.clone()),
                crate::types::HookType::Callback { .. } => None,
            })
            .collect()
    }

    /// Deduplicate hooks by their keys.
    ///
    /// Command hooks with the same command string are deduplicated.
    /// Callback hooks are never deduplicated.
    pub fn deduplicate_hooks(&self, hooks: Vec<HookConfig>) -> Vec<HookConfig> {
        let mut seen_commands = std::collections::HashSet::new();
        let mut result = Vec::new();

        for hook in hooks {
            match &hook.hook_type {
                crate::types::HookType::Command { command, .. } => {
                    if seen_commands.insert(command.clone()) {
                        result.push(hook);
                    }
                }
                crate::types::HookType::Callback { .. } => {
                    // Callbacks are never deduplicated
                    result.push(hook);
                }
            }
        }

        result
    }
}

/// Build a hook registry from JSON configuration.
///
/// Returns (HookRegistry, Option<shell_prefix>).
pub fn build_from_json_config(
    config: &crate::config::HooksJsonConfig,
) -> (HookRegistry, Option<String>) {
    use crate::types::HookType;

    let registry = HookRegistry::new();

    if config.disable_all_hooks {
        debug!("Hooks are disabled via configuration");
        return (registry, config.shell_prefix.clone());
    }

    for (event_name, matchers) in &config.hooks {
        let event_type = match parse_event_type(event_name) {
            Some(e) => e,
            None => {
                debug!(event = %event_name, "Unknown hook event type, skipping");
                continue;
            }
        };

        for matcher_json in matchers {
            let hooks: Vec<HookConfig> = matcher_json
                .hooks
                .iter()
                .filter_map(|hook_json| {
                    if hook_json.hook_type != "command" {
                        debug!(
                            hook_type = %hook_json.hook_type,
                            "Unsupported hook type, skipping"
                        );
                        return None;
                    }

                    let command = hook_json.command.as_ref()?.clone();
                    Some(HookConfig {
                        hook_type: HookType::Command {
                            command,
                            timeout_secs: hook_json.timeout as u32,
                            status_message: hook_json.status_message.clone(),
                        },
                        on_success: None,
                    })
                })
                .collect();

            if !hooks.is_empty() {
                registry.register_global(
                    event_type,
                    vec![HookMatcher {
                        matcher: matcher_json.matcher.clone(),
                        hooks,
                    }],
                );
            }
        }
    }

    (registry, config.shell_prefix.clone())
}

/// Parse event type from string name.
fn parse_event_type(name: &str) -> Option<HookEventType> {
    match name {
        "PreToolUse" => Some(HookEventType::PreToolUse),
        "PostToolUse" => Some(HookEventType::PostToolUse),
        "PostToolUseFailure" => Some(HookEventType::PostToolUseFailure),
        "SessionStart" => Some(HookEventType::SessionStart),
        "SessionEnd" => Some(HookEventType::SessionEnd),
        "Stop" => Some(HookEventType::Stop),
        "SubagentStart" => Some(HookEventType::SubagentStart),
        "SubagentStop" => Some(HookEventType::SubagentStop),
        "UserPromptSubmit" => Some(HookEventType::UserPromptSubmit),
        "Notification" => Some(HookEventType::Notification),
        "PreCompact" => Some(HookEventType::PreCompact),
        "PermissionRequest" => Some(HookEventType::PermissionRequest),
        _ => None,
    }
}

/// Builder for creating hook registries from configuration.
#[derive(Debug, Default)]
pub struct HookRegistryBuilder {
    registry: HookRegistry,
}

impl HookRegistryBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add global hooks for an event type.
    pub fn with_global_hooks(self, event_type: HookEventType, matchers: Vec<HookMatcher>) -> Self {
        self.registry.register_global(event_type, matchers);
        self
    }

    /// Build the registry.
    pub fn build(self) -> HookRegistry {
        self.registry
    }

    /// Build and wrap in Arc for shared ownership.
    pub fn build_arc(self) -> Arc<HookRegistry> {
        Arc::new(self.build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::HookType;

    fn make_command_hook(command: &str) -> HookConfig {
        HookConfig {
            hook_type: HookType::Command {
                command: command.to_string(),
                timeout_secs: 60,
                status_message: None,
            },
            on_success: None,
        }
    }

    #[test]
    fn test_register_global_hooks() {
        let registry = HookRegistry::new();
        let hook = make_command_hook("echo test");

        registry.register_global_hook(HookEventType::PreToolUse, "Bash", hook);

        assert_eq!(registry.global_hook_count(HookEventType::PreToolUse), 1);
        assert_eq!(registry.global_hook_count(HookEventType::PostToolUse), 0);
    }

    #[test]
    fn test_get_matching_hooks_exact() {
        let registry = HookRegistry::new();
        registry.register_global_hook(
            HookEventType::PreToolUse,
            "Bash",
            make_command_hook("echo bash"),
        );
        registry.register_global_hook(
            HookEventType::PreToolUse,
            "Write",
            make_command_hook("echo write"),
        );

        let hooks = registry.get_matching_hooks(HookEventType::PreToolUse, Some("Bash"), None);
        assert_eq!(hooks.len(), 1);

        let hooks = registry.get_matching_hooks(HookEventType::PreToolUse, Some("Write"), None);
        assert_eq!(hooks.len(), 1);

        let hooks = registry.get_matching_hooks(HookEventType::PreToolUse, Some("Read"), None);
        assert!(hooks.is_empty());
    }

    #[test]
    fn test_get_matching_hooks_wildcard() {
        let registry = HookRegistry::new();
        registry.register_global_hook(
            HookEventType::SessionStart,
            "*",
            make_command_hook("echo session"),
        );

        let hooks = registry.get_matching_hooks(HookEventType::SessionStart, Some("cli"), None);
        assert_eq!(hooks.len(), 1);

        let hooks =
            registry.get_matching_hooks(HookEventType::SessionStart, Some("anything"), None);
        assert_eq!(hooks.len(), 1);
    }

    #[test]
    fn test_get_matching_hooks_pipe_separated() {
        let registry = HookRegistry::new();
        registry.register_global_hook(
            HookEventType::PreToolUse,
            "Bash|Write|Edit",
            make_command_hook("echo tools"),
        );

        let hooks = registry.get_matching_hooks(HookEventType::PreToolUse, Some("Bash"), None);
        assert_eq!(hooks.len(), 1);

        let hooks = registry.get_matching_hooks(HookEventType::PreToolUse, Some("Edit"), None);
        assert_eq!(hooks.len(), 1);

        let hooks = registry.get_matching_hooks(HookEventType::PreToolUse, Some("Read"), None);
        assert!(hooks.is_empty());
    }

    #[test]
    fn test_session_hooks() {
        let registry = HookRegistry::new();
        let session_id = "session-123";

        registry.register_session_hook(
            session_id,
            HookEventType::PreToolUse,
            HookMatcher {
                matcher: "Bash".to_string(),
                hooks: vec![make_command_hook("echo session")],
            },
        );

        // Should find with correct session
        let hooks =
            registry.get_matching_hooks(HookEventType::PreToolUse, Some("Bash"), Some(session_id));
        assert_eq!(hooks.len(), 1);

        // Should not find with wrong session
        let hooks =
            registry.get_matching_hooks(HookEventType::PreToolUse, Some("Bash"), Some("other"));
        assert!(hooks.is_empty());

        // Should not find without session
        let hooks = registry.get_matching_hooks(HookEventType::PreToolUse, Some("Bash"), None);
        assert!(hooks.is_empty());

        // Clear session hooks
        registry.clear_session_hooks(session_id);
        let hooks =
            registry.get_matching_hooks(HookEventType::PreToolUse, Some("Bash"), Some(session_id));
        assert!(hooks.is_empty());
    }

    #[test]
    fn test_no_matcher_events() {
        let registry = HookRegistry::new();
        registry.register_global_hook(
            HookEventType::Stop,
            "anything",
            make_command_hook("echo stop"),
        );

        // Stop events have no matcher - all hooks run
        let hooks = registry.get_matching_hooks(HookEventType::Stop, None, None);
        assert_eq!(hooks.len(), 1);
    }

    #[test]
    fn test_deduplicate_hooks() {
        let registry = HookRegistry::new();
        let hooks = vec![
            make_command_hook("echo test"),
            make_command_hook("echo test"), // Duplicate
            make_command_hook("echo other"),
        ];

        let deduped = registry.deduplicate_hooks(hooks);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_has_hooks() {
        let registry = HookRegistry::new();
        assert!(!registry.has_hooks(HookEventType::PreToolUse, None));

        registry.register_global_hook(
            HookEventType::PreToolUse,
            "Bash",
            make_command_hook("echo test"),
        );
        assert!(registry.has_hooks(HookEventType::PreToolUse, None));
    }

    #[test]
    fn test_builder() {
        let registry = HookRegistryBuilder::new()
            .with_global_hooks(
                HookEventType::SessionStart,
                vec![HookMatcher {
                    matcher: "*".to_string(),
                    hooks: vec![make_command_hook("echo init")],
                }],
            )
            .build();

        assert_eq!(registry.global_hook_count(HookEventType::SessionStart), 1);
    }

    #[test]
    fn test_build_from_json_config() {
        use crate::config::HookConfigJson;
        use crate::config::HookMatcherJson;
        use crate::config::HooksJsonConfig;
        use std::collections::HashMap;

        let mut hooks = HashMap::new();
        hooks.insert(
            "PreToolUse".to_string(),
            vec![HookMatcherJson {
                matcher: "Bash|Write".to_string(),
                hooks: vec![
                    HookConfigJson {
                        hook_type: "command".to_string(),
                        command: Some("echo cmd1".to_string()),
                        timeout: 30,
                        status_message: None,
                    },
                    HookConfigJson {
                        hook_type: "command".to_string(),
                        command: Some("echo cmd2".to_string()),
                        timeout: 60,
                        status_message: Some("Testing...".to_string()),
                    },
                ],
            }],
        );

        let config = HooksJsonConfig {
            disable_all_hooks: false,
            shell_prefix: Some("/prefix.sh".to_string()),
            hooks,
        };

        let (registry, shell_prefix) = build_from_json_config(&config);

        assert_eq!(registry.global_hook_count(HookEventType::PreToolUse), 2);
        assert_eq!(shell_prefix, Some("/prefix.sh".to_string()));

        // Test matching
        let matching = registry.get_matching_hooks(HookEventType::PreToolUse, Some("Bash"), None);
        assert_eq!(matching.len(), 2);

        let matching = registry.get_matching_hooks(HookEventType::PreToolUse, Some("Read"), None);
        assert!(matching.is_empty());
    }

    #[test]
    fn test_build_from_json_config_disabled() {
        use crate::config::HooksJsonConfig;
        use std::collections::HashMap;

        let config = HooksJsonConfig {
            disable_all_hooks: true,
            shell_prefix: None,
            hooks: HashMap::new(),
        };

        let (registry, _) = build_from_json_config(&config);
        assert!(!registry.has_hooks(HookEventType::PreToolUse, None));
    }
}
