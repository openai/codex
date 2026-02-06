//! Message injection for system reminders.
//!
//! This module provides utilities for injecting system reminders
//! into the message history.

use tracing::debug;

use crate::types::ContentBlock;
use crate::types::MessageRole;
use crate::types::ReminderOutput;
use crate::types::SystemReminder;
use crate::xml::wrap_with_tag;

/// Injection position for system reminders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionPosition {
    /// Before the user's message.
    BeforeUserMessage,
    /// After the user's message.
    AfterUserMessage,
    /// At the end of the conversation.
    EndOfConversation,
}

/// Result of injecting reminders.
#[derive(Debug)]
pub struct InjectionResult {
    /// Number of reminders injected.
    pub count: i32,
    /// Position where reminders were injected.
    pub position: InjectionPosition,
}

// ============================================================================
// Injected Message Types for Multi-Message Injection
// ============================================================================

/// Injected message - unified representation of all reminder output types.
///
/// This enum represents messages that should be injected into the conversation,
/// supporting both simple text reminders and multi-message tool_use/tool_result pairs.
#[derive(Debug, Clone)]
pub enum InjectedMessage {
    /// User text message (wrapped in XML tags).
    UserText {
        /// The wrapped content.
        content: String,
        /// Whether this is metadata (hidden from user, visible to model).
        is_meta: bool,
    },
    /// Assistant message containing content blocks (typically tool_use).
    AssistantBlocks {
        /// Content blocks (text, tool_use).
        blocks: Vec<InjectedBlock>,
        /// Whether this is metadata.
        is_meta: bool,
    },
    /// User message containing content blocks (typically tool_result).
    UserBlocks {
        /// Content blocks (text, tool_result).
        blocks: Vec<InjectedBlock>,
        /// Whether this is metadata.
        is_meta: bool,
    },
}

/// Injected content block.
///
/// Represents a single content block within an injected message.
#[derive(Debug, Clone)]
pub enum InjectedBlock {
    /// Plain text content.
    Text(String),
    /// Tool use block (synthetic tool call).
    ToolUse {
        /// Unique ID for the tool call.
        id: String,
        /// Name of the tool.
        name: String,
        /// Tool input parameters.
        input: serde_json::Value,
    },
    /// Tool result block.
    ToolResult {
        /// ID of the tool_use this is responding to.
        tool_use_id: String,
        /// Result content.
        content: String,
    },
}

/// Create injected messages from system reminders.
///
/// This function converts `SystemReminder` outputs into a unified `InjectedMessage`
/// representation that can be converted to API messages by the driver.
///
/// - `ReminderOutput::Text` becomes `InjectedMessage::UserText` with XML wrapping
/// - `ReminderOutput::Messages` becomes multiple `InjectedMessage::AssistantBlocks`
///   and `InjectedMessage::UserBlocks` preserving the tool_use/tool_result structure
///
/// # Arguments
///
/// * `reminders` - The reminders to convert
///
/// # Returns
///
/// A vector of injected messages ready for conversion to API messages.
pub fn create_injected_messages(reminders: Vec<SystemReminder>) -> Vec<InjectedMessage> {
    let mut result = Vec::new();

    for reminder in reminders {
        // Extract fields before consuming output
        let xml_tag = reminder.xml_tag();
        let attachment_type = reminder.attachment_type;
        let is_meta = reminder.is_meta;

        match reminder.output {
            ReminderOutput::Text(content) => {
                let wrapped = wrap_with_tag(&content, xml_tag);
                debug!(
                    "Creating UserText injection for {} ({} bytes)",
                    attachment_type,
                    wrapped.len()
                );
                result.push(InjectedMessage::UserText {
                    content: wrapped,
                    is_meta,
                });
            }
            ReminderOutput::Messages(msgs) => {
                debug!(
                    "Creating {} message injections for {}",
                    msgs.len(),
                    attachment_type
                );
                for msg in msgs {
                    let blocks: Vec<InjectedBlock> = msg
                        .blocks
                        .into_iter()
                        .map(|b| match b {
                            ContentBlock::Text { text } => InjectedBlock::Text(text),
                            ContentBlock::ToolUse { id, name, input } => {
                                InjectedBlock::ToolUse { id, name, input }
                            }
                            ContentBlock::ToolResult {
                                tool_use_id,
                                content,
                            } => InjectedBlock::ToolResult {
                                tool_use_id,
                                content,
                            },
                        })
                        .collect();

                    match msg.role {
                        MessageRole::Assistant => {
                            result.push(InjectedMessage::AssistantBlocks {
                                blocks,
                                is_meta: msg.is_meta,
                            });
                        }
                        MessageRole::User => {
                            result.push(InjectedMessage::UserBlocks {
                                blocks,
                                is_meta: msg.is_meta,
                            });
                        }
                    }
                }
            }
        }
    }

    result
}

