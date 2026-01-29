//! Task tool for launching sub-agents.

use super::prompts;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::{ConcurrencySafety, ToolOutput};
use serde_json::Value;

/// Tool for launching specialized sub-agents.
///
/// Delegates to a SubagentManager (connected externally).
pub struct TaskTool;

impl TaskTool {
    /// Create a new Task tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for TaskTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "Task"
    }

    fn description(&self) -> &str {
        prompts::TASK_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A short (3-5 word) description of the task"
                },
                "prompt": {
                    "type": "string",
                    "description": "The task for the agent to perform"
                },
                "subagent_type": {
                    "type": "string",
                    "description": "The type of specialized agent to use"
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Run agent in background",
                    "default": false
                },
                "model": {
                    "type": "string",
                    "description": "Optional model to use for this agent"
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Maximum turns before stopping"
                },
                "resume": {
                    "type": "string",
                    "description": "Agent ID to resume from"
                },
                "allowed_tools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tools to grant this agent"
                }
            },
            "required": ["description", "prompt", "subagent_type"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Safe
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let description = input["description"]
            .as_str()
            .ok_or_else(|| ToolError::invalid_input("description must be a string"))?;
        let prompt = input["prompt"]
            .as_str()
            .ok_or_else(|| ToolError::invalid_input("prompt must be a string"))?;
        let subagent_type = input["subagent_type"]
            .as_str()
            .ok_or_else(|| ToolError::invalid_input("subagent_type must be a string"))?;

        ctx.emit_progress(format!("Launching {subagent_type} agent: {description}"))
            .await;

        // Stub: SubagentManager will be connected in core/subagent (Step 7)
        Ok(ToolOutput::text(format!(
            "Agent '{subagent_type}' launched for: {description}\nPrompt: {prompt}\n\n\
             [SubagentManager not yet connected - this is a stub response]"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_context() -> ToolContext {
        ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"))
    }

    #[tokio::test]
    async fn test_task_tool() {
        let tool = TaskTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "description": "Search codebase",
            "prompt": "Find all error handling code",
            "subagent_type": "Explore"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[test]
    fn test_tool_properties() {
        let tool = TaskTool::new();
        assert_eq!(tool.name(), "Task");
        assert!(tool.is_concurrent_safe());
    }
}
