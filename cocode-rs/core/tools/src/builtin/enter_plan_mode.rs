//! EnterPlanMode tool for transitioning to plan mode.

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::{ConcurrencySafety, ToolOutput};
use serde_json::Value;

/// Tool for entering plan mode.
///
/// Transitions the agent into plan mode where it explores the codebase
/// and designs an implementation approach for user approval.
pub struct EnterPlanModeTool;

impl EnterPlanModeTool {
    /// Create a new EnterPlanMode tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for EnterPlanModeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str {
        "EnterPlanMode"
    }

    fn description(&self) -> &str {
        prompts::ENTER_PLAN_MODE_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Unsafe
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, _input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        ctx.emit_progress("Entering plan mode").await;

        // Emit plan mode event
        ctx.emit_event(cocode_protocol::LoopEvent::PlanModeEntered {
            plan_file: ctx.cwd.join(".plan.md"),
        })
        .await;

        Ok(ToolOutput::text(
            "Entered plan mode. Explore the codebase and design your implementation approach.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_enter_plan_mode() {
        let tool = EnterPlanModeTool::new();
        let mut ctx = ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"));

        let input = serde_json::json!({});
        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[test]
    fn test_tool_properties() {
        let tool = EnterPlanModeTool::new();
        assert_eq!(tool.name(), "EnterPlanMode");
        assert!(!tool.is_concurrent_safe());
    }
}
