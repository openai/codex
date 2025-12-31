//! Subagent Tool Handlers
//!
//! Handlers for Task and TaskOutput tools that integrate with the subagent system.
//! Uses the delegate pattern (run_subagent_delegate) to spawn full Codex sessions
//! for subagent execution.

use crate::function_tool::FunctionCallError;
use crate::subagent::SubagentConfigBuilder;
use crate::subagent::SubagentStatus;
use crate::subagent::get_or_create_stores;
use crate::subagent::run_subagent_delegate;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

/// Arguments for Task tool invocation
#[derive(Debug, Clone, Deserialize)]
pub struct TaskArgs {
    pub subagent_type: String,
    pub prompt: String,
    pub description: String,
    /// Provider name override - references config.model_providers HashMap key.
    /// Takes highest priority for provider selection.
    #[serde(default)]
    #[allow(dead_code)] // Reserved for model selection
    pub model_provider: Option<String>,
    /// Model name override.
    #[serde(default)]
    #[allow(dead_code)]
    pub model: Option<String>,
    #[serde(default)]
    pub run_in_background: bool,
    #[serde(default)]
    pub resume: Option<String>,
}

/// Task Tool Handler
///
/// Spawns subagents using the delegate pattern (run_subagent_delegate).
/// This approach spawns full Codex sessions for subagent execution,
/// enabling access to all tools, skills, compact, and MCP integration.
#[derive(Debug, Default)]
pub struct TaskHandler;

impl TaskHandler {
    /// Create a new Task handler.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ToolHandler for TaskHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // Parse arguments
        let arguments = match &invocation.payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "Invalid payload type for Task".to_string(),
                ));
            }
        };

        let args: TaskArgs = serde_json::from_str(arguments)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid arguments: {e}")))?;

        // Get session-scoped stores from global registry
        let stores = get_or_create_stores(invocation.session.conversation_id);

        // Get agent definition
        let definition = stores
            .registry
            .get(&args.subagent_type)
            .await
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(format!(
                    "Unknown subagent type '{}'. Available types: Explore, Plan",
                    args.subagent_type
                ))
            })?;

        // Get base config from parent session
        let base_config = {
            let state = invocation.session.state.lock().await;
            state
                .session_configuration
                .original_config_do_not_use
                .clone()
        };

        // Build subagent config using the config builder
        let subagent_config = SubagentConfigBuilder::new(base_config, definition.clone()).build();

        // Get auth_manager and models_manager from parent session services
        let auth_manager = invocation.session.services.auth_manager.clone();
        let models_manager = invocation.session.services.models_manager.clone();

        // Create cancellation token for subagent (child of parent's token)
        let cancel_token = invocation.cancellation_token.child_token();

        if args.run_in_background {
            // Generate agent ID for tracking
            let agent_id = generate_agent_id(&args.subagent_type);
            let prompt = args.prompt.clone();
            let description = args.description.clone();

            // Phase 1: Pre-register with Pending status (before spawn)
            stores.background_store.register_pending(
                agent_id.clone(),
                description.clone(),
                prompt.clone(),
            );

            // Clone what we need for the spawned task
            let agent_id_for_task = agent_id.clone();
            let parent_session = invocation.session.clone();
            let parent_ctx = invocation.turn.clone();
            let transcript_store = stores.transcript_store.clone();

            let handle = tokio::spawn(async move {
                run_subagent_delegate(
                    subagent_config,
                    prompt,
                    auth_manager,
                    models_manager,
                    parent_session,
                    parent_ctx,
                    cancel_token,
                    None, // No event sender for background tasks
                    Some(&transcript_store),
                    None, // No resume for new background tasks
                )
                .await
                .unwrap_or_else(|e| crate::subagent::SubagentResult {
                    status: SubagentStatus::Error,
                    result: format!("Spawn error: {e}"),
                    turns_used: 0,
                    duration: Duration::ZERO,
                    agent_id: agent_id_for_task,
                    total_tool_use_count: 0,
                    total_duration_ms: 0,
                    total_tokens: 0,
                    usage: None,
                })
            });

            // Phase 2: Set handle and transition to Running status
            stores.background_store.set_handle(&agent_id, handle);

            Ok(ToolOutput::Function {
                content: serde_json::json!({
                    "status": "async_launched",
                    "agent_id": agent_id,
                    "description": description,
                })
                .to_string(),
                content_items: None,
                success: Some(true),
            })
        } else {
            // Synchronous execution using delegate
            let result = run_subagent_delegate(
                subagent_config,
                args.prompt,
                auth_manager,
                models_manager,
                invocation.session.clone(),
                invocation.turn.clone(),
                cancel_token,
                None, // No event sender for synchronous execution
                Some(&stores.transcript_store),
                args.resume.as_deref(),
            )
            .await
            .map_err(|e| FunctionCallError::RespondToModel(format!("Execution failed: {e}")))?;

            Ok(ToolOutput::Function {
                content: serde_json::json!({
                    "status": result.status,
                    "result": result.result,
                    "turns_used": result.turns_used,
                    "duration_seconds": result.duration.as_secs_f32(),
                    "agent_id": result.agent_id,
                    "total_tool_use_count": result.total_tool_use_count,
                    "total_tokens": result.total_tokens,
                })
                .to_string(),
                content_items: None,
                success: Some(result.status == SubagentStatus::Goal),
            })
        }
    }
}

