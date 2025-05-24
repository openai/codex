//! Hook registry for managing hook definitions and routing.

use std::collections::HashMap;

use crate::hooks::config::{HooksConfig, HookConfig};
use crate::hooks::context::HookContext;
use crate::hooks::types::{HookError, LifecycleEvent, LifecycleEventType, HookPriority};

/// Registry for managing hook definitions and event routing.
pub struct HookRegistry {
    hooks_by_event: HashMap<LifecycleEventType, Vec<HookConfig>>,
    config: HooksConfig,
}

impl HookRegistry {
    /// Create a new hook registry with the given configuration.
    pub async fn new(config: HooksConfig) -> Result<Self, HookError> {
        let mut registry = Self {
            hooks_by_event: HashMap::new(),
            config: config.clone(),
        };

        // Populate hooks from configuration
        registry.load_hooks_from_config(&config)?;

        Ok(registry)
    }

    /// Load hooks from the configuration into the registry.
    fn load_hooks_from_config(&mut self, config: &HooksConfig) -> Result<(), HookError> {
        // Load hooks from all categories
        let all_hook_groups = [
            (&config.hooks.session, "session"),
            (&config.hooks.task, "task"),
            (&config.hooks.exec, "exec"),
            (&config.hooks.patch, "patch"),
            (&config.hooks.mcp, "mcp"),
            (&config.hooks.agent, "agent"),
            (&config.hooks.error, "error"),
            (&config.hooks.integration, "integration"),
        ];

        for (hook_group, category) in all_hook_groups {
            for hook in hook_group {
                self.register_hook_internal(hook.clone(), category)?;
            }
        }

        // Sort hooks by priority within each event type
        self.sort_hooks_by_priority();

        Ok(())
    }

    /// Internal method to register a hook with category tracking.
    fn register_hook_internal(&mut self, hook: HookConfig, category: &str) -> Result<(), HookError> {
        // Validate the hook configuration
        hook.validate().map_err(|e| {
            HookError::Registry(format!("Invalid hook in {} category: {}", category, e))
        })?;

        // Add the hook to the appropriate event type
        self.hooks_by_event
            .entry(hook.event)
            .or_insert_with(Vec::new)
            .push(hook);

        Ok(())
    }

    /// Sort hooks by priority within each event type.
    fn sort_hooks_by_priority(&mut self) {
        for hooks in self.hooks_by_event.values_mut() {
            hooks.sort_by_key(|hook| hook.priority);
        }
    }

    /// Get all hooks for a specific event type, sorted by priority.
    pub fn get_hooks_for_event(&self, event_type: LifecycleEventType) -> Vec<&HookConfig> {
        self.hooks_by_event
            .get(&event_type)
            .map(|hooks| hooks.iter().collect())
            .unwrap_or_default()
    }

    /// Get hooks for a specific event type that match the given condition.
    pub fn get_matching_hooks(
        &self,
        event: &LifecycleEvent,
        context: &HookContext,
    ) -> Result<Vec<&HookConfig>, HookError> {
        let event_type = event.event_type();
        let all_hooks = self.get_hooks_for_event(event_type);

        let mut matching_hooks = Vec::new();

        for hook in all_hooks {
            if self.evaluate_hook_condition(hook, event, context)? {
                matching_hooks.push(hook);
            }
        }

        Ok(matching_hooks)
    }

    /// Evaluate whether a hook's condition is met for the given event and context.
    fn evaluate_hook_condition(
        &self,
        hook: &HookConfig,
        event: &LifecycleEvent,
        context: &HookContext,
    ) -> Result<bool, HookError> {
        // If no condition is specified, the hook always matches
        let Some(condition) = &hook.condition else {
            return Ok(true);
        };

        // Evaluate the condition
        self.evaluate_condition_expression(condition, event, context)
    }

    /// Evaluate a condition expression.
    /// This is a basic implementation - in the future this could be extended
    /// with a proper expression parser.
    fn evaluate_condition_expression(
        &self,
        condition: &str,
        event: &LifecycleEvent,
        context: &HookContext,
    ) -> Result<bool, HookError> {
        // Basic condition evaluation
        // For now, we support simple conditions like:
        // - "success == true"
        // - "exit_code == 0"
        // - "message.contains('ERROR')"
        // - "task_id == 'specific_task'"

        let condition = condition.trim();

        // Handle boolean conditions
        if condition == "true" {
            return Ok(true);
        }
        if condition == "false" {
            return Ok(false);
        }

        // Handle equality conditions
        if let Some((left, right)) = condition.split_once("==") {
            let left = left.trim();
            let right = right.trim().trim_matches('"').trim_matches('\'');

            return self.evaluate_equality_condition(left, right, event, context);
        }

        // Handle contains conditions
        if condition.contains(".contains(") {
            return self.evaluate_contains_condition(condition, event, context);
        }

        // Handle not equals conditions
        if let Some((left, right)) = condition.split_once("!=") {
            let left = left.trim();
            let right = right.trim().trim_matches('"').trim_matches('\'');

            return Ok(!self.evaluate_equality_condition(left, right, event, context)?);
        }

        // If we can't parse the condition, log a warning and return true
        tracing::warn!("Unknown condition format: '{}', defaulting to true", condition);
        Ok(true)
    }

