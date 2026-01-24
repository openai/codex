//! Hook executor - orchestrates hook execution and aggregates results.
//!
//! Aligned with Claude Code's hook execution patterns:
//! - Parallel execution of hooks within an event
//! - "Deny wins" permission aggregation
//! - Early termination on blocking errors
//! - System message collection
//! - Input modification propagation

use std::path::Path;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::error::HookError;
use crate::executors::callback::execute_callback;
use crate::executors::command::CommandConfig;
use crate::executors::command::execute_command;
use crate::input::HookContext;
use crate::input::HookEventData;
use crate::input::HookInput;
use crate::output::BlockingError;
use crate::output::HookOutcome;
use crate::output::HookResult;
use crate::output::PermissionDecision;
use crate::registry::HookRegistry;
use crate::types::HookConfig;
use crate::types::HookEventType;
use crate::types::HookType;

/// Hook executor that runs hooks and aggregates results.
#[derive(Debug)]
pub struct HookExecutor {
    registry: Arc<HookRegistry>,
    /// Shell prefix from config (takes precedence over env var).
    shell_prefix: Option<String>,
}

impl HookExecutor {
    /// Create a new executor with the given registry.
    pub fn new(registry: Arc<HookRegistry>) -> Self {
        Self {
            registry,
            shell_prefix: None,
        }
    }

    /// Create a new executor with the given registry and shell prefix.
    pub fn with_shell_prefix(registry: Arc<HookRegistry>, shell_prefix: Option<String>) -> Self {
        Self {
            registry,
            shell_prefix,
        }
    }

    /// Get a reference to the underlying registry.
    pub fn registry(&self) -> &HookRegistry {
        &self.registry
    }

    /// Run all matching hooks for an event.
    ///
    /// # Arguments
    ///
    /// * `event_type` - The type of hook event
    /// * `event_data` - Event-specific data
    /// * `context` - Base context for all hooks
    /// * `session_id` - Optional session ID for session hooks
    /// * `cwd` - Working directory for command hooks
    /// * `cancel` - Cancellation token
    pub async fn run_hooks(
        &self,
        event_type: HookEventType,
        event_data: HookEventData,
        context: HookContext,
        session_id: Option<&str>,
        cwd: &Path,
        cancel: CancellationToken,
    ) -> HookExecutionResult {
        // Get match value from event data
        let match_value = event_data.match_value();

        // Get matching hooks
        let hooks = self
            .registry
            .get_matching_hooks(event_type, match_value, session_id);

        if hooks.is_empty() {
            debug!(event = %event_type, "No matching hooks found");
            return HookExecutionResult::empty();
        }

        // Deduplicate hooks
        let hooks = self.registry.deduplicate_hooks(hooks);
        let hook_count = hooks.len();

        info!(
            event = %event_type,
            count = hook_count,
            "Running hooks"
        );

        // Build input
        let input = HookInput::new(event_type, context, event_data);

        // Execute hooks
        let mut results = Vec::with_capacity(hook_count);
        let tool_use_id = extract_tool_use_id(&input);

        for (index, hook) in hooks.iter().enumerate() {
            let hook_index = index as i32;

            let result = self
                .execute_hook(
                    hook,
                    &input,
                    event_type,
                    tool_use_id.clone(),
                    cwd,
                    cancel.clone(),
                    hook_index,
                )
                .await;

            // Call success callback if configured
            if let Ok(ref hook_result) = result {
                if hook_result.outcome == HookOutcome::Success {
                    if let Some(ref on_success) = hook.on_success {
                        on_success.on_success(hook, hook_result);
                    }
                }
            }

            match result {
                Ok(hook_result) => {
                    // Check for blocking error - stop execution
                    if hook_result.outcome == HookOutcome::Blocking {
                        warn!(
                            event = %event_type,
                            hook_index,
                            "Hook returned blocking error, stopping execution"
                        );
                        results.push(hook_result);
                        break;
                    }
                    results.push(hook_result);
                }
                Err(e) => {
                    warn!(
                        event = %event_type,
                        hook_index,
                        error = %e,
                        "Hook execution failed"
                    );
                    // Non-blocking errors don't stop execution
                    if e.is_blocking() {
                        results.push(HookResult::blocking(e.to_string(), get_hook_name(hook)));
                        break;
                    }
                }
            }
        }

        // Aggregate results
        aggregate_results(results, hook_count)
    }

