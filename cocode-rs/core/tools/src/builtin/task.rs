//! Task tool for launching sub-agents.

use super::prompts;
use crate::context::SpawnAgentInput;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::ToolOutput;
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
        let description = input["description"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "description must be a string",
            }
            .build()
        })?;
        let prompt = input["prompt"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "prompt must be a string",
            }
            .build()
        })?;
        let subagent_type = input["subagent_type"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "subagent_type must be a string",
            }
            .build()
        })?;

        // Parse optional fields
        let run_in_background = input["run_in_background"].as_bool().unwrap_or(false);
        let model = input["model"].as_str().map(String::from);
        let max_turns = input["max_turns"].as_i64().map(|n| n as i32);
        let allowed_tools = input["allowed_tools"].as_array().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

        ctx.emit_progress(format!("Launching {subagent_type} agent: {description}"))
            .await;

        // Check if spawning is available
        if !ctx.can_spawn_agent() {
            return Ok(ToolOutput::text(format!(
                "Agent '{subagent_type}' launched for: {description}\nPrompt: {prompt}\n\n\
                 [SubagentManager not configured - returning stub response]"
            )));
        }

        // Build spawn input with parent's selections for isolation
        let spawn_input = SpawnAgentInput {
            agent_type: subagent_type.to_string(),
            prompt: prompt.to_string(),
            model,
            max_turns,
            run_in_background,
            allowed_tools,
            parent_selections: ctx.parent_selections.clone(),
            permission_mode: None, // Resolved by driver from AgentDefinition
        };

        // Spawn the agent
        match ctx.spawn_agent(spawn_input).await {
            Ok(result) => {
                if run_in_background {
                    // Background agent - return ID and output file path
                    let output_path = result
                        .output_file
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    Ok(ToolOutput::text(format!(
                        "Agent '{subagent_type}' started in background.\n\
                         Agent ID: {}\n\
                         Output file: {output_path}",
                        result.agent_id
                    )))
                } else {
                    // Foreground agent - return the result
                    let output = result.output.unwrap_or_else(|| {
                        format!("Agent '{subagent_type}' completed with no output.")
                    });
                    Ok(ToolOutput::text(format!(
                        "agentId: {}\n\n{}",
                        result.agent_id, output
                    )))
                }
            }
            Err(e) => Ok(ToolOutput::error(format!(
                "Failed to spawn agent '{subagent_type}': {e}"
            ))),
        }
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
