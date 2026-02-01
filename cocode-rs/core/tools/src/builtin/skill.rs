//! Skill tool for executing named skills.

use super::prompts;
use crate::context::InvokedSkill;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::ToolOutput;
use cocode_skill::register_skill_hooks;
use serde_json::Value;
use std::time::Instant;

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
        let skill_name = input["skill"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "skill must be a string",
            }
            .build()
        })?;
        let args = input["args"].as_str().unwrap_or("");

        ctx.emit_progress(format!("Executing skill: {skill_name}"))
            .await;

        // Get skill manager from context
        let skill_manager = ctx.skill_manager.as_ref().ok_or_else(|| {
            crate::error::tool_error::InternalSnafu {
                message: "Skill manager not configured",
            }
            .build()
        })?;

        // Look up the skill
        let skill = skill_manager.get(skill_name).ok_or_else(|| {
            crate::error::tool_error::NotFoundSnafu {
                name: format!("skill '{skill_name}'"),
            }
            .build()
        })?;

        // Build the prompt with argument substitution
        let prompt = if skill.prompt.contains("$ARGUMENTS") {
            skill.prompt.replace("$ARGUMENTS", args)
        } else if args.is_empty() {
            skill.prompt.clone()
        } else {
            format!("{}\n\nArguments: {}", skill.prompt, args)
        };

        // Register skill hooks if the skill has an interface with hooks
        if let Some(ref interface) = skill.interface {
            if let Some(ref registry) = ctx.hook_registry {
                let hook_count = register_skill_hooks(registry, interface);
                if hook_count > 0 {
                    tracing::debug!(skill_name, hook_count, "Registered skill hooks");
                }
            }

            // Track the invoked skill for later cleanup
            ctx.invoked_skills.lock().await.push(InvokedSkill {
                name: skill_name.to_string(),
                started_at: Instant::now(),
            });
        }

        // Return the skill prompt wrapped in XML tags for the model
        Ok(ToolOutput::text(format!(
            "<skill-invoked name=\"{skill_name}\">\n{prompt}\n</skill-invoked>"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cocode_skill::SkillManager;
    use cocode_skill::SkillPromptCommand;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn make_skill_manager() -> Arc<SkillManager> {
        let mut manager = SkillManager::new();
        manager.register(SkillPromptCommand {
            name: "commit".to_string(),
            description: "Generate a commit message".to_string(),
            prompt: "Analyze the changes and generate a commit message".to_string(),
            allowed_tools: None,
            interface: None,
        });
        manager.register(SkillPromptCommand {
            name: "review-pr".to_string(),
            description: "Review a pull request".to_string(),
            prompt: "Review PR #$ARGUMENTS".to_string(),
            allowed_tools: None,
            interface: None,
        });
        Arc::new(manager)
    }

    fn make_context() -> ToolContext {
        ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"))
            .with_skill_manager(make_skill_manager())
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
        assert!(text.contains("<skill-invoked"));
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
        // $ARGUMENTS should be replaced with "123"
        assert!(text.contains("Review PR #123"));
        assert!(text.contains("<skill-invoked"));
    }

    #[tokio::test]
    async fn test_skill_not_found() {
        let tool = SkillTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "skill": "nonexistent"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_skill_manager_not_configured() {
        let tool = SkillTool::new();
        // Context without skill manager
        let mut ctx = ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"));

        let input = serde_json::json!({
            "skill": "commit"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_properties() {
        let tool = SkillTool::new();
        assert_eq!(tool.name(), "Skill");
        assert!(!tool.is_concurrent_safe());
        assert!(!tool.is_read_only());
    }
}