    /// Execute a single hook.
    async fn execute_hook(
        &self,
        hook: &HookConfig,
        input: &HookInput,
        event_type: HookEventType,
        tool_use_id: Option<String>,
        cwd: &Path,
        cancel: CancellationToken,
        hook_index: i32,
    ) -> Result<HookResult, HookError> {
        match &hook.hook_type {
            HookType::Command {
                command,
                timeout_secs,
                status_message,
            } => {
                let config = CommandConfig {
                    command: command.clone(),
                    timeout_secs: *timeout_secs,
                    status_message: status_message.clone(),
                };
                let cmd_result = execute_command(
                    &config,
                    input,
                    event_type,
                    cwd,
                    cancel,
                    hook_index,
                    self.shell_prefix.as_deref(),
                )
                .await?;

                let mut result: HookResult = cmd_result.into();

                // Fill in command name for blocking errors
                if let Some(ref mut blocking_error) = result.blocking_error {
                    blocking_error.command = command.clone();
                }

                Ok(result)
            }
            HookType::Callback {
                callback,
                timeout_ms,
            } => {
                execute_callback(
                    callback.as_ref(),
                    input.clone(),
                    tool_use_id,
                    cancel,
                    hook_index,
                    *timeout_ms,
                )
                .await
            }
        }
    }

    /// Check if any hooks are registered for an event.
    pub fn has_hooks(&self, event_type: HookEventType, session_id: Option<&str>) -> bool {
        self.registry.has_hooks(event_type, session_id)
    }
}

/// Aggregated result from hook execution.
#[derive(Debug, Clone)]
pub struct HookExecutionResult {
    /// Number of hooks that were executed.
    pub hooks_executed: i32,

    /// Individual outcomes from each hook.
    pub outcomes: Vec<HookOutcome>,

    /// Aggregated permission decision ("deny wins").
    pub permission: PermissionDecision,

    /// Collected system messages from all hooks.
    pub system_messages: Vec<String>,

    /// Updated input from hooks (last non-None value).
    pub updated_input: Option<serde_json::Value>,

    /// Additional context from hooks (concatenated).
    pub additional_context: Option<String>,

    /// Blocking error if execution was stopped.
    pub blocking_error: Option<BlockingError>,

    /// Whether execution should continue.
    pub should_continue: bool,

    /// Stop reason if execution should not continue.
    pub stop_reason: Option<String>,
}

impl HookExecutionResult {
    /// Create an empty result (no hooks executed).
    pub fn empty() -> Self {
        Self {
            hooks_executed: 0,
            outcomes: Vec::new(),
            permission: PermissionDecision::Ask,
            system_messages: Vec::new(),
            updated_input: None,
            additional_context: None,
            blocking_error: None,
            should_continue: true,
            stop_reason: None,
        }
    }

    /// Check if any hook returned a blocking error.
    pub fn is_blocking(&self) -> bool {
        self.blocking_error.is_some()
            || self.permission == PermissionDecision::Deny
            || !self.should_continue
    }

    /// Check if permission was granted.
    pub fn is_allowed(&self) -> bool {
        self.permission == PermissionDecision::Allow
    }

    /// Check if user should be prompted for permission.
    pub fn should_ask(&self) -> bool {
        self.permission == PermissionDecision::Ask && self.should_continue
    }

    /// Get combined additional context.
    pub fn combined_context(&self) -> Option<String> {
        let mut parts = Vec::new();

        if let Some(ref ctx) = self.additional_context {
            parts.push(ctx.clone());
        }

        for msg in &self.system_messages {
            parts.push(msg.clone());
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n\n"))
        }
    }
}