/// Generate a unique agent ID.
fn generate_agent_id(agent_type: &str) -> String {
    use std::time::SystemTime;
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let random: u32 = rand::random();
    format!("{agent_type}-{timestamp:x}-{random:04x}")
}

/// Arguments for TaskOutput tool invocation
#[derive(Debug, Clone, Deserialize)]
pub struct TaskOutputArgs {
    pub agent_id: String,
    #[serde(default = "default_block")]
    pub block: bool,
    #[serde(default = "default_timeout")]
    pub timeout: i32,
}

fn default_block() -> bool {
    true
}

fn default_timeout() -> i32 {
    300
}

/// TaskOutput Tool Handler
///
/// Retrieves results from background subagent tasks.
/// Stores are obtained from the global registry using conversation_id.
#[derive(Debug, Default)]
pub struct TaskOutputHandler;

/// Default cleanup duration for old tasks and transcripts (1 hour).
const CLEANUP_OLDER_THAN: Duration = Duration::from_secs(60 * 60);

impl TaskOutputHandler {
    /// Create a new TaskOutput handler.
    /// Stores are obtained from global registry at runtime via conversation_id.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ToolHandler for TaskOutputHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // Parse arguments
        let arguments = match &invocation.payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "Invalid payload type for TaskOutput".to_string(),
                ));
            }
        };

        let args: TaskOutputArgs = serde_json::from_str(arguments)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid arguments: {e}")))?;

        // Get session-scoped stores from global registry
        let stores = get_or_create_stores(invocation.session.conversation_id);

        let timeout = Duration::from_secs(args.timeout as u64);

        match stores
            .background_store
            .get_result(&args.agent_id, args.block, timeout)
            .await
        {
            Some(result) => {
                // Trigger cleanup opportunistically after retrieving a result
                stores
                    .background_store
                    .cleanup_old_tasks(CLEANUP_OLDER_THAN);
                stores
                    .transcript_store
                    .cleanup_old_transcripts(CLEANUP_OLDER_THAN);

                Ok(ToolOutput::Function {
                    content: serde_json::json!({
                        "status": result.status,
                        "result": result.result,
                        "turns_used": result.turns_used,
                        "duration_seconds": result.duration.as_secs_f32(),
                    })
                    .to_string(),
                    content_items: None,
                    success: Some(result.status == SubagentStatus::Goal),
                })
            }
            None => {
                let status = stores.background_store.get_status(&args.agent_id);
                Ok(ToolOutput::Function {
                    content: serde_json::json!({
                        "status": status.map(|s| format!("{:?}", s)).unwrap_or("not_found".to_string()),
                        "message": if status.is_some() {
                            "Task still running or timed out waiting"
                        } else {
                            "No task found with that agent_id"
                        },
                    })
                    .to_string(),
                    content_items: None,
                    success: Some(false),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_handler_kind() {
        let handler = TaskHandler::new();
        assert_eq!(handler.kind(), ToolKind::Function);
    }

    #[test]
    fn test_task_output_handler_kind() {
        let handler = TaskOutputHandler::new();
        assert_eq!(handler.kind(), ToolKind::Function);
    }

    #[test]
    fn test_parse_task_args() {
        let args: TaskArgs = serde_json::from_str(
            r#"{"subagent_type": "Explore", "prompt": "Find files", "description": "Finding files"}"#,
        )
        .expect("should parse");
        assert_eq!(args.subagent_type, "Explore");
        assert_eq!(args.prompt, "Find files");
        assert!(!args.run_in_background);
    }

    #[test]
    fn test_parse_task_output_args() {
        let args: TaskOutputArgs =
            serde_json::from_str(r#"{"agent_id": "agent-123"}"#).expect("should parse");
        assert_eq!(args.agent_id, "agent-123");
        assert!(args.block);
        assert_eq!(args.timeout, 300);
    }

    #[test]
    fn test_parse_task_args_with_model_provider() {
        let args: TaskArgs = serde_json::from_str(
            r#"{"subagent_type": "Explore", "prompt": "Find files", "description": "Finding files", "model_provider": "openai", "model": "gpt-4"}"#,
        )
        .expect("should parse");
        assert_eq!(args.subagent_type, "Explore");
        assert_eq!(args.model_provider, Some("openai".to_string()));
        assert_eq!(args.model, Some("gpt-4".to_string()));
    }

    #[test]
    fn test_generate_agent_id() {
        let id1 = generate_agent_id("Explore");
        let id2 = generate_agent_id("Explore");
        assert!(id1.starts_with("Explore-"));
        assert!(id2.starts_with("Explore-"));
        // IDs should be unique
        assert_ne!(id1, id2);
    }
}
