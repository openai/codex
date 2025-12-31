//! Critical instruction generator.
//!
//! Always-on user-defined instruction (P0).
//! Matches FH5() in Claude Code chunks.107.mjs:1910-1917.

use crate::config::system_reminder::SystemReminderConfig;
use crate::error::Result;
use crate::system_reminder::generator::AttachmentGenerator;
use crate::system_reminder::generator::GeneratorContext;
use crate::system_reminder::throttle::ThrottleConfig;
use crate::system_reminder::types::AttachmentType;
use crate::system_reminder::types::SystemReminder;
use async_trait::async_trait;

/// Critical instruction generator.
///
/// Always generates when configured, no throttling.
#[derive(Debug)]
pub struct CriticalInstructionGenerator;

impl CriticalInstructionGenerator {
    /// Create a new critical instruction generator.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CriticalInstructionGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AttachmentGenerator for CriticalInstructionGenerator {
    fn name(&self) -> &str {
        "critical_instruction"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::CriticalInstruction
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Only generate if critical instruction is configured
        if let Some(instruction) = ctx.critical_instruction {
            tracing::debug!(
                generator = "critical_instruction",
                "Generating critical instruction reminder"
            );
            Ok(Some(SystemReminder::new(
                AttachmentType::CriticalInstruction,
                instruction.to_string(),
            )))
        } else {
            Ok(None)
        }
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.enabled && config.attachments.critical_instruction
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttling for critical instructions
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
    use crate::system_reminder::types::ReminderTier;
    use std::path::Path;

    fn make_context<'a>(
        critical_instruction: Option<&'a str>,
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
            critical_instruction,
            diagnostics_store: None,
            lsp_diagnostics_min_severity: LspDiagnosticsMinSeverity::default(),
            output_style: None,
            approved_plan: None,
        }
    }

    #[tokio::test]
    async fn test_generates_when_configured() {
        let generator = CriticalInstructionGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let ctx = make_context(Some("Always run tests"), &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert_eq!(
            reminder.attachment_type,
            AttachmentType::CriticalInstruction
        );
        assert!(reminder.content.contains("Always run tests"));
    }

    #[tokio::test]
    async fn test_returns_none_when_not_configured() {
        let generator = CriticalInstructionGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let ctx = make_context(None, &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_no_throttling() {
        let generator = CriticalInstructionGenerator::new();
        let config = generator.throttle_config();
        assert_eq!(config.min_turns_between, 0);
        assert_eq!(config.min_turns_after_trigger, 0);
    }

    #[test]
    fn test_attachment_type() {
        let generator = CriticalInstructionGenerator::new();
        assert_eq!(
            generator.attachment_type(),
            AttachmentType::CriticalInstruction
        );
        assert_eq!(generator.tier(), ReminderTier::Core);
    }
}
