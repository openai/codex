//! TaskOutput tool for retrieving output from background tasks.

use super::prompts;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::{ConcurrencySafety, ToolOutput};
use serde_json::Value;

/// Tool for retrieving output from background tasks or agents.
pub struct TaskOutputTool;

impl TaskOutputTool {
    /// Create a new TaskOutput tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for TaskOutputTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> &str {
        "TaskOutput"
    }

    fn description(&self) -> &str {
        prompts::TASK_OUTPUT_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to get output from"
                },
                "block": {
                    "type": "boolean",
                    "description": "Whether to wait for completion",
                    "default": true
                },
                "timeout": {
                    "type": "integer",
                    "description": "Max wait time in ms",
                    "default": 30000
                }
            },
            "required": ["task_id"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Safe
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let task_id = input["task_id"]
            .as_str()
            .ok_or_else(|| ToolError::invalid_input("task_id must be a string"))?;
        let _block = input["block"].as_bool().unwrap_or(true);
        let _timeout_ms = input["timeout"].as_i64().unwrap_or(30_000);

        ctx.emit_progress(format!("Getting output for task {task_id}"))
            .await;

        // Stub: BackgroundTaskRegistry will be connected in exec/shell (Step 4)
        Ok(ToolOutput::text(format!(
            "Task {task_id}: [BackgroundTaskRegistry not yet connected - stub response]"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_task_output_tool() {
        let tool = TaskOutputTool::new();
        let mut ctx = ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"));

        let input = serde_json::json!({
            "task_id": "task-123",
            "block": true,
            "timeout": 5000
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[test]
    fn test_tool_properties() {
        let tool = TaskOutputTool::new();
        assert_eq!(tool.name(), "TaskOutput");
        assert!(tool.is_concurrent_safe());
    }
}
