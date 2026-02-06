//! Delegate mode generator.
//!
//! This generator provides instructions when operating in delegate mode,
//! where the main agent delegates work to specialized sub-agents.

use async_trait::async_trait;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::throttle::ThrottleConfig;
use crate::types::AttachmentType;
use crate::types::SystemReminder;

/// Generator for delegate mode instructions.
///
/// Provides context and instructions when the main agent is operating
/// in delegate mode, coordinating with specialized sub-agents.
#[derive(Debug)]
pub struct DelegateModeGenerator;

#[async_trait]
impl AttachmentGenerator for DelegateModeGenerator {
    fn name(&self) -> &str {
        "DelegateModeGenerator"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::DelegateMode
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.attachments.delegate_mode
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // Reminder every 5 turns while in delegate mode
        ThrottleConfig {
            min_turns_between: 5,
            min_turns_after_trigger: 0,
            max_per_session: None,
        }
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        if !ctx.is_delegate_mode {
            return Ok(None);
        }

        // Build agent status section if there are delegated agents
        let agent_status = if !ctx.delegated_agents.is_empty() {
            let mut lines = vec!["## Active Agents\n".to_string()];
            for agent in &ctx.delegated_agents {
                lines.push(format!(
                    "- **{}** ({}): {} - {}",
                    agent.agent_id, agent.agent_type, agent.status, agent.description
                ));
            }
            lines.join("\n")
        } else {
            String::new()
        };

        // Different message if exiting delegate mode
        let content = if ctx.delegate_mode_exiting {
            format!("{}\n\n{}", DELEGATE_MODE_EXIT_INSTRUCTIONS, agent_status)
        } else {
            format!("{}\n\n{}", DELEGATE_MODE_INSTRUCTIONS, agent_status)
        };

        Ok(Some(SystemReminder::new(
            AttachmentType::DelegateMode,
            content.trim().to_string(),
        )))
    }
}

/// Instructions for delegate mode.
const DELEGATE_MODE_INSTRUCTIONS: &str = r#"## Delegate Mode Active

You are operating in delegate mode, coordinating with specialized agents.

**Guidelines:**
- Monitor agent progress and handle any issues
- Synthesize results from completed agents
- Delegate appropriate tasks to specialized agents when beneficial
- Keep the user informed of overall progress
- You can run multiple agents in parallel when tasks are independent"#;

/// Instructions when exiting delegate mode.
const DELEGATE_MODE_EXIT_INSTRUCTIONS: &str = r#"## Exiting Delegate Mode

Delegate mode is ending. Please:

1. Review outputs from all completed agents
2. Synthesize the results into a coherent response
3. Address any incomplete or failed tasks
4. Provide a summary to the user"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::DelegatedAgentInfo;
    use std::path::PathBuf;

    fn test_config() -> SystemReminderConfig {
        SystemReminderConfig::default()
    }

    #[tokio::test]
    async fn test_not_triggered_when_not_delegate_mode() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .is_delegate_mode(false)
            .build();

        let generator = DelegateModeGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_triggered_in_delegate_mode() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .is_delegate_mode(true)
            .build();

        let generator = DelegateModeGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content().unwrap().contains("Delegate Mode Active"));
    }

    #[tokio::test]
    async fn test_shows_agent_status() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .is_delegate_mode(true)
            .delegated_agents(vec![
                DelegatedAgentInfo {
                    agent_id: "agent-1".to_string(),
                    agent_type: "Explore".to_string(),
                    status: "running".to_string(),
                    description: "Searching for API endpoints".to_string(),
                },
                DelegatedAgentInfo {
                    agent_id: "agent-2".to_string(),
                    agent_type: "Plan".to_string(),
                    status: "completed".to_string(),
                    description: "Planning implementation".to_string(),
                },
            ])
            .build();

        let generator = DelegateModeGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content().unwrap().contains("Active Agents"));
        assert!(reminder.content().unwrap().contains("agent-1"));
        assert!(reminder.content().unwrap().contains("Explore"));
        assert!(
            reminder
                .content()
                .unwrap()
                .contains("Searching for API endpoints")
        );
    }

    #[tokio::test]
    async fn test_exit_message() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .is_delegate_mode(true)
            .delegate_mode_exiting(true)
            .build();

        let generator = DelegateModeGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(
            reminder
                .content()
                .unwrap()
                .contains("Exiting Delegate Mode")
        );
        assert!(
            reminder
                .content()
                .unwrap()
                .contains("Synthesize the results")
        );
    }

    #[test]
    fn test_throttle_config() {
        let generator = DelegateModeGenerator;
        let throttle = generator.throttle_config();
        assert_eq!(throttle.min_turns_between, 5);
    }
}
