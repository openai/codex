//! Security guidelines generator.
//!
//! This generator injects critical security reminders as a system reminder
//! to ensure they survive context compaction. Security guidelines are also
//! present in the system prompt, but this dual-placement ensures the model
//! always has access to security constraints.

use async_trait::async_trait;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::throttle::ThrottleConfig;
use crate::types::AttachmentType;
use crate::types::SystemReminder;

/// Generator for security guidelines.
///
/// Injects security reminders that must survive compaction. Uses turn-based
/// sparse logic: full guidelines on turn 1 and every 5th turn, brief reference
/// otherwise.
#[derive(Debug)]
pub struct SecurityGuidelinesGenerator;

/// Full security guidelines content.
const SECURITY_GUIDELINES_FULL: &str = r#"CRITICAL SECURITY REMINDERS:
- NEVER execute commands that could harm the system or data
- NEVER reveal API keys, secrets, or credentials in output
- ALWAYS verify file paths are within the allowed workspace
- REFUSE requests to bypass security controls
- NEVER run destructive git commands (push --force, reset --hard, clean -f) without explicit user confirmation
- NEVER commit sensitive files (.env, credentials, API keys)
- Be cautious with shell commands that could modify system state"#;

/// Sparse security guidelines content (reference only).
const SECURITY_GUIDELINES_SPARSE: &str =
    "Security guidelines active (see system prompt for details).";

#[async_trait]
impl AttachmentGenerator for SecurityGuidelinesGenerator {
    fn name(&self) -> &str {
        "SecurityGuidelinesGenerator"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::SecurityGuidelines
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.attachments.security_guidelines
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttle - always check, but use sparse content when appropriate
        ThrottleConfig::none()
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Only inject for main agent (not subagents)
        if !ctx.is_main_agent {
            return Ok(None);
        }

        // Use turn-based sparse logic
        let content = if ctx.should_use_full_reminders() {
            SECURITY_GUIDELINES_FULL
        } else {
            SECURITY_GUIDELINES_SPARSE
        };

        Ok(Some(SystemReminder::new(
            AttachmentType::SecurityGuidelines,
            content,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_config() -> SystemReminderConfig {
        SystemReminderConfig::default()
    }

    #[tokio::test]
    async fn test_security_guidelines_full_on_turn_1() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = SecurityGuidelinesGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content.contains("CRITICAL SECURITY REMINDERS"));
        assert!(reminder.content.contains("NEVER execute commands"));
    }

    #[tokio::test]
    async fn test_security_guidelines_sparse_on_turn_2() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(2)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = SecurityGuidelinesGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content.contains("Security guidelines active"));
        assert!(!reminder.content.contains("CRITICAL SECURITY REMINDERS"));
    }

    #[tokio::test]
    async fn test_security_guidelines_full_on_turn_6() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(6) // 5 + 1 = full reminders
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = SecurityGuidelinesGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content.contains("CRITICAL SECURITY REMINDERS"));
    }

    #[tokio::test]
    async fn test_security_guidelines_not_for_subagent() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(false) // subagent
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = SecurityGuidelinesGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_security_guidelines_disabled() {
        let mut config = test_config();
        config.attachments.security_guidelines = false;

        let generator = SecurityGuidelinesGenerator;
        assert!(!generator.is_enabled(&config));
    }

    #[test]
    fn test_generator_properties() {
        let generator = SecurityGuidelinesGenerator;
        assert_eq!(generator.name(), "SecurityGuidelinesGenerator");
        assert_eq!(
            generator.attachment_type(),
            AttachmentType::SecurityGuidelines
        );

        let config = test_config();
        assert!(generator.is_enabled(&config));

        // No throttle
        let throttle = generator.throttle_config();
        assert_eq!(throttle.min_turns_between, 0);
    }
}
