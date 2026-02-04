//! Queued commands generator for real-time steering.
//!
//! This generator converts queued user commands (entered via Enter during streaming)
//! into system reminders that steer the model in real-time. This aligns with
//! Claude Code's dual-purpose queue mechanism where queued commands are:
//!
//! 1. Injected as `<system-reminder>User sent: {message}</system-reminder>` immediately
//! 2. Executed as new user turns after the current turn completes

use async_trait::async_trait;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::throttle::ThrottleConfig;
use crate::types::AttachmentType;
use crate::types::SystemReminder;

/// Generator for queued commands (real-time steering).
///
/// When the user queues a command during streaming, this generator
/// converts it to a steering message that the model can use to
/// adjust its current response.
#[derive(Debug)]
pub struct QueuedCommandsGenerator;

#[async_trait]
impl AttachmentGenerator for QueuedCommandsGenerator {
    fn name(&self) -> &str {
        "QueuedCommandsGenerator"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::QueuedCommands
    }

    fn is_enabled(&self, _config: &SystemReminderConfig) -> bool {
        // Always enabled - this is a core steering mechanism
        true
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttle - always inject immediately for real-time steering
        ThrottleConfig::none()
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        if ctx.queued_commands.is_empty() {
            return Ok(None);
        }

        // Convert each queued command to "User sent: {message}" format
        // This matches Claude Code's generateQueuedCommandsAttachment behavior
        let content = ctx
            .queued_commands
            .iter()
            .map(|cmd| format!("User sent: {}", cmd.prompt))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(Some(SystemReminder::new(
            AttachmentType::QueuedCommands,
            content,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::QueuedCommandInfo;
    use std::path::PathBuf;

    fn test_config() -> SystemReminderConfig {
        SystemReminderConfig::default()
    }

    #[tokio::test]
    async fn test_not_triggered_without_queued_commands() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = QueuedCommandsGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_generates_user_sent_format() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(5)
            .cwd(PathBuf::from("/tmp"))
            .queued_commands(vec![QueuedCommandInfo {
                id: "cmd-1".to_string(),
                prompt: "use TypeScript instead".to_string(),
                queued_at: 1234567890,
            }])
            .build();

        let generator = QueuedCommandsGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert_eq!(reminder.attachment_type, AttachmentType::QueuedCommands);
        assert_eq!(reminder.content, "User sent: use TypeScript instead");
        assert!(reminder.is_meta);
    }

    #[tokio::test]
    async fn test_generates_multiple_commands() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(10)
            .cwd(PathBuf::from("/tmp"))
            .queued_commands(vec![
                QueuedCommandInfo {
                    id: "cmd-1".to_string(),
                    prompt: "use TypeScript".to_string(),
                    queued_at: 1234567890,
                },
                QueuedCommandInfo {
                    id: "cmd-2".to_string(),
                    prompt: "add error handling".to_string(),
                    queued_at: 1234567891,
                },
            ])
            .build();

        let generator = QueuedCommandsGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content.contains("User sent: use TypeScript"));
        assert!(reminder.content.contains("User sent: add error handling"));
        assert!(reminder.content.contains('\n'));
    }

    #[test]
    fn test_throttle_config() {
        let generator = QueuedCommandsGenerator;
        let throttle = generator.throttle_config();
        assert_eq!(throttle.min_turns_between, 0);
    }

    #[test]
    fn test_always_enabled() {
        let generator = QueuedCommandsGenerator;
        let config = test_config();
        assert!(generator.is_enabled(&config));
    }
}
