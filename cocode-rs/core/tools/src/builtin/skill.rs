//! Skill tool for executing named skills.

use super::prompts;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::{ConcurrencySafety, ToolOutput};
use serde_json::Value;

/// Tool for executing named skills (slash commands).
///
/// Delegates to the skill system to load and run skills
/// defined in the project or user configuration.
pub struct SkillTool;

impl SkillTool {
    /// Create a new Skill tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for SkillTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "Skill"
    }

    fn description(&self) -> &str {
        prompts::SKILL_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "skill": {
                    "type": "string",
                    "description": "The skill name. E.g., 'commit', 'review-pr', or 'pdf'"
                },
                "args": {
                    "type": "string",
                    "description": "Optional arguments for the skill"
                }
            },
            "required": ["skill"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Unsafe
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let skill_name = input["skill"]
            .as_str()
            .ok_or_else(|| ToolError::invalid_input("skill must be a string"))?;
        let args = input["args"].as_str().unwrap_or("");

        ctx.emit_progress(format!("Executing skill: {skill_name}"))
            .await;

        // Stub: SkillLoader/SkillScanner will be connected from cocode-skill crate
        Ok(ToolOutput::text(format!(
            "Skill '{skill_name}' invoked{}\n\n\
             [Skill system not yet connected — this is a stub response.\n\
             To enable, wire up the cocode-skill crate.]",
            if args.is_empty() {
                String::new()
            } else {
                format!(" with args: {args}")
            }
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
    async fn test_skill_tool() {
        let tool = SkillTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "skill": "commit"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
        let text = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("commit"));
    }

    #[tokio::test]
    async fn test_skill_tool_with_args() {
        let tool = SkillTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "skill": "review-pr",
            "args": "123"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
        let text = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("review-pr"));
        assert!(text.contains("123"));
    }

    #[test]
    fn test_tool_properties() {
        let tool = SkillTool::new();
        assert_eq!(tool.name(), "Skill");
        assert!(!tool.is_concurrent_safe());
        assert!(!tool.is_read_only());
    }
}
