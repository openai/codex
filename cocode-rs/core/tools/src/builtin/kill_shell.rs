//! KillShell tool for stopping background tasks.

use super::prompts;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::{ConcurrencySafety, ToolOutput};
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
        let task_id = input["task_id"]
            .as_str()
            .ok_or_else(|| ToolError::invalid_input("task_id must be a string"))?;

        ctx.emit_progress(format!("Stopping task {task_id}")).await;

        // Stub: BackgroundTaskRegistry will be connected in exec/shell (Step 4)
        Ok(ToolOutput::text(format!(
            "Task {task_id} stop requested. [BackgroundTaskRegistry not yet connected]"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_kill_shell_tool() {
        let tool = KillShellTool::new();
        let mut ctx = ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"));

        let input = serde_json::json!({
            "task_id": "task-123"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[test]
    fn test_tool_properties() {
        let tool = KillShellTool::new();
        assert_eq!(tool.name(), "TaskStop");
        assert!(tool.is_concurrent_safe());
    }
}