/// Inject text-based system reminders and return wrapped content.
///
/// This is a simple helper that wraps each text reminder in its XML tags
/// and returns them as a list of strings ready to be converted to messages.
/// Multi-message reminders are skipped (they should be handled separately).
///
/// # Arguments
///
/// * `reminders` - The reminders to inject
///
/// # Returns
///
/// A vector of wrapped reminder content strings for text reminders only.
pub fn inject_reminders(reminders: Vec<SystemReminder>) -> Vec<String> {
    let mut result = Vec::with_capacity(reminders.len());

    for reminder in reminders {
        if let Some(wrapped) = reminder.wrapped_content() {
            debug!(
                "Injecting {} reminder ({} bytes)",
                reminder.attachment_type,
                wrapped.len()
            );
            result.push(wrapped);
        } else {
            debug!(
                "Skipping {} reminder (multi-message type)",
                reminder.attachment_type
            );
        }
    }

    result
}

/// Combine multiple text reminders into a single message.
///
/// This is useful when you want to inject all reminders as a single
/// user message rather than multiple messages. Multi-message reminders
/// are skipped.
pub fn combine_reminders(reminders: Vec<SystemReminder>) -> Option<String> {
    if reminders.is_empty() {
        return None;
    }

    let parts: Vec<String> = reminders
        .iter()
        .filter_map(|r| r.wrapped_content())
        .collect();

    if parts.is_empty() {
        return None;
    }

    Some(parts.join("\n\n"))
}

/// Information about injected reminders for logging/telemetry.
#[derive(Debug, Default)]
pub struct InjectionStats {
    /// Total number of reminders injected.
    pub total_count: i32,
    /// Total byte size of all text reminders.
    pub total_bytes: i64,
    /// Breakdown by attachment type.
    pub by_type: std::collections::HashMap<String, i32>,
    /// Number of multi-message reminders.
    pub multi_message_count: i32,
}