    /// Evaluate an equality condition.
    fn evaluate_equality_condition(
        &self,
        left: &str,
        right: &str,
        event: &LifecycleEvent,
        context: &HookContext,
    ) -> Result<bool, HookError> {
        let left_value = self.get_condition_value(left, event, context)?;
        Ok(left_value == right)
    }

    /// Evaluate a contains condition.
    fn evaluate_contains_condition(
        &self,
        condition: &str,
        event: &LifecycleEvent,
        context: &HookContext,
    ) -> Result<bool, HookError> {
        // Parse "field.contains('value')" format
        if let Some(start) = condition.find(".contains(") {
            let field = condition[..start].trim();
            let rest = &condition[start + 10..]; // Skip ".contains("

            if let Some(end) = rest.find(')') {
                let value = rest[..end].trim().trim_matches('"').trim_matches('\'');
                let field_value = self.get_condition_value(field, event, context)?;
                return Ok(field_value.contains(value));
            }
        }

        Err(HookError::Registry(format!(
            "Invalid contains condition format: '{}'",
            condition
        )))
    }

    /// Get the value of a field for condition evaluation.
    fn get_condition_value(
        &self,
        field: &str,
        event: &LifecycleEvent,
        context: &HookContext,
    ) -> Result<String, HookError> {
        match field {
            "success" => match event {
                LifecycleEvent::TaskComplete { success, .. }
                | LifecycleEvent::PatchAfter { success, .. }
                | LifecycleEvent::McpToolAfter { success, .. } => Ok(success.to_string()),
                LifecycleEvent::ExecAfter { exit_code, .. } => Ok((exit_code == &0).to_string()),
                _ => Ok("false".to_string()),
            },
            "exit_code" => match event {
                LifecycleEvent::ExecAfter { exit_code, .. } => Ok(exit_code.to_string()),
                _ => Ok("0".to_string()),
            },
            "message" => match event {
                LifecycleEvent::AgentMessage { message, .. } => Ok(message.clone()),
                _ => Ok(String::new()),
            },
            "task_id" => Ok(event.task_id().unwrap_or("").to_string()),
            "session_id" => match event {
                LifecycleEvent::SessionStart { session_id, .. }
                | LifecycleEvent::SessionEnd { session_id, .. }
                | LifecycleEvent::TaskStart { session_id, .. }
                | LifecycleEvent::TaskComplete { session_id, .. } => Ok(session_id.clone()),
                _ => Ok(String::new()),
            },
            "model" => match event {
                LifecycleEvent::SessionStart { model, .. } => Ok(model.clone()),
                _ => Ok(String::new()),
            },
            "server" => match event {
                LifecycleEvent::McpToolBefore { server, .. }
                | LifecycleEvent::McpToolAfter { server, .. } => Ok(server.clone()),
                _ => Ok(String::new()),
            },
            "tool" => match event {
                LifecycleEvent::McpToolBefore { tool, .. }
                | LifecycleEvent::McpToolAfter { tool, .. } => Ok(tool.clone()),
                _ => Ok(String::new()),
            },
            // Check environment variables
            field if field.starts_with("env.") => {
                let env_var = &field[4..]; // Remove "env." prefix
                Ok(context.get_env(env_var).cloned().unwrap_or_default())
            },
            _ => {
                tracing::warn!("Unknown condition field: '{}'", field);
                Ok(String::new())
            }
        }
    }

    /// Register a new hook at runtime.
    pub fn register_hook(&mut self, hook: HookConfig) -> Result<(), HookError> {
        self.register_hook_internal(hook, "runtime")?;
        self.sort_hooks_by_priority();
        Ok(())
    }

    /// Remove hooks matching a predicate.
    pub fn remove_hooks<F>(&mut self, predicate: F) -> usize
    where
        F: Fn(&HookConfig) -> bool,
    {
        let mut removed_count = 0;

        for hooks in self.hooks_by_event.values_mut() {
            let original_len = hooks.len();
            hooks.retain(|hook| !predicate(hook));
            removed_count += original_len - hooks.len();
        }

        removed_count
    }

    /// Get hooks by tag.
    pub fn get_hooks_by_tag(&self, tag: &str) -> Vec<&HookConfig> {
        let mut matching_hooks = Vec::new();

        for hooks in self.hooks_by_event.values() {
            for hook in hooks {
                if hook.tags.contains(&tag.to_string()) {
                    matching_hooks.push(hook);
                }
            }
        }

        // Sort by priority
        matching_hooks.sort_by_key(|hook| hook.priority);
        matching_hooks
    }

