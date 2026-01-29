//! ExitPlanMode tool for finalizing plan and requesting approval.

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::{ConcurrencySafety, ToolOutput};
use serde_json::Value;

/// Tool for exiting plan mode.
///
/// Signals that the plan is complete and ready for user review and approval.
pub struct ExitPlanModeTool;

impl ExitPlanModeTool {
    /// Create a new ExitPlanMode tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExitPlanModeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        "ExitPlanMode"
    }

    fn description(&self) -> &str {
        prompts::EXIT_PLAN_MODE_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "allowedPrompts": {
                    "type": "array",
                    "description": "Prompt-based permissions needed to implement the plan",
                    "items": {
                        "type": "object",
                        "properties": {
                            "tool": {
                                "type": "string",
                                "description": "The tool this prompt applies to"
                            },
                            "prompt": {
                                "type": "string",
                                "description": "Semantic description of the action"
                            }
                        },
                        "required": ["tool", "prompt"]
                    }
                }
            },
            "additionalProperties": true
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Unsafe
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, _input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        ctx.emit_progress("Exiting plan mode - awaiting approval")
            .await;

        // Emit plan mode exit event
        ctx.emit_event(cocode_protocol::LoopEvent::PlanModeExited { approved: false })
            .await;

        Ok(ToolOutput::text(
            "Plan submitted for review. Waiting for user approval.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_exit_plan_mode() {
        let tool = ExitPlanModeTool::new();
        let mut ctx = ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"));

        let input = serde_json::json!({});
        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_exit_plan_mode_with_prompts() {
        let tool = ExitPlanModeTool::new();
        let mut ctx = ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"));

        let input = serde_json::json!({
            "allowedPrompts": [
                {"tool": "Bash", "prompt": "run tests"},
                {"tool": "Bash", "prompt": "install dependencies"}
            ]
        });
        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[test]
    fn test_tool_properties() {
        let tool = ExitPlanModeTool::new();
        assert_eq!(tool.name(), "ExitPlanMode");
        assert!(!tool.is_concurrent_safe());
    }
}