impl InjectionStats {
    /// Create stats from a list of reminders.
    pub fn from_reminders(reminders: &[SystemReminder]) -> Self {
        let mut stats = Self::default();

        for reminder in reminders {
            stats.total_count += 1;
            match &reminder.output {
                ReminderOutput::Text(content) => {
                    stats.total_bytes += content.len() as i64;
                }
                ReminderOutput::Messages(msgs) => {
                    stats.multi_message_count += 1;
                    // Estimate size for multi-message reminders
                    for msg in msgs {
                        for block in &msg.blocks {
                            if let crate::types::ContentBlock::Text { text } = block {
                                stats.total_bytes += text.len() as i64;
                            }
                        }
                    }
                }
            }
            *stats
                .by_type
                .entry(reminder.attachment_type.name().to_string())
                .or_default() += 1;
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AttachmentType;
    use crate::types::ContentBlock;
    use crate::types::ReminderMessage;

    fn test_reminder(content: &str) -> SystemReminder {
        SystemReminder::new(AttachmentType::ChangedFiles, content)
    }

    #[test]
    fn test_inject_reminders() {
        let reminders = vec![
            test_reminder("File a.rs changed"),
            test_reminder("File b.rs changed"),
        ];

        let injected = inject_reminders(reminders);
        assert_eq!(injected.len(), 2);
        assert!(injected[0].contains("<system-reminder>"));
        assert!(injected[0].contains("File a.rs changed"));
    }

    #[test]
    fn test_inject_empty() {
        let injected = inject_reminders(vec![]);
        assert!(injected.is_empty());
    }

    #[test]
    fn test_inject_skips_multi_message() {
        let messages = vec![
            ReminderMessage::assistant(vec![ContentBlock::tool_use(
                "test-id",
                "Read",
                serde_json::json!({}),
            )]),
            ReminderMessage::user(vec![ContentBlock::tool_result("test-id", "content")]),
        ];
        let reminders = vec![
            test_reminder("Text reminder"),
            SystemReminder::messages(AttachmentType::AlreadyReadFile, messages),
        ];

        let injected = inject_reminders(reminders);
        // Only the text reminder should be included
        assert_eq!(injected.len(), 1);
        assert!(injected[0].contains("Text reminder"));
    }

    #[test]
    fn test_combine_reminders() {
        let reminders = vec![
            test_reminder("First reminder"),
            test_reminder("Second reminder"),
        ];

        let combined = combine_reminders(reminders);
        assert!(combined.is_some());

        let content = combined.expect("content");
        assert!(content.contains("First reminder"));
        assert!(content.contains("Second reminder"));
        assert!(content.contains("\n\n")); // Separated by double newline
    }

    #[test]
    fn test_combine_empty() {
        let combined = combine_reminders(vec![]);
        assert!(combined.is_none());
    }

    #[test]
    fn test_combine_only_multi_message() {
        let messages = vec![ReminderMessage::assistant(vec![ContentBlock::tool_use(
            "test-id",
            "Read",
            serde_json::json!({}),
        )])];
        let reminders = vec![SystemReminder::messages(
            AttachmentType::AlreadyReadFile,
            messages,
        )];

        let combined = combine_reminders(reminders);
        assert!(combined.is_none());
    }

    #[test]
    fn test_injection_stats() {
        let reminders = vec![
            SystemReminder::new(AttachmentType::ChangedFiles, "change 1"),
            SystemReminder::new(AttachmentType::ChangedFiles, "change 2"),
            SystemReminder::new(AttachmentType::PlanModeEnter, "plan instructions"),
        ];

        let stats = InjectionStats::from_reminders(&reminders);
        assert_eq!(stats.total_count, 3);
        assert_eq!(stats.by_type.get("changed_files"), Some(&2));
        assert_eq!(stats.by_type.get("plan_mode_enter"), Some(&1));
        assert_eq!(stats.multi_message_count, 0);
    }

    #[test]
    fn test_injection_stats_with_multi_message() {
        let messages = vec![
            ReminderMessage::assistant(vec![ContentBlock::tool_use(
                "test-id",
                "Read",
                serde_json::json!({}),
            )]),
            ReminderMessage::user(vec![ContentBlock::tool_result("test-id", "file content")]),
        ];
        let reminders = vec![
            SystemReminder::new(AttachmentType::ChangedFiles, "change 1"),
            SystemReminder::messages(AttachmentType::AlreadyReadFile, messages),
        ];

        let stats = InjectionStats::from_reminders(&reminders);
        assert_eq!(stats.total_count, 2);
        assert_eq!(stats.multi_message_count, 1);
        assert_eq!(stats.by_type.get("already_read_file"), Some(&1));
    }

    // ======== Tests for create_injected_messages ========

    #[test]
    fn test_create_injected_messages_text() {
        let reminders = vec![
            test_reminder("File changed"),
            SystemReminder::new(AttachmentType::PlanModeEnter, "Plan mode active"),
        ];

        let injected = create_injected_messages(reminders);
        assert_eq!(injected.len(), 2);

        // Check first message
        match &injected[0] {
            InjectedMessage::UserText { content, is_meta } => {
                assert!(content.contains("<system-reminder>"));
                assert!(content.contains("File changed"));
                assert!(*is_meta);
            }
            _ => panic!("Expected UserText"),
        }
    }

    #[test]
    fn test_create_injected_messages_multi_message() {
        let messages = vec![
            ReminderMessage::assistant(vec![ContentBlock::tool_use(
                "test-id",
                "Read",
                serde_json::json!({"file_path": "/test.rs"}),
            )]),
            ReminderMessage::user(vec![ContentBlock::tool_result(
                "test-id",
                "[Previously read: 10 lines]",
            )]),
        ];
        let reminders = vec![SystemReminder::messages(
            AttachmentType::AlreadyReadFile,
            messages,
        )];

        let injected = create_injected_messages(reminders);
        assert_eq!(injected.len(), 2);

        // Check assistant message with tool_use
        match &injected[0] {
            InjectedMessage::AssistantBlocks { blocks, is_meta } => {
                assert_eq!(blocks.len(), 1);
                assert!(*is_meta);
                match &blocks[0] {
                    InjectedBlock::ToolUse { id, name, input } => {
                        assert_eq!(id, "test-id");
                        assert_eq!(name, "Read");
                        assert_eq!(input["file_path"], "/test.rs");
                    }
                    _ => panic!("Expected ToolUse block"),
                }
            }
            _ => panic!("Expected AssistantBlocks"),
        }

        // Check user message with tool_result
        match &injected[1] {
            InjectedMessage::UserBlocks { blocks, is_meta } => {
                assert_eq!(blocks.len(), 1);
                assert!(*is_meta);
                match &blocks[0] {
                    InjectedBlock::ToolResult {
                        tool_use_id,
                        content,
                    } => {
                        assert_eq!(tool_use_id, "test-id");
                        assert!(content.contains("Previously read"));
                    }
                    _ => panic!("Expected ToolResult block"),
                }
            }
            _ => panic!("Expected UserBlocks"),
        }
    }

    #[test]
    fn test_create_injected_messages_mixed() {
        let messages = vec![
            ReminderMessage::assistant(vec![ContentBlock::tool_use(
                "read-1",
                "Read",
                serde_json::json!({}),
            )]),
            ReminderMessage::user(vec![ContentBlock::tool_result("read-1", "content")]),
        ];
        let reminders = vec![
            test_reminder("Text reminder"),
            SystemReminder::messages(AttachmentType::AlreadyReadFile, messages),
        ];

        let injected = create_injected_messages(reminders);
        assert_eq!(injected.len(), 3); // 1 text + 2 messages

        // First should be text
        assert!(matches!(&injected[0], InjectedMessage::UserText { .. }));
        // Then assistant blocks
        assert!(matches!(
            &injected[1],
            InjectedMessage::AssistantBlocks { .. }
        ));
        // Then user blocks
        assert!(matches!(&injected[2], InjectedMessage::UserBlocks { .. }));
    }

    #[test]
    fn test_create_injected_messages_empty() {
        let injected = create_injected_messages(vec![]);
        assert!(injected.is_empty());
    }
}
