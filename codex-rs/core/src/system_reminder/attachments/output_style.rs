//! Output style generator.
//!
//! Injects active output style prompt into system reminders.
//! Matches KH5() in Claude Code chunks.107.mjs:1919-1926.
//! Tier: MainAgentOnly

use crate::config::system_reminder::SystemReminderConfig;
use crate::error::Result;
use crate::system_reminder::generator::AttachmentGenerator;
use crate::system_reminder::generator::GeneratorContext;
use crate::system_reminder::throttle::ThrottleConfig;
use crate::system_reminder::types::AttachmentType;
use crate::system_reminder::types::ReminderTier;
use crate::system_reminder::types::SystemReminder;
use async_trait::async_trait;

/// Output style generator.
///
/// Generates output style reminder for non-default styles.
#[derive(Debug)]
pub struct OutputStyleGenerator;

impl OutputStyleGenerator {
    /// Create a new output style generator.
    pub fn new() -> Self {
        Self
    }
}

impl Default for OutputStyleGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AttachmentGenerator for OutputStyleGenerator {
    fn name(&self) -> &str {
        "output_style"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::OutputStyle
    }

    fn tier(&self) -> ReminderTier {
        // Output style is MainAgentOnly - sub-agents don't get style prompts
        ReminderTier::MainAgentOnly
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Check if we have an active output style
        let current_style = ctx.output_style.as_ref();

        match current_style {
            Some(style) if !style.is_default() => {
                tracing::debug!(
                    generator = "output_style",
                    style = %style.name,
                    "Generating output style reminder"
                );

                // Build content: style name + instructions + full prompt if available
                let content = if let Some(ref prompt) = style.prompt {
                    format!(
                        "{} output style is active. Remember to follow the specific guidelines for this style.\n\n{}",
                        style.name, prompt
                    )
                } else {
                    format!(
                        "{} output style is active. Remember to follow the specific guidelines for this style.",
                        style.name
                    )
                };

                Ok(Some(SystemReminder::new(
                    AttachmentType::OutputStyle,
                    content,
                )))
            }
            _ => Ok(None),
        }
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.enabled && config.attachments.output_style
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttling - inject every turn when active
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
    use crate::config::output_style::OutputStyle;
    use crate::config::output_style::OutputStyleSource;
    use crate::config::system_reminder::LspDiagnosticsMinSeverity;
    use crate::system_reminder::file_tracker::FileTracker;
    use crate::system_reminder::generator::PlanState;
    use std::path::Path;

    fn make_style(name: &str, prompt: Option<&str>) -> OutputStyle {
        OutputStyle::new(
            name,
            "Test description",
            prompt.map(String::from),
            false,
            OutputStyleSource::BuiltIn,
        )
    }

    fn make_context<'a>(
        output_style: Option<&'a OutputStyle>,
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
            output_style,
            approved_plan: None,
        }
    }

    #[tokio::test]
    async fn test_returns_none_for_default_style() {
        let generator = OutputStyleGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let style = make_style("default", None);
        let ctx = make_context(Some(&style), &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_returns_none_when_no_style() {
        let generator = OutputStyleGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let ctx = make_context(None, &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_generates_for_explanatory_style() {
        let generator = OutputStyleGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let style = make_style("Explanatory", Some("Custom prompt content"));
        let ctx = make_context(Some(&style), &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert_eq!(reminder.attachment_type, AttachmentType::OutputStyle);
        assert!(
            reminder
                .content
                .contains("Explanatory output style is active")
        );
        assert!(reminder.content.contains("Custom prompt content"));
    }

    #[tokio::test]
    async fn test_generates_for_style_without_prompt() {
        let generator = OutputStyleGenerator::new();
        let tracker = FileTracker::new();
        let plan_state = PlanState::default();
        let style = make_style("Custom", None);
        let ctx = make_context(Some(&style), &tracker, &plan_state);

        let result = generator.generate(&ctx).await.unwrap();
        assert!(result.is_some());

        let reminder = result.unwrap();
        assert!(reminder.content.contains("Custom output style is active"));
        assert!(!reminder.content.contains("\n\n"));
    }

    #[test]
    fn test_tier_is_main_agent_only() {
        let generator = OutputStyleGenerator::new();
        assert_eq!(generator.tier(), ReminderTier::MainAgentOnly);
    }

    #[test]
    fn test_no_throttling() {
        let generator = OutputStyleGenerator::new();
        let config = generator.throttle_config();
        assert_eq!(config.min_turns_between, 0);
        assert_eq!(config.min_turns_after_trigger, 0);
        assert!(config.max_per_session.is_none());
    }

    #[test]
    fn test_attachment_type() {
        let generator = OutputStyleGenerator::new();
        assert_eq!(generator.attachment_type(), AttachmentType::OutputStyle);
    }
}