/// Aggregate results from multiple hooks.
fn aggregate_results(results: Vec<HookResult>, total_hooks: usize) -> HookExecutionResult {
    let mut outcomes = Vec::with_capacity(results.len());
    let mut permission = PermissionDecision::Ask;
    let mut system_messages = Vec::new();
    let mut updated_input = None;
    let mut additional_contexts = Vec::new();
    let mut blocking_error = None;
    let mut should_continue = true;
    let mut stop_reason = None;

    for result in results {
        outcomes.push(result.outcome);

        // Capture blocking error
        if result.outcome == HookOutcome::Blocking {
            if let Some(error) = result.blocking_error {
                blocking_error = Some(error);
            }
        }

        if let Some(output) = result.output {
            // "Deny wins" permission aggregation
            if let Some(decision) = output.permission_decision() {
                permission = match (permission, decision) {
                    (_, PermissionDecision::Deny) => PermissionDecision::Deny,
                    (PermissionDecision::Deny, _) => PermissionDecision::Deny,
                    (PermissionDecision::Ask, PermissionDecision::Allow) => {
                        PermissionDecision::Allow
                    }
                    (PermissionDecision::Allow, _) => PermissionDecision::Allow,
                    (PermissionDecision::Ask, PermissionDecision::Ask) => PermissionDecision::Ask,
                };
            }

            // Take last updated input
            if let Some(input) = output.updated_input() {
                updated_input = Some(input.clone());
            }

            // Collect additional context
            if let Some(ctx) = output.additional_context() {
                additional_contexts.push(ctx.to_string());
            }

            // Check continue flag
            if output.r#continue == Some(false) {
                should_continue = false;
                if let Some(ref reason) = output.stop_reason {
                    stop_reason = Some(reason.clone());
                }
            }

            // Collect system messages (after other borrows)
            if let Some(msg) = output.system_message {
                system_messages.push(msg);
            }
        }
    }

    // Combine additional contexts
    let additional_context = if additional_contexts.is_empty() {
        None
    } else {
        Some(additional_contexts.join("\n\n"))
    };

    HookExecutionResult {
        hooks_executed: total_hooks as i32,
        outcomes,
        permission,
        system_messages,
        updated_input,
        additional_context,
        blocking_error,
        should_continue,
        stop_reason,
    }
}

/// Extract tool_use_id from hook input if present.
fn extract_tool_use_id(input: &HookInput) -> Option<String> {
    match &input.event_data {
        HookEventData::ToolEvent { tool_use_id, .. } => Some(tool_use_id.clone()),
        _ => None,
    }
}

