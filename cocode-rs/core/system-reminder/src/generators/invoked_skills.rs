//! Invoked skills generator.
//!
//! Injects skill prompt content for skills invoked by the user.
//! This replaces the separate skill injection path with a unified
//! attachment-based system.

use async_trait::async_trait;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::types::AttachmentType;
use crate::types::ReminderTier;
use crate::types::SystemReminder;

/// Key for storing invoked skills in extension data.
pub const INVOKED_SKILLS_KEY: &str = "invoked_skills";

/// Information about an invoked skill.
#[derive(Debug, Clone)]
pub struct InvokedSkillInfo {
    /// Skill name (slash command identifier, e.g., "commit", "review-pr").
    pub name: String,
    /// The skill's prompt content (typically from SKILL.md or similar).
    pub prompt_content: String,
}

/// Generator for invoked skills.
///
/// Injects skill prompt content when a user invokes a skill via `/skill-name`.
/// The skill content is passed via extension_data using INVOKED_SKILLS_KEY.
#[derive(Debug)]
pub struct InvokedSkillsGenerator;

#[async_trait]
impl AttachmentGenerator for InvokedSkillsGenerator {
    fn name(&self) -> &str {
        "InvokedSkillsGenerator"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::InvokedSkills
    }

    fn tier(&self) -> ReminderTier {
        ReminderTier::UserPrompt
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.attachments.invoked_skills
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        // Get invoked skills from extension data
        let skills: Option<&Vec<InvokedSkillInfo>> = ctx
            .extension_data
            .get(INVOKED_SKILLS_KEY)
            .and_then(|v| v.downcast_ref());

        let skills = match skills {
            Some(s) if !s.is_empty() => s,
            _ => return Ok(None),
        };

        let mut content = String::new();

        for skill in skills.iter() {
            // Format: inject the skill's prompt content with a header
            content.push_str(&format!("<command-name>{}</command-name>\n", skill.name));
            content.push_str(&skill.prompt_content);
            content.push_str("\n\n");
        }

        if content.is_empty() {
            return Ok(None);
        }

        Ok(Some(SystemReminder::new(
            AttachmentType::InvokedSkills,
            content.trim(),
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
    async fn test_no_skills() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = InvokedSkillsGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_with_skill() {
        let config = test_config();
        let skills: Vec<InvokedSkillInfo> = vec![InvokedSkillInfo {
            name: "commit".to_string(),
            prompt_content: "Generate a commit message for the staged changes.".to_string(),
        }];

        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .extension(INVOKED_SKILLS_KEY, skills)
            .build();

        let generator = InvokedSkillsGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(
            reminder
                .content()
                .unwrap()
                .contains("<command-name>commit</command-name>")
        );
        assert!(
            reminder
                .content()
                .unwrap()
                .contains("Generate a commit message")
        );
    }

    #[tokio::test]
    async fn test_with_multiple_skills() {
        let config = test_config();
        let skills: Vec<InvokedSkillInfo> = vec![
            InvokedSkillInfo {
                name: "commit".to_string(),
                prompt_content: "Generate a commit message.".to_string(),
            },
            InvokedSkillInfo {
                name: "review-pr".to_string(),
                prompt_content: "Review the pull request.".to_string(),
            },
        ];

        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .is_main_agent(true)
            .cwd(PathBuf::from("/tmp"))
            .extension(INVOKED_SKILLS_KEY, skills)
            .build();

        let generator = InvokedSkillsGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert!(reminder.content().unwrap().contains("commit"));
        assert!(reminder.content().unwrap().contains("review-pr"));
    }

    #[test]
    fn test_generator_properties() {
        let generator = InvokedSkillsGenerator;
        assert_eq!(generator.name(), "InvokedSkillsGenerator");
        assert_eq!(generator.tier(), ReminderTier::UserPrompt);
        assert_eq!(generator.attachment_type(), AttachmentType::InvokedSkills);
    }
}
