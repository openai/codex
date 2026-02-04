//! Agent mentions generator.
//!
//! Injects instructions for @agent-* mentions in user prompts.

use async_trait::async_trait;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::parsing::parse_agent_mentions;
use crate::types::AttachmentType;
use crate::types::ReminderTier;
use crate::types::SystemReminder;

/// Generator for @agent-* mentions.
///
/// Parses the user prompt for @agent-type mentions and generates
/// agent-specific instructions.
#[derive(Debug)]
pub struct AgentMentionsGenerator;

#[async_trait]
impl AttachmentGenerator for AgentMentionsGenerator {
    fn name(&self) -> &str {
        "AgentMentionsGenerator"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::AgentMentions
    }

    fn tier(&self) -> ReminderTier {
        ReminderTier::UserPrompt
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.attachments.agent_mentions
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Need user prompt to parse mentions
        let user_prompt = match ctx.user_prompt {
            Some(p) if !p.is_empty() => p,
            _ => return Ok(None),
        };

        // Parse @agent-* mentions from prompt
        let mentions = parse_agent_mentions(user_prompt);
        if mentions.is_empty() {
            return Ok(None);
        }

        let mut content = String::new();
        content.push_str("The user has requested the following agent types:\n\n");

        for mention in &mentions {
            let agent_type = &mention.agent_type;
            let instructions = get_agent_instructions(agent_type);
            content.push_str(&format!("## @agent-{agent_type}\n"));
            content.push_str(&format!("{instructions}\n\n"));
        }

        Ok(Some(SystemReminder::new(
            AttachmentType::AgentMentions,
            content.trim(),
        )))
    }
}

/// Get instructions for a specific agent type.
fn get_agent_instructions(agent_type: &str) -> &'static str {
    match agent_type {
        "search" | "explore" => {
            "Use search and exploration tools to find relevant files and code patterns. \
             Focus on understanding the codebase structure before making changes."
        }
        "edit" | "write" => {
            "Focus on making precise code modifications. Use the Edit tool for existing files \
             and Write tool only when creating new files is necessary."
        }
        "plan" => {
            "Create a detailed implementation plan before writing code. Break down the task \
             into clear steps and identify dependencies between tasks."
        }
        "review" | "analyze" => {
            "Analyze the code carefully for potential issues, bugs, or improvements. \
             Consider security, performance, and maintainability aspects."
        }
        "test" => {
            "Focus on testing aspects: run existing tests, write new tests, and ensure \
             proper test coverage for the changes being made."
        }
        "debug" => {
            "Investigate the issue systematically. Look at error messages, stack traces, \
             and relevant code paths to identify the root cause."
        }
        "refactor" => {
            "Focus on improving code structure without changing behavior. Identify \
             opportunities for simplification and better organization."
        }
        _ => {
            "Proceed with the requested agent type. If unsure about the specific behavior, \
             ask the user for clarification."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_config() -> SystemReminderConfig {
        SystemReminderConfig::default()
    }

    #[tokio::test]
    async fn test_no_mentions() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .has_user_input(true)
            .user_prompt("Hello, how are you?")
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = AgentMentionsGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_agent_mention() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .has_user_input(true)
            .user_prompt("Use @agent-search to find the files")
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = AgentMentionsGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content.contains("@agent-search"));
        assert!(reminder.content.contains("search and exploration"));
    }

    #[tokio::test]
    async fn test_multiple_agent_mentions() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .has_user_input(true)
            .user_prompt("Use @agent-plan then @agent-edit")
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = AgentMentionsGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content.contains("@agent-plan"));
        assert!(reminder.content.contains("@agent-edit"));
    }

    #[test]
    fn test_agent_instructions() {
        assert!(get_agent_instructions("search").contains("search"));
        assert!(get_agent_instructions("edit").contains("Edit tool"));
        assert!(get_agent_instructions("plan").contains("implementation plan"));
        assert!(get_agent_instructions("unknown").contains("Proceed"));
    }

    #[test]
    fn test_generator_properties() {
        let generator = AgentMentionsGenerator;
        assert_eq!(generator.name(), "AgentMentionsGenerator");
        assert_eq!(generator.tier(), ReminderTier::UserPrompt);
        assert_eq!(generator.attachment_type(), AttachmentType::AgentMentions);
    }
}
