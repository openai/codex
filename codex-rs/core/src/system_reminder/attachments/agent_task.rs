//! Agent task status generator.
//!
//! Notification of background async agent completion (P1).
//! Matches generateAsyncAgentsAttachment (hH5) in Claude Code chunks.107.mjs:2522-2551.

use crate::config::system_reminder::SystemReminderConfig;
use crate::error::Result;
use crate::system_reminder::generator::AttachmentGenerator;
use crate::system_reminder::generator::BackgroundTaskInfo;
use crate::system_reminder::generator::BackgroundTaskStatus;
use crate::system_reminder::generator::BackgroundTaskType;
use crate::system_reminder::generator::GeneratorContext;
use crate::system_reminder::throttle::ThrottleConfig;
use crate::system_reminder::types::AttachmentType;
use crate::system_reminder::types::ReminderTier;
use crate::system_reminder::types::SystemReminder;
use async_trait::async_trait;

/// Agent task status generator.
///
/// Generates notifications about completed async agents using `<system-notification>` tag.
#[derive(Debug)]
pub struct AgentTaskGenerator;

impl AgentTaskGenerator {
    /// Create a new agent task generator.
    pub fn new() -> Self {
        Self
    }

    /// Build the notification content from agent updates.
    fn build_content(&self, tasks: &[&BackgroundTaskInfo]) -> String {
        let mut content = String::new();

        for task in tasks {
            let status_str = match task.status {
                BackgroundTaskStatus::Completed => "completed",
                BackgroundTaskStatus::Failed => "failed",
                BackgroundTaskStatus::Running => "running",
            };
            content.push_str(&format!(
                "Async agent \"{}\" {}. The output can be retrieved using TaskOutput with agentId: \"{}\"",
                task.description, status_str, task.task_id
            ));
            content.push('\n');
        }

        content.trim_end().to_string()
    }
}

impl Default for AgentTaskGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AttachmentGenerator for AgentTaskGenerator {
    fn name(&self) -> &str {
        "agent_task"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::AsyncAgentStatus
    }

    fn tier(&self) -> ReminderTier {
        ReminderTier::MainAgentOnly
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Only for main agent
        if !ctx.is_main_agent {
            return Ok(None);
        }

        // Filter to async agent tasks that have completed/failed and not yet notified
        let updates: Vec<_> = ctx
            .background_tasks
            .iter()
            .filter(|t| {
                t.task_type == BackgroundTaskType::AsyncAgent
                    && t.status != BackgroundTaskStatus::Running
                    && !t.notified
            })
            .collect();

        if updates.is_empty() {
            return Ok(None);
        }

        tracing::info!(
            generator = "agent_task",
            task_count = updates.len(),
            "Generating agent task status notification"
        );
        Ok(Some(SystemReminder::new(
            AttachmentType::AsyncAgentStatus,
            self.build_content(&updates),
        )))
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        // Use the background_task setting for now (shares config)
        config.enabled && config.attachments.background_task
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttling - immediate notification on completion
        ThrottleConfig {
            min_turns_between: 0,
            min_turns_after_trigger: 0,
            max_per_session: None,
        }
    }
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::system_reminder::LspDiagnosticsMinSeverity;
    use crate::system_reminder::file_tracker::FileTracker;
    use crate::system_reminder::generator::PlanState;
    use std::path::Path;

    fn make_context<'a>(
        is_main_agent: bool,
        background_tasks: &'a [BackgroundTaskInfo],
        file_tracker: &'a FileTracker,
        plan_state: &'a PlanState,
    ) -> GeneratorContext<'a> {
        GeneratorContext {
            turn_number: 1,
            is_main_agent,
            has_user_input: true,
            user_prompt: None,
            cwd: Path::new("/test"),
            agent_id: "test-agent",
            file_tracker,
            is_plan_mode: false,
            plan_file_path: None,
            is_plan_reentry: false,
            plan_state,
            background_tasks,
            critical_instruction: None,
            diagnostics_store: None,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            output_style: None,
            approved_plan: None,
        }
    }

    #[tokio::test]
    async fn test_returns_none_for_subagent() {
        let generator = AgentTaskGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let tasks = vec![BackgroundTaskInfo {
            task_id: "agent-1".to_string(),
            task_type: BackgroundTaskType::AsyncAgent,
            command: None,
            description: "Explore codebase".to_string(),
            status: BackgroundTaskStatus::Completed,
            exit_code: None,
            has_new_output: false,
            notified: false,
        }];
        let ctx = make_context(false, &tasks, &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_generates_for_completed_agent() {
        let generator = AgentTaskGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let tasks = vec![BackgroundTaskInfo {
            task_id: "agent-1".to_string(),
            task_type: BackgroundTaskType::AsyncAgent,
            command: None,
            description: "Explore codebase".to_string(),
            status: BackgroundTaskStatus::Completed,
            exit_code: None,
            has_new_output: false,
            notified: false,
        }];
        let ctx = make_context(true, &tasks, &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert_eq!(reminder.attachment_type, AttachmentType::AsyncAgentStatus);
        assert!(reminder.content.contains("agent-1"));
        assert!(reminder.content.contains("Explore codebase"));
        assert!(reminder.content.contains("completed"));
        assert!(reminder.content.contains("TaskOutput"));
    }

    #[tokio::test]
    async fn test_returns_none_for_running_agent() {
        let generator = AgentTaskGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let tasks = vec![BackgroundTaskInfo {
            task_id: "agent-1".to_string(),
            task_type: BackgroundTaskType::AsyncAgent,
            command: None,
            description: "Explore codebase".to_string(),
            status: BackgroundTaskStatus::Running,
            exit_code: None,
            has_new_output: false,
            notified: false,
        }];
        let ctx = make_context(true, &tasks, &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_returns_none_for_already_notified() {
        let generator = AgentTaskGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let tasks = vec![BackgroundTaskInfo {
            task_id: "agent-1".to_string(),
            task_type: BackgroundTaskType::AsyncAgent,
            command: None,
            description: "Explore codebase".to_string(),
            status: BackgroundTaskStatus::Completed,
            exit_code: None,
            has_new_output: false,
            notified: true, // Already notified
        }];
        let ctx = make_context(true, &tasks, &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_ignores_shell_tasks() {
        let generator = AgentTaskGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let tasks = vec![BackgroundTaskInfo {
            task_id: "shell-1".to_string(),
            task_type: BackgroundTaskType::Shell, // Shell, not AsyncAgent
            command: Some("npm test".to_string()),
            description: "Running tests".to_string(),
            status: BackgroundTaskStatus::Completed,
            exit_code: Some(0),
            has_new_output: false,
            notified: false,
        }];
        let ctx = make_context(true, &tasks, &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_uses_system_notification_tag() {
        let generator = AgentTaskGenerator::new();
        assert_eq!(
            generator.attachment_type(),
            AttachmentType::AsyncAgentStatus
        );
        // AsyncAgentStatus maps to XmlTag::SystemNotification
    }

    #[test]
    fn test_main_agent_only_tier() {
        let generator = AgentTaskGenerator::new();
        assert_eq!(generator.tier(), ReminderTier::MainAgentOnly);
    }
}
