//! Already read files generator.
//!
//! This generator creates synthetic tool_use/tool_result pairs for files
//! that have been previously read. This helps the model know what files
//! it has already seen without including full content.

use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use crate::Result;
use crate::config::SystemReminderConfig;
use crate::generator::AttachmentGenerator;
use crate::generator::GeneratorContext;
use crate::throttle::ThrottleConfig;
use crate::types::AttachmentType;
use crate::types::ContentBlock;
use crate::types::MessageRole;
use crate::types::ReminderMessage;
use crate::types::SystemReminder;

/// Maximum number of files to include in the reminder.
const MAX_FILES_TO_INCLUDE: usize = 10;

/// Generator for already read files.
///
/// Creates synthetic tool_use/tool_result pairs for files the model has
/// previously read. This allows the model to know which files it has seen
/// without needing to include full file contents.
#[derive(Debug)]
pub struct AlreadyReadFilesGenerator;

#[async_trait]
impl AttachmentGenerator for AlreadyReadFilesGenerator {
    fn name(&self) -> &str {
        "AlreadyReadFilesGenerator"
    }

    fn attachment_type(&self) -> AttachmentType {
        AttachmentType::AlreadyReadFile
    }

    fn is_enabled(&self, config: &SystemReminderConfig) -> bool {
        config.attachments.already_read_files
    }

    fn throttle_config(&self) -> ThrottleConfig {
        // Only inject on first turn or every 5th turn (aligned with full reminders)
        ThrottleConfig {
            min_turns_between: 5,
            min_turns_after_trigger: 0,
            max_per_session: None,
        }
    }

