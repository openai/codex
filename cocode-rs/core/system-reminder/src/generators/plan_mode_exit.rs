//! Plan mode exit generator.
//!
//! This generator provides one-time instructions when exiting plan mode
//! after the user approves the plan.

use async_trait::async_trait;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::throttle::ThrottleConfig;
use crate::types::AttachmentType;
use crate::types::SystemReminder;

/// Generator for plan mode exit instructions.
///
/// Provides one-time instructions when the plan has been approved
/// and the agent is transitioning out of plan mode to implementation.
#[derive(Debug)]
pub struct PlanModeExitGenerator;

#[async_trait]
impl AttachmentGenerator for PlanModeExitGenerator {
    fn name(&self) -> &str {
        "PlanModeExitGenerator"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::PlanModeExit
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.attachments.plan_mode_exit
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttle - this is a one-time injection
        ThrottleConfig::none()
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Only trigger when plan mode exit is pending
        if !ctx.plan_mode_exit_pending {
            return Ok(None);
        }

        // Must have an approved plan
        let Some(approved) = &ctx.approved_plan else {
            return Ok(None);
        };

        let content = format!(
            "{}\n\n## Your Approved Plan\n\n{}",
            PLAN_MODE_EXIT_INSTRUCTIONS, approved.content
        );

        Ok(Some(SystemReminder::new(
            AttachmentType::PlanModeExit,
            content,
        )))
    }
}

/// Instructions for transitioning out of plan mode.
const PLAN_MODE_EXIT_INSTRUCTIONS: &str = r#"## Plan Approved - Begin Implementation

The user has approved your plan. You are now exiting plan mode.

**Important:**
- You now have full access to all tools including Edit, Write, and Bash
- Follow your plan step by step
- Keep the user informed of your progress
- If you encounter issues not covered by the plan, explain what you're doing differently and why
- After completing each major step, briefly summarize what was done

Begin implementing your plan now."#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::ApprovedPlanInfo;
    use std::path::PathBuf;

    fn test_config() -> SystemReminderConfig {
        SystemReminderConfig::default()
    }

    #[tokio::test]
    async fn test_not_triggered_without_pending_flag() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .plan_mode_exit_pending(false)
            .approved_plan(ApprovedPlanInfo {
                content: "Step 1: Do something".to_string(),
                approved_turn: 5,
            })
            .build();

        let generator = PlanModeExitGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_not_triggered_without_approved_plan() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .plan_mode_exit_pending(true)
            // No approved_plan
            .build();

        let generator = PlanModeExitGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_triggered_with_pending_and_approved() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .plan_mode_exit_pending(true)
            .approved_plan(ApprovedPlanInfo {
                content: "Step 1: Implement feature X\nStep 2: Add tests".to_string(),
                approved_turn: 5,
            })
            .build();

        let generator = PlanModeExitGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content().unwrap().contains("Plan Approved"));
        assert!(reminder.content().unwrap().contains("Begin Implementation"));
        assert!(
            reminder
                .content()
                .unwrap()
                .contains("Step 1: Implement feature X")
        );
    }

    #[test]
    fn test_throttle_config() {
        let generator = PlanModeExitGenerator;
        let throttle = generator.throttle_config();
        assert_eq!(throttle.min_turns_between, 0);
    }
}
