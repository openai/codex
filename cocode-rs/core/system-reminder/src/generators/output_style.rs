//! Output style generator.
//!
//! Injects output style instructions to modify model response behavior.

use async_trait::async_trait;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::throttle::ThrottleConfig;
use crate::types::AttachmentType;
use crate::types::ReminderTier;
use crate::types::SystemReminder;

/// Generator for output style instructions.
///
/// This generator injects output style instructions that modify how the model
/// responds. It supports:
/// - Built-in styles: "explanatory" (educational insights) and "learning" (hands-on learning)
/// - Custom instruction text provided via configuration
///
/// The generator only runs for the main agent and uses high throttling (once per 50 turns)
/// since the style should remain consistent throughout a session.
#[derive(Debug)]
pub struct OutputStyleGenerator;

#[async_trait]
impl AttachmentGenerator for OutputStyleGenerator {
    fn name(&self) -> &str {
        "OutputStyleGenerator"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::OutputStyle
    }

    fn tier(&self) -> ReminderTier {
        ReminderTier::MainAgentOnly
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.attachments.output_style
            && config.output_style.enabled
            && config.output_style.resolve_instruction().is_some()
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // Output style injects once per session at the start,
        // consistent with Claude Code behavior
        ThrottleConfig::output_style()
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Resolve the output style instruction from config
        let instruction = match ctx.config.output_style.resolve_instruction() {
            Some(i) => i,
            None => return Ok(None),
        };

        Ok(Some(SystemReminder::new(
            AttachmentType::OutputStyle,
            instruction,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_config_with_style(
        style_name: Option<&str>,
        instruction: Option<&str>,
    ) -> SystemReminderConfig {
        let mut config = SystemReminderConfig::default();
        config.output_style.enabled = true;
        config.output_style.style_name = style_name.map(String::from);
        config.output_style.instruction = instruction.map(String::from);
        config
    }

    #[tokio::test]
    async fn test_disabled_when_output_style_disabled() {
        let mut config = SystemReminderConfig::default();
        config.output_style.enabled = false;
        config.output_style.style_name = Some("explanatory".to_string());

        let generator = OutputStyleGenerator;
        assert!(!generator.is_enabled(&config));
    }

    #[tokio::test]
    async fn test_disabled_when_attachment_disabled() {
        let mut config = test_config_with_style(Some("explanatory"), None);
        config.attachments.output_style = false;

        let generator = OutputStyleGenerator;
        assert!(!generator.is_enabled(&config));
    }

    #[tokio::test]
    async fn test_disabled_when_no_instruction() {
        let mut config = SystemReminderConfig::default();
        config.output_style.enabled = true;
        // No style_name or instruction set

        let generator = OutputStyleGenerator;
        assert!(!generator.is_enabled(&config));
    }

    #[tokio::test]
    async fn test_enabled_with_builtin_style() {
        let config = test_config_with_style(Some("explanatory"), None);

        let generator = OutputStyleGenerator;
        assert!(generator.is_enabled(&config));
    }

    #[tokio::test]
    async fn test_enabled_with_custom_instruction() {
        let config = test_config_with_style(None, Some("Be concise"));

        let generator = OutputStyleGenerator;
        assert!(generator.is_enabled(&config));
    }

    #[tokio::test]
    async fn test_generate_with_builtin_style() {
        let config = test_config_with_style(Some("explanatory"), None);
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = OutputStyleGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert_eq!(reminder.attachment_type, AttachmentType::OutputStyle);
        assert!(
            reminder
                .content()
                .unwrap()
                .contains("Explanatory Style Active")
        );
    }

    #[tokio::test]
    async fn test_generate_with_learning_style() {
        let config = test_config_with_style(Some("learning"), None);
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = OutputStyleGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(
            reminder
                .content()
                .unwrap()
                .contains("Learning Style Active")
        );
        assert!(reminder.content().unwrap().contains("TODO(human)"));
    }

    #[tokio::test]
    async fn test_generate_with_custom_instruction() {
        let config = test_config_with_style(None, Some("Always be brief and direct."));
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = OutputStyleGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert_eq!(reminder.content().unwrap(), "Always be brief and direct.");
    }

    #[tokio::test]
    async fn test_custom_instruction_takes_precedence() {
        // Both style_name and instruction set - instruction should win
        let config = test_config_with_style(Some("explanatory"), Some("My custom override"));
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = OutputStyleGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert_eq!(reminder.content().unwrap(), "My custom override");
        assert!(
            !reminder
                .content()
                .unwrap()
                .contains("Explanatory Style Active")
        );
    }

    #[test]
    fn test_generator_properties() {
        let generator = OutputStyleGenerator;
        assert_eq!(generator.name(), "OutputStyleGenerator");
        assert_eq!(generator.attachment_type(), AttachmentType::OutputStyle);
        assert_eq!(generator.tier(), ReminderTier::MainAgentOnly);

        // Output style injects once per session (max_per_session: 1)
        let throttle = generator.throttle_config();
        assert_eq!(throttle.min_turns_between, 0);
        assert_eq!(throttle.max_per_session, Some(1));
    }
}
