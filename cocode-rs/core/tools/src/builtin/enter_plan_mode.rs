//! EnterPlanMode tool for transitioning to plan mode.

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_plan_mode::PlanFileManager;
use cocode_plan_mode::generate_slug;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::ToolOutput;
use serde_json::Value;

/// Tool for entering plan mode.
///
/// Transitions the agent into plan mode where it explores the codebase
/// and designs an implementation approach for user approval.
///
/// Plan files are stored at `~/.cocode/plans/{slug}.md` following
/// Claude Code v2.1.7 conventions.
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

        // Generate unique slug for this session
        let slug = generate_slug();

        // Create plan file manager and ensure directory exists
        let manager = match ctx.agent_id.as_ref() {
            Some(agent_id) => PlanFileManager::for_agent(&ctx.session_id, agent_id),
            None => PlanFileManager::new(&ctx.session_id),
        };

        let plan_path = match manager.ensure_and_get_path() {
            Ok(path) => path,
            Err(e) => {
                tracing::warn!("Failed to create plan directory: {e}");
                // Fallback to cwd-based plan file
                ctx.cwd.join(".plan.md")
            }
        };

        // Emit plan mode event with the plan file path
        ctx.emit_event(cocode_protocol::LoopEvent::PlanModeEntered {
            plan_file: plan_path.clone(),
        })
        .await;

        // Return message with plan file path (aligned with Claude Code)
        let message = format!(
            "Entered plan mode. Explore the codebase and design your implementation approach.\n\n\
             Plan file: {}\n\n\
             Use the Write tool to create your plan and the Edit tool to modify it.\n\
             When ready, use ExitPlanMode to submit for review.",
            plan_path.display()
        );

        tracing::info!(
            session_id = %ctx.session_id,
            plan_file = %plan_path.display(),
            slug = %slug,
            "Entered plan mode"
        );

        Ok(ToolOutput::text(message))
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

        // Check output contains plan file path
        if let cocode_protocol::ToolResultContent::Text(content) = &result.content {
            assert!(content.contains("Plan file:"));
            assert!(content.contains("Write tool"));
            assert!(content.contains("Edit tool"));
        }
    }

    #[tokio::test]
    async fn test_enter_plan_mode_with_agent_id() {
        let tool = EnterPlanModeTool::new();
        let mut ctx = ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"))
            .with_agent_id("explore-1");

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
