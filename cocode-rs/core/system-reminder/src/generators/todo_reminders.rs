//! Todo/task reminders generator.
//!
//! Injects current task list context to help the agent track progress.

use async_trait::async_trait;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::generator::TodoStatus;
use crate::throttle::ThrottleConfig;
use crate::types::AttachmentType;
use crate::types::ReminderTier;
use crate::types::SystemReminder;

/// Generator for todo/task reminders.
#[derive(Debug)]
pub struct TodoRemindersGenerator;

#[async_trait]
impl AttachmentGenerator for TodoRemindersGenerator {
    fn name(&self) -> &str {
        "TodoRemindersGenerator"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::TodoReminders
    }

    fn tier(&self) -> ReminderTier {
        ReminderTier::MainAgentOnly
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.attachments.todo_reminders
    }

    fn throttle_config(&self) -> ThrottleConfig {
        ThrottleConfig::todo_reminder()
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        if !ctx.has_todos() {
            return Ok(None);
        }

        let mut content = String::new();
        content.push_str("## Current Tasks\n\n");

        // Group tasks by status
        let in_progress: Vec<_> = ctx.in_progress_todos().collect();
        let pending: Vec<_> = ctx.pending_todos().collect();

        if !in_progress.is_empty() {
            content.push_str("### In Progress\n");
            for task in &in_progress {
                let blocked = if task.is_blocked { " (blocked)" } else { "" };
                content.push_str(&format!("- [{}] {}{}\n", task.id, task.subject, blocked));
            }
            content.push('\n');
        }

        if !pending.is_empty() {
            content.push_str("### Pending\n");
            for task in &pending {
                let blocked = if task.is_blocked { " (blocked)" } else { "" };
                content.push_str(&format!("- [{}] {}{}\n", task.id, task.subject, blocked));
            }
            content.push('\n');
        }

        // Add summary
        let completed_count = ctx
            .todos
            .iter()
            .filter(|t| t.status == TodoStatus::Completed)
            .count();
        let total = ctx.todos.len();

        content.push_str(&format!(
            "Progress: {completed_count}/{total} tasks completed\n"
        ));

        content.push_str("\nUse TaskUpdate to mark tasks as in_progress or completed.");

        Ok(Some(SystemReminder::new(
            AttachmentType::TodoReminders,
            content,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::TodoItem;
    use std::path::PathBuf;

    fn test_config() -> SystemReminderConfig {
        SystemReminderConfig::default()
    }

    #[tokio::test]
    async fn test_no_todos() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = TodoRemindersGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_with_todos() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .todos(vec![
                TodoItem {
                    id: "1".to_string(),
                    subject: "Implement feature X".to_string(),
                    status: TodoStatus::InProgress,
                    is_blocked: false,
                },
                TodoItem {
                    id: "2".to_string(),
                    subject: "Write tests".to_string(),
                    status: TodoStatus::Pending,
                    is_blocked: false,
                },
                TodoItem {
                    id: "3".to_string(),
                    subject: "Update docs".to_string(),
                    status: TodoStatus::Completed,
                    is_blocked: false,
                },
            ])
            .build();

        let generator = TodoRemindersGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content().unwrap().contains("In Progress"));
        assert!(reminder.content().unwrap().contains("Implement feature X"));
        assert!(reminder.content().unwrap().contains("Pending"));
        assert!(reminder.content().unwrap().contains("Write tests"));
        assert!(reminder.content().unwrap().contains("1/3 tasks completed"));
    }

    #[tokio::test]
    async fn test_blocked_tasks() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .todos(vec![TodoItem {
                id: "1".to_string(),
                subject: "Blocked task".to_string(),
                status: TodoStatus::Pending,
                is_blocked: true,
            }])
            .build();

        let generator = TodoRemindersGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content().unwrap().contains("(blocked)"));
    }

    #[test]
    fn test_generator_properties() {
        let generator = TodoRemindersGenerator;
        assert_eq!(generator.name(), "TodoRemindersGenerator");
        assert_eq!(generator.tier(), ReminderTier::MainAgentOnly);

        let throttle = generator.throttle_config();
        assert_eq!(throttle.min_turns_between, 5);
    }
}
