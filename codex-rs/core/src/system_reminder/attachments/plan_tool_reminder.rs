//! Plan tool reminder generator.
//!
//! Periodic reminder about empty/stale plan (P0).
//! Reminds about using the update_plan tool.
//! Adapted from Claude Code's todo reminder (_H5() in chunks.107.mjs:2379-2394).

use crate::config::system_reminder::SystemReminderConfig;
use crate::error::Result;
use crate::system_reminder::generator::AttachmentGenerator;
use crate::system_reminder::generator::GeneratorContext;
use crate::system_reminder::generator::PlanState;
use crate::system_reminder::throttle::ThrottleConfig;
use crate::system_reminder::throttle::default_throttle_config;
use crate::system_reminder::types::AttachmentType;
use crate::system_reminder::types::SystemReminder;
use async_trait::async_trait;

/// Plan tool reminder generator.
///
/// Generates periodic reminders about using the update_plan tool.
/// Named "plan_tool_reminder" to distinguish from "plan_mode" (the 5-phase planning workflow).
#[derive(Debug)]
pub struct PlanToolReminderGenerator;

impl PlanToolReminderGenerator {
    /// Create a new plan tool reminder generator.
    pub fn new() -> Self {
        Self
    }

    /// Build reminder content matching Claude Code format.
    ///
    /// Two cases:
    /// - Empty plan: gentle reminder to create a plan
    /// - Stale plan: reminder that plan hasn't been updated, with current steps
    fn build_content(&self, plan_state: &PlanState) -> String {
        if plan_state.is_empty {
            // EMPTY PLAN CASE (matching CC lines 1145-1150)
            String::from(
                "This is a reminder that your plan is currently empty. DO NOT mention this \
                 to the user explicitly because they are already aware. If you are working on \
                 tasks that would benefit from a plan please use the update_plan tool to create \
                 one. If not, please feel free to ignore. Again do not mention this message \
                 to the user.",
            )
        } else {
            // STALE PLAN CASE (matching CC lines 1168-1171)
            let formatted_list: String = plan_state
                .steps
                .iter()
                .enumerate()
                .map(|(i, s)| format!("{}. [{}] {}", i + 1, s.status, s.step))
                .collect::<Vec<_>>()
                .join("\n");

            format!(
                "The update_plan tool hasn't been used recently. If you're working on tasks \
                 that would benefit from tracking progress, consider using the update_plan tool \
                 to track progress. Also consider cleaning up the plan if it has become stale \
                 and no longer matches what you are working on. Only use it if it's relevant \
                 to the current work. This is just a gentle reminder - ignore if not applicable. \
                 Make sure that you NEVER mention this reminder to the user\n\n\
                 Here are the existing steps in your plan:\n\n[{formatted_list}]"
            )
        }
    }
}

impl Default for PlanToolReminderGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AttachmentGenerator for PlanToolReminderGenerator {
    fn name(&self) -> &str {
        "plan_tool_reminder"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::PlanToolReminder
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Additional trigger check: min calls since last update_plan
        // turn_number here is actually inject_call_count
        let calls_since_update = ctx
            .turn_number
            .saturating_sub(ctx.plan_state.last_update_count);
        if calls_since_update < 5 {
            // GY2.TURNS_SINCE_UPDATE (5 calls minimum)
            return Ok(None);
        }

        tracing::info!(
            generator = "plan_tool_reminder",
            calls_since_update = calls_since_update,
            step_count = ctx.plan_state.steps.len(),
            "Generating plan tool reminder"
        );
        Ok(Some(SystemReminder::new(
            AttachmentType::PlanToolReminder,
            self.build_content(ctx.plan_state),
        )))
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.enabled && config.attachments.plan_tool_reminder
    }

    fn throttle_config(&self) -> ThrottleConfig {
        default_throttle_config(AttachmentType::PlanToolReminder)
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
    use crate::system_reminder::generator::PlanStep;
    use crate::system_reminder::types::ReminderTier;
    use std::path::Path;

    fn make_context<'a>(
        turn_number: i32,
        plan_state: &'a PlanState,
        file_tracker: &'a FileTracker,
    ) -> GeneratorContext<'a> {
        GeneratorContext {
            turn_number,
            is_main_agent: true,
            has_user_input: true,
            user_prompt: None,
            cwd: Path::new("/test"),
            agent_id: "test-agent",
            file_tracker,
            is_plan_mode: false,
            plan_file_path: None,
            is_plan_reentry: false,
            plan_state,
            background_tasks: &[],
            critical_instruction: None,
            diagnostics_store: None,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            output_style: None,
            approved_plan: None,
        }
    }

    #[tokio::test]
    async fn test_generates_after_turns_since_update() {
        let generator = PlanToolReminderGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState {
            is_empty: true,
            last_update_count: 1,
            steps: vec![],
        };
        // Turn 7: 7 - 1 = 6 >= 5
        let ctx = make_context(7, &plan_state, &tracker);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert_eq!(reminder.attachment_type, AttachmentType::PlanToolReminder);
        assert!(reminder.content.contains("update_plan tool"));
    }

    #[tokio::test]
    async fn test_returns_none_when_recent_update() {
        let generator = PlanToolReminderGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState {
            is_empty: true,
            last_update_count: 3,
            steps: vec![],
        };
        // Turn 5: 5 - 3 = 2 < 5
        let ctx = make_context(5, &plan_state, &tracker);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_empty_plan_message() {
        let generator = PlanToolReminderGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState {
            is_empty: true,
            last_update_count: 1,
            steps: vec![],
        };
        let ctx = make_context(10, &plan_state, &tracker);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert!(reminder.content.contains("plan is currently empty"));
        assert!(reminder.content.contains("DO NOT mention this"));
    }

    #[tokio::test]
    async fn test_stale_plan_includes_steps() {
        let generator = PlanToolReminderGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState {
            is_empty: false,
            last_update_count: 1,
            steps: vec![
                PlanStep {
                    step: "First step".to_string(),
                    status: "pending".to_string(),
                },
                PlanStep {
                    step: "Second step".to_string(),
                    status: "completed".to_string(),
                },
            ],
        };
        let ctx = make_context(10, &plan_state, &tracker);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert!(reminder.content.contains("First step"));
        assert!(reminder.content.contains("Second step"));
        assert!(reminder.content.contains("[pending]"));
        assert!(reminder.content.contains("[completed]"));
        assert!(reminder.content.contains("hasn't been used recently"));
    }

    #[test]
    fn test_throttle_config() {
        let generator = PlanToolReminderGenerator::new();
        let config = generator.throttle_config();
        assert_eq!(config.min_turns_between, 3);
        assert_eq!(config.min_turns_after_trigger, 5);
    }

    #[test]
    fn test_attachment_type() {
        let generator = PlanToolReminderGenerator::new();
        assert_eq!(
            generator.attachment_type(),
            AttachmentType::PlanToolReminder
        );
        assert_eq!(generator.tier(), ReminderTier::Core);
    }
}
