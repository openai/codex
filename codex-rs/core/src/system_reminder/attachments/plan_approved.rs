//! Plan approved generator.
//!
//! One-time injection after ExitPlanMode approval.
//! Matches Claude Code's post-approval plan embedding behavior.

use crate::config::system_reminder::SystemReminderConfig;
use crate::error::Result;
use crate::system_reminder::generator::AttachmentGenerator;
use crate::system_reminder::generator::GeneratorContext;
use crate::system_reminder::throttle::ThrottleConfig;
use crate::system_reminder::types::AttachmentType;
use crate::system_reminder::types::SystemReminder;
use async_trait::async_trait;

/// Plan approved generator.
///
/// Generates a one-time reminder after plan approval containing:
/// - Full plan content
/// - "User has approved your plan. You can now start coding." instruction
///
/// Matches Claude Code's ExitPlanMode tool_result embedding.
#[derive(Debug)]
pub struct PlanApprovedGenerator;

impl PlanApprovedGenerator {
    /// Create a new plan approved generator.
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlanApprovedGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AttachmentGenerator for PlanApprovedGenerator {
    fn name(&self) -> &str {
        "plan_approved"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::PlanApproved
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Only generate if there's an approved plan pending injection
        let Some(plan) = &ctx.approved_plan else {
            return Ok(None);
        };

        let content = format!(
            "## Approved Plan\n\n\
             User has approved your plan. You can now start coding. \
             Start with updating your todo list if applicable.\n\n\
             **Plan file:** {}\n\n\
             {}\n\n\
             You can refer back to this plan file if needed during implementation.",
            plan.file_path, plan.content
        );

        tracing::info!(
            generator = "plan_approved",
            file_path = %plan.file_path,
            "Generating approved plan reminder"
        );

        Ok(Some(SystemReminder::new(
            AttachmentType::PlanApproved,
            content,
        )))
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        // Always enabled when system reminders are enabled
        // (no specific config for this generator)
        config.enabled
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttling - one-time injection controlled by approved_plan presence
        ThrottleConfig {
            min_turns_between: 0,
            min_turns_after_trigger: 0,
            max_per_session: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::system_reminder::LspDiagnosticsMinSeverity;
    use crate::system_reminder::file_tracker::FileTracker;
    use crate::system_reminder::generator::ApprovedPlanInfo;
    use crate::system_reminder::generator::PlanState;
    use crate::system_reminder::types::ReminderTier;
    use std::path::Path;

    fn make_context<'a>(
        approved_plan: Option<ApprovedPlanInfo>,
        file_tracker: &'a FileTracker,
        plan_state: &'a PlanState,
    ) -> GeneratorContext<'a> {
        GeneratorContext {
            turn_number: 1,
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
            approved_plan,
        }
    }

    #[tokio::test]
    async fn test_generates_when_plan_approved() {
        let generator = PlanApprovedGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();

        let approved_plan = ApprovedPlanInfo {
            content: "# My Plan\n\n1. Step one\n2. Step two".to_string(),
            file_path: "/path/to/plan.md".to_string(),
        };

        let ctx = make_context(Some(approved_plan), &tracker, &plan_state);
        let result = generator.generate(&ctx).await.unwrap();

        assert!(result.is_some());
        let reminder = result.unwrap();
        assert_eq!(reminder.attachment_type, AttachmentType::PlanApproved);
        assert!(reminder.content.contains("User has approved your plan"));
        assert!(reminder.content.contains("Step one"));
        assert!(reminder.content.contains("/path/to/plan.md"));
    }

    #[tokio::test]
    async fn test_returns_none_without_approved_plan() {
        let generator = PlanApprovedGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();

        let ctx = make_context(None, &tracker, &plan_state);
        let result = generator.generate(&ctx).await.unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn test_no_throttling() {
        let generator = PlanApprovedGenerator::new();
        let config = generator.throttle_config();
        assert_eq!(config.min_turns_between, 0);
        assert_eq!(config.min_turns_after_trigger, 0);
    }

    #[test]
    fn test_attachment_type() {
        let generator = PlanApprovedGenerator::new();
        assert_eq!(generator.attachment_type(), AttachmentType::PlanApproved);
        assert_eq!(generator.tier(), ReminderTier::Core);
    }
}