    async fn generate(&self, ctx: &GeneratorContext<'_>) -> Result<Option<SystemReminder>> {
        let Some(tracker) = ctx.file_tracker else {
            return Ok(None);
        };

        let tracked_files = tracker.tracked_files();
        if tracked_files.is_empty() {
            return Ok(None);
        }

        // Build tool_use/tool_result pairs for each tracked file
        let mut messages = Vec::new();

        for path in tracked_files.iter().take(MAX_FILES_TO_INCLUDE) {
            let Some(state) = tracker.get_state(path) else {
                continue;
            };

            let id = format!("synth-read-{}", Uuid::new_v4());
            let path_str = path.display().to_string();

            // Create tool_use message (assistant role)
            let tool_use_block =
                ContentBlock::tool_use(id.clone(), "Read", json!({ "file_path": path_str }));
            messages.push(ReminderMessage {
                role: MessageRole::Assistant,
                blocks: vec![tool_use_block],
                is_meta: true,
            });

            // Create tool_result message (user role)
            let summary = if state.is_partial() {
                let offset = state.offset.unwrap_or(0);
                let limit = state.limit.unwrap_or(0);
                format!(
                    "[Previously read (partial): lines {}–{}, {} bytes]",
                    offset,
                    offset + limit,
                    state.content.len()
                )
            } else {
                format!(
                    "[Previously read: {} lines, {} bytes]",
                    state.content.lines().count(),
                    state.content.len()
                )
            };

            let tool_result_block = ContentBlock::tool_result(id, summary);
            messages.push(ReminderMessage {
                role: MessageRole::User,
                blocks: vec![tool_result_block],
                is_meta: true,
            });
        }

        // Add ellipsis if more files were tracked
        if tracked_files.len() > MAX_FILES_TO_INCLUDE {
            let remaining = tracked_files.len() - MAX_FILES_TO_INCLUDE;
            let id = format!("synth-note-{}", Uuid::new_v4());

            messages.push(ReminderMessage {
                role: MessageRole::Assistant,
                blocks: vec![ContentBlock::tool_use(
                    id.clone(),
                    "Read",
                    json!({ "note": format!("...and {} more files", remaining) }),
                )],
                is_meta: true,
            });

            messages.push(ReminderMessage {
                role: MessageRole::User,
                blocks: vec![ContentBlock::tool_result(
                    id,
                    format!("[{} additional files previously read]", remaining),
                )],
                is_meta: true,
            });
        }

        if messages.is_empty() {
            return Ok(None);
        }

        Ok(Some(SystemReminder::messages(
            AttachmentType::AlreadyReadFile,
            messages,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SystemReminderConfig;
    use crate::file_tracker::FileTracker;
    use crate::file_tracker::ReadFileState;
    use crate::types::ReminderTier;
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

        let generator = AlreadyReadFilesGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_empty_tracker() {
        let config = test_config();
        let tracker = FileTracker::new();

        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .file_tracker(&tracker)
            .build();

        let generator = AlreadyReadFilesGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_generates_tool_pairs() {
        let config = test_config();
        let tracker = FileTracker::new();

        // Track a file read
        let state = ReadFileState::new("fn main() {}\n".to_string(), None, 1);
        tracker.track_read("/project/src/main.rs", state);

        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .file_tracker(&tracker)
            .build();

        let generator = AlreadyReadFilesGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        assert_eq!(reminder.attachment_type, AttachmentType::AlreadyReadFile);
        assert!(reminder.is_messages());

        let messages = reminder.output.as_messages().unwrap();
        assert_eq!(messages.len(), 2); // One tool_use + one tool_result

        // Check assistant message with tool_use
        assert_eq!(messages[0].role, MessageRole::Assistant);
        assert!(matches!(
            &messages[0].blocks[0],
            ContentBlock::ToolUse { name, .. } if name == "Read"
        ));

        // Check user message with tool_result
        assert_eq!(messages[1].role, MessageRole::User);
        assert!(matches!(
            &messages[1].blocks[0],
            ContentBlock::ToolResult { content, .. } if content.contains("Previously read")
        ));
    }

    #[tokio::test]
    async fn test_partial_read_summary() {
        let config = test_config();
        let tracker = FileTracker::new();

        // Track a partial file read
        let state = ReadFileState::partial("partial content".to_string(), None, 1, 10, 50);
        tracker.track_read("/project/large.rs", state);

        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .file_tracker(&tracker)
            .build();

        let generator = AlreadyReadFilesGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        let messages = reminder.output.as_messages().unwrap();

        // Check that partial read is indicated
        if let ContentBlock::ToolResult { content, .. } = &messages[1].blocks[0] {
            assert!(content.contains("partial"));
        } else {
            panic!("Expected ToolResult");
        }
    }

    #[tokio::test]
    async fn test_multiple_files() {
        let config = test_config();
        let tracker = FileTracker::new();

        // Track multiple files
        tracker.track_read(
            "/project/a.rs",
            ReadFileState::new("a".to_string(), None, 1),
        );
        tracker.track_read(
            "/project/b.rs",
            ReadFileState::new("b".to_string(), None, 1),
        );
        tracker.track_read(
            "/project/c.rs",
            ReadFileState::new("c".to_string(), None, 1),
        );

        let ctx = GeneratorContext::builder()
            .config(&config)
            .turn_number(1)
            .cwd(PathBuf::from("/tmp"))
            .file_tracker(&tracker)
            .build();

        let generator = AlreadyReadFilesGenerator;
        let result = generator.generate(&ctx).await.expect("generate");
        assert!(result.is_some());

        let reminder = result.expect("reminder");
        let messages = reminder.output.as_messages().unwrap();
        // 3 files × 2 messages each = 6 messages
        assert_eq!(messages.len(), 6);
    }

    #[test]
    fn test_generator_properties() {
        let generator = AlreadyReadFilesGenerator;
        assert_eq!(generator.name(), "AlreadyReadFilesGenerator");
        assert_eq!(generator.attachment_type(), AttachmentType::AlreadyReadFile);
        assert_eq!(generator.tier(), ReminderTier::MainAgentOnly);

        let throttle = generator.throttle_config();
        assert_eq!(throttle.min_turns_between, 5);
    }
}
