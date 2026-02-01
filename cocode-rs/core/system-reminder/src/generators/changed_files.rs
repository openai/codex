//! Changed files generator.
//!
//! Detects and reports files that have been modified since they were last read.

use async_trait::async_trait;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::throttle::ThrottleConfig;
use crate::types::AttachmentType;
use crate::types::SystemReminder;

/// Generator for detecting changed files.
#[derive(Debug)]
pub struct ChangedFilesGenerator;

#[async_trait]
impl AttachmentGenerator for ChangedFilesGenerator {
    fn name(&self) -> &str {
        "ChangedFilesGenerator"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::ChangedFiles
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.attachments.changed_files
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // No throttle - always check for changes
        ThrottleConfig::none()
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        let Some(tracker) = ctx.file_tracker else {
            return Ok(None);
        };

        let changed = tracker.changed_files();

        if changed.is_empty() {
            return Ok(None);
        }

        // Format the changed files message
        let mut content = String::new();
        content.push_str("The following files have been modified since you last read them:\n\n");

        for path in &changed {
            let display_path = path.display();
            content.push_str(&format!("- {display_path}\n"));
        }

        content.push_str("\nYou may want to re-read these files before making changes to ensure you have the latest content.");

        Ok(Some(SystemReminder::new(
            AttachmentType::ChangedFiles,
            content,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_tracker::FileTracker;
    use std::path::PathBuf;

    fn test_config() -> SystemReminderConfig {
        SystemReminderConfig::default()
    }

    #[tokio::test]
    async fn test_no_tracker() {
        let config = test_config();
        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .build();

        let generator = ChangedFilesGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_no_changes() {
        let config = test_config();
        let tracker = FileTracker::new();

        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .file_tracker(&tracker)
            .build();

        let generator = ChangedFilesGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[test]
    fn test_generator_properties() {
        let generator = ChangedFilesGenerator;
        assert_eq!(generator.name(), "ChangedFilesGenerator");
        assert_eq!(generator.attachment_type(), AttachmentType::ChangedFiles);

        let config = test_config();
        assert!(generator.is_enabled(&config));

        // No throttle
        let throttle = generator.throttle_config();
        assert_eq!(throttle.min_turns_between, 0);
    }
}
