//! KillShell tool for stopping background tasks.

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::ToolOutput;
use serde_json::Value;

/// Tool for stopping background shell processes or agents.
pub struct KillShellTool;

impl KillShellTool {
    /// Create a new KillShell tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for KillShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for KillShellTool {
    fn name(&self) -> &str {
        "TaskStop"
    }

    fn description(&self) -> &str {
        prompts::TASK_STOP_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The ID of the background task to stop"
                }
            },
            "required": ["task_id"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Safe
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let task_id = input["task_id"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "task_id must be a string",
            }
            .build()
        })?;

        ctx.emit_progress(format!("Stopping task {task_id}")).await;

        // Get final output before stopping
        let final_output = ctx
            .shell_executor
            .background_registry
            .get_output(task_id)
            .await
            .unwrap_or_default();

        // Stop the task
        let was_running = ctx.shell_executor.background_registry.stop(task_id).await;

        if was_running {
            Ok(ToolOutput::text(format!(
                "Task {task_id} stopped successfully.\n\nFinal output:\n{final_output}"
            )))
        } else {
            Ok(ToolOutput::error(format!(
                "Task {task_id} not found. It may have already completed or never started."
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cocode_shell::BackgroundProcess;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::sync::Notify;

    #[tokio::test]
    async fn test_kill_shell_tool_not_found() {
        let tool = KillShellTool::new();
        let mut ctx = ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"));

        let input = serde_json::json!({
            "task_id": "task-nonexistent"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        // Non-existent task returns error
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_kill_shell_tool_stops_task() {
        let tool = KillShellTool::new();
        let mut ctx = ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"));

        // Register a background task
        let output = Arc::new(Mutex::new("task output".to_string()));
        let process = BackgroundProcess {
            id: "task-123".to_string(),
            command: "sleep 60".to_string(),
            output,
            completed: Arc::new(Notify::new()),
        };
        ctx.shell_executor
            .background_registry
            .register("task-123".to_string(), process)
            .await;

        // Verify task is running
        assert!(
            ctx.shell_executor
                .background_registry
                .is_running("task-123")
                .await
        );

        let input = serde_json::json!({
            "task_id": "task-123"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
        match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => {
                assert!(t.contains("stopped successfully"));
                assert!(t.contains("task output"));
            }
            _ => panic!("Expected text content"),
        }

        // Verify task is no longer running
        assert!(
            !ctx.shell_executor
                .background_registry
                .is_running("task-123")
                .await
        );
    }

    #[test]
    fn test_tool_properties() {
        let tool = KillShellTool::new();
        assert_eq!(tool.name(), "TaskStop");
        assert!(tool.is_concurrent_safe());
    }
}