    /// Get hooks by priority range.
    pub fn get_hooks_by_priority_range(
        &self,
        min_priority: HookPriority,
        max_priority: HookPriority,
    ) -> Vec<&HookConfig> {
        let mut matching_hooks = Vec::new();

        for hooks in self.hooks_by_event.values() {
            for hook in hooks {
                if hook.priority >= min_priority && hook.priority <= max_priority {
                    matching_hooks.push(hook);
                }
            }
        }

        // Sort by priority
        matching_hooks.sort_by_key(|hook| hook.priority);
        matching_hooks
    }

    /// Get statistics about registered hooks.
    pub fn get_statistics(&self) -> HookRegistryStatistics {
        let mut total_hooks = 0;
        let mut hooks_by_event_count = HashMap::new();
        let mut hooks_by_priority = HashMap::new();

        for (event_type, hooks) in &self.hooks_by_event {
            let count = hooks.len();
            total_hooks += count;
            hooks_by_event_count.insert(*event_type, count);

            for hook in hooks {
                *hooks_by_priority.entry(hook.priority).or_insert(0) += 1;
            }
        }

        HookRegistryStatistics {
            total_hooks,
            hooks_by_event: hooks_by_event_count,
            hooks_by_priority,
            enabled: self.config.hooks.enabled,
        }
    }

    /// Check if the registry is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.hooks.enabled
    }
}

/// Statistics about the hook registry.
#[derive(Debug, Clone)]
pub struct HookRegistryStatistics {
    pub total_hooks: usize,
    pub hooks_by_event: HashMap<LifecycleEventType, usize>,
    pub hooks_by_priority: HashMap<HookPriority, usize>,
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::config::{GlobalHooksConfig, HooksConfig};
    use crate::hooks::types::{HookType, HookExecutionMode};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn create_test_hook(event: LifecycleEventType, priority: HookPriority) -> HookConfig {
        HookConfig {
            event,
            hook_type: HookType::Script {
                command: vec!["echo".to_string(), "test".to_string()],
                cwd: None,
                environment: HashMap::new(),
                timeout: None,
            },
            mode: HookExecutionMode::Async,
            priority,
            condition: None,
            blocking: false,
            required: false,
            tags: Vec::new(),
            description: None,
        }
    }

    #[tokio::test]
    async fn test_hook_registry_creation() {
        let config = HooksConfig {
            hooks: GlobalHooksConfig {
                enabled: true,
                timeout_seconds: 30,
                parallel_execution: true,
                task: vec![create_test_hook(LifecycleEventType::TaskStart, HookPriority::NORMAL)],
                ..Default::default()
            },
        };

        let registry = HookRegistry::new(config).await.unwrap();
        let hooks = registry.get_hooks_for_event(LifecycleEventType::TaskStart);
        assert_eq!(hooks.len(), 1);
    }

    #[tokio::test]
    async fn test_hook_priority_sorting() {
        let config = HooksConfig {
            hooks: GlobalHooksConfig {
                enabled: true,
                timeout_seconds: 30,
                parallel_execution: true,
                task: vec![
                    create_test_hook(LifecycleEventType::TaskStart, HookPriority::LOW),
                    create_test_hook(LifecycleEventType::TaskStart, HookPriority::HIGH),
                    create_test_hook(LifecycleEventType::TaskStart, HookPriority::NORMAL),
                ],
                ..Default::default()
            },
        };

        let registry = HookRegistry::new(config).await.unwrap();
        let hooks = registry.get_hooks_for_event(LifecycleEventType::TaskStart);

        // Should be sorted by priority (HIGH < NORMAL < LOW)
        assert_eq!(hooks[0].priority, HookPriority::HIGH);
        assert_eq!(hooks[1].priority, HookPriority::NORMAL);
        assert_eq!(hooks[2].priority, HookPriority::LOW);
    }

    #[tokio::test]
    async fn test_condition_evaluation() {
        let config = HooksConfig::default();
        let registry = HookRegistry::new(config).await.unwrap();

        let event = LifecycleEvent::TaskComplete {
            task_id: "test_task".to_string(),
            session_id: "test_session".to_string(),
            success: true,
            output: None,
            duration: std::time::Duration::from_secs(1),
            timestamp: chrono::Utc::now(),
        };

        let context = HookContext::new(event.clone(), PathBuf::from("/tmp"));

        // Test simple boolean condition
        assert!(registry.evaluate_condition_expression("true", &event, &context).unwrap());
        assert!(!registry.evaluate_condition_expression("false", &event, &context).unwrap());

        // Test equality condition
        assert!(registry.evaluate_condition_expression("success == true", &event, &context).unwrap());
        assert!(!registry.evaluate_condition_expression("success == false", &event, &context).unwrap());

        // Test task_id condition
        assert!(registry.evaluate_condition_expression("task_id == test_task", &event, &context).unwrap());
        assert!(!registry.evaluate_condition_expression("task_id == other_task", &event, &context).unwrap());
    }
}