/// Get a display name for a hook.
fn get_hook_name(hook: &HookConfig) -> String {
    match &hook.hook_type {
        HookType::Command { command, .. } => {
            // Truncate long commands
            if command.len() > 50 {
                format!("{}...", &command[..47])
            } else {
                command.clone()
            }
        }
        HookType::Callback { callback, .. } => format!("{callback:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executors::callback::BlockingCallback;
    use crate::executors::callback::NoOpCallback;
    use crate::executors::callback::SystemMessageCallback;
    use crate::output::HookOutput;
    use crate::registry::HookRegistryBuilder;
    use crate::types::HookMatcher;
    use std::path::PathBuf;

    fn make_context() -> HookContext {
        HookContext {
            session_id: "test-session".to_string(),
            transcript_path: "/tmp/transcript.json".to_string(),
            cwd: "/tmp".to_string(),
            permission_mode: serde_json::Value::Null,
        }
    }

    fn make_command_hook(command: &str) -> HookConfig {
        HookConfig {
            hook_type: HookType::Command {
                command: command.to_string(),
                timeout_secs: 5,
                status_message: None,
            },
            on_success: None,
        }
    }

    fn make_callback_hook<C: crate::types::HookCallback + 'static>(callback: C) -> HookConfig {
        HookConfig {
            hook_type: HookType::Callback {
                callback: Arc::new(callback),
                timeout_ms: Some(5000),
            },
            on_success: None,
        }
    }

    #[tokio::test]
    async fn test_no_hooks() {
        let registry = HookRegistryBuilder::new().build_arc();
        let executor = HookExecutor::new(registry);

        let result = executor
            .run_hooks(
                HookEventType::PreToolUse,
                HookEventData::pre_tool_use(
                    "Bash".to_string(),
                    serde_json::json!({}),
                    "tool-1".to_string(),
                ),
                make_context(),
                None,
                &PathBuf::from("/tmp"),
                CancellationToken::new(),
            )
            .await;

        assert_eq!(result.hooks_executed, 0);
        assert!(!result.is_blocking());
    }

    #[tokio::test]
    async fn test_single_command_hook() {
        let registry = HookRegistryBuilder::new()
            .with_global_hooks(
                HookEventType::PreToolUse,
                vec![HookMatcher {
                    matcher: "Bash".to_string(),
                    hooks: vec![make_command_hook("echo 'test'")],
                }],
            )
            .build_arc();

        let executor = HookExecutor::new(registry);

        let result = executor
            .run_hooks(
                HookEventType::PreToolUse,
                HookEventData::pre_tool_use(
                    "Bash".to_string(),
                    serde_json::json!({}),
                    "tool-1".to_string(),
                ),
                make_context(),
                None,
                &PathBuf::from("/tmp"),
                CancellationToken::new(),
            )
            .await;

        assert_eq!(result.hooks_executed, 1);
        assert!(!result.is_blocking());
        assert_eq!(result.outcomes.len(), 1);
        assert_eq!(result.outcomes[0], HookOutcome::Success);
    }

    #[tokio::test]
    async fn test_blocking_command_hook() {
        let registry = HookRegistryBuilder::new()
            .with_global_hooks(
                HookEventType::PreToolUse,
                vec![HookMatcher {
                    matcher: "*".to_string(),
                    hooks: vec![make_command_hook("exit 2")],
                }],
            )
            .build_arc();

        let executor = HookExecutor::new(registry);

        let result = executor
            .run_hooks(
                HookEventType::PreToolUse,
                HookEventData::pre_tool_use(
                    "Bash".to_string(),
                    serde_json::json!({}),
                    "tool-1".to_string(),
                ),
                make_context(),
                None,
                &PathBuf::from("/tmp"),
                CancellationToken::new(),
            )
            .await;

        assert!(result.is_blocking());
        assert!(result.blocking_error.is_some());
    }

    #[tokio::test]
    async fn test_callback_hook() {
        let registry = HookRegistryBuilder::new()
            .with_global_hooks(
                HookEventType::SessionStart,
                vec![HookMatcher {
                    matcher: "*".to_string(),
                    hooks: vec![make_callback_hook(NoOpCallback)],
                }],
            )
            .build_arc();

        let executor = HookExecutor::new(registry);

        let result = executor
            .run_hooks(
                HookEventType::SessionStart,
                HookEventData::SessionStart {
                    source: "cli".to_string(),
                },
                make_context(),
                None,
                &PathBuf::from("/tmp"),
                CancellationToken::new(),
            )
            .await;

        assert_eq!(result.hooks_executed, 1);
        assert!(!result.is_blocking());
    }

    #[tokio::test]
    async fn test_blocking_callback_hook() {
        let registry = HookRegistryBuilder::new()
            .with_global_hooks(
                HookEventType::PreToolUse,
                vec![HookMatcher {
                    matcher: "*".to_string(),
                    hooks: vec![make_callback_hook(BlockingCallback::new("Not allowed"))],
                }],
            )
            .build_arc();

        let executor = HookExecutor::new(registry);

        let result = executor
            .run_hooks(
                HookEventType::PreToolUse,
                HookEventData::pre_tool_use(
                    "Bash".to_string(),
                    serde_json::json!({}),
                    "tool-1".to_string(),
                ),
                make_context(),
                None,
                &PathBuf::from("/tmp"),
                CancellationToken::new(),
            )
            .await;

        assert!(result.is_blocking());
    }

    #[tokio::test]
    async fn test_system_message_aggregation() {
        let registry = HookRegistryBuilder::new()
            .with_global_hooks(
                HookEventType::SessionStart,
                vec![HookMatcher {
                    matcher: "*".to_string(),
                    hooks: vec![
                        make_callback_hook(SystemMessageCallback::new("Message 1")),
                        make_callback_hook(SystemMessageCallback::new("Message 2")),
                    ],
                }],
            )
            .build_arc();

        let executor = HookExecutor::new(registry);

        let result = executor
            .run_hooks(
                HookEventType::SessionStart,
                HookEventData::SessionStart {
                    source: "cli".to_string(),
                },
                make_context(),
                None,
                &PathBuf::from("/tmp"),
                CancellationToken::new(),
            )
            .await;

        assert_eq!(result.system_messages.len(), 2);
        assert!(result.system_messages.contains(&"Message 1".to_string()));
        assert!(result.system_messages.contains(&"Message 2".to_string()));
    }

    #[tokio::test]
    async fn test_early_termination_on_blocking() {
        let registry = HookRegistryBuilder::new()
            .with_global_hooks(
                HookEventType::PreToolUse,
                vec![HookMatcher {
                    matcher: "*".to_string(),
                    hooks: vec![
                        make_callback_hook(BlockingCallback::new("Blocked")),
                        make_callback_hook(NoOpCallback), // Should not run
                    ],
                }],
            )
            .build_arc();

        let executor = HookExecutor::new(registry);

        let result = executor
            .run_hooks(
                HookEventType::PreToolUse,
                HookEventData::pre_tool_use(
                    "Bash".to_string(),
                    serde_json::json!({}),
                    "tool-1".to_string(),
                ),
                make_context(),
                None,
                &PathBuf::from("/tmp"),
                CancellationToken::new(),
            )
            .await;

        // Only one hook should have run (the blocking one)
        assert_eq!(result.outcomes.len(), 1);
        assert!(result.is_blocking());
    }

    #[test]
    fn test_hook_execution_result_empty() {
        let result = HookExecutionResult::empty();
        assert_eq!(result.hooks_executed, 0);
        assert!(!result.is_blocking());
        assert!(result.should_ask());
    }

    #[test]
    fn test_deny_wins_aggregation() {
        // Test the "deny wins" principle
        use crate::output::HookSpecificOutput;

        let outputs = vec![
            HookOutput {
                hook_specific_output: Some(HookSpecificOutput::PreToolUse {
                    permission_decision: Some(PermissionDecision::Allow),
                    permission_decision_reason: None,
                    updated_input: None,
                }),
                ..Default::default()
            },
            HookOutput {
                hook_specific_output: Some(HookSpecificOutput::PreToolUse {
                    permission_decision: Some(PermissionDecision::Deny),
                    permission_decision_reason: None,
                    updated_input: None,
                }),
                ..Default::default()
            },
        ];

        let results: Vec<HookResult> = outputs
            .into_iter()
            .map(|o| HookResult {
                outcome: HookOutcome::Success,
                output: Some(o),
                blocking_error: None,
                stdout: None,
                stderr: None,
                exit_code: None,
            })
            .collect();

        let aggregated = aggregate_results(results, 2);

        // Deny should win
        assert_eq!(aggregated.permission, PermissionDecision::Deny);
    }

    #[test]
    fn test_combined_context() {
        let result = HookExecutionResult {
            hooks_executed: 2,
            outcomes: vec![HookOutcome::Success, HookOutcome::Success],
            permission: PermissionDecision::Ask,
            system_messages: vec!["System message".to_string()],
            updated_input: None,
            additional_context: Some("Context info".to_string()),
            blocking_error: None,
            should_continue: true,
            stop_reason: None,
        };

        let combined = result.combined_context().expect("Should have context");
        assert!(combined.contains("Context info"));
        assert!(combined.contains("System message"));
    }
}
