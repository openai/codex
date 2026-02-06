//! Core types for system reminders.
//!
//! This module defines the fundamental types used throughout the system reminder
//! infrastructure, including attachment types, reminder tiers, and XML tags.

use serde::Deserialize;
use serde::Serialize;

/// Reminder tier determines when generators run.
///
/// Tiers allow filtering generators based on the agent context:
/// - `Core`: Always runs, for all agents including sub-agents
/// - `MainAgentOnly`: Only runs for the main agent, not sub-agents
/// - `UserPrompt`: Only runs when user input is present
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReminderTier {
    /// Always checked, available to all agents including sub-agents.
    Core,
    /// Only for main agent, not sub-agents.
    MainAgentOnly,
    /// Only when user input exists in this turn.
    UserPrompt,
}

/// XML tag types for wrapping reminder content.
///
/// Different tags serve different purposes and may be handled differently
/// by the model or UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XmlTag {
    /// Primary system reminder tag: `<system-reminder>`.
    SystemReminder,
    /// Async status notifications: `<system-notification>`.
    SystemNotification,
    /// LSP diagnostic issues: `<new-diagnostics>`.
    NewDiagnostics,
    /// Past session data: `<session-memory>`.
    SessionMemory,
    /// No XML wrapping (content is already wrapped or should be raw).
    None,
}

impl XmlTag {
    /// Get the XML tag name string.
    pub fn tag_name(&self) -> Option<&'static str> {
        match self {
            XmlTag::SystemReminder => Some("system-reminder"),
            XmlTag::SystemNotification => Some("system-notification"),
            XmlTag::NewDiagnostics => Some("new-diagnostics"),
            XmlTag::SessionMemory => Some("session-memory"),
            XmlTag::None => None,
        }
    }
}

/// Types of attachments that can be generated.
///
/// Each attachment type has an associated tier and XML tag. The generator
/// for each type produces content specific to that attachment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentType {
    // === Core tier (always run) ===
    /// Security guidelines (dual-placed for compaction survival).
    SecurityGuidelines,
    /// Detects files that changed since last read.
    ChangedFiles,
    /// Plan mode entry instructions (5-phase workflow).
    PlanModeEnter,
    /// Plan content after ExitPlanMode approval.
    PlanModeApproved,
    /// Reference to plan file after compaction.
    PlanModeFileReference,
    /// Periodic reminder to use update_plan tool.
    PlanToolReminder,
    /// Plan mode exit instructions (one-time after approval).
    PlanModeExit,
    /// User-defined critical instructions.
    CriticalInstruction,
    /// Auto-discovered CLAUDE.md and rules files.
    NestedMemory,

    // === MainAgentOnly tier ===
    /// Available skills for the Skill tool.
    AvailableSkills,
    /// Background shell task status.
    BackgroundTask,
    /// LSP diagnostic injection.
    LspDiagnostics,
    /// Output style instructions.
    OutputStyle,
    /// Task/todo list context.
    TodoReminders,
    /// Delegate mode instructions.
    DelegateMode,
    /// Collaboration notifications from other agents.
    CollabNotifications,
    /// Plan verification reminder during implementation.
    PlanVerification,

    // === UserPrompt tier ===
    /// Files mentioned via @file syntax.
    AtMentionedFiles,
    /// Agent invocations via @agent-type syntax.
    AgentMentions,
    /// Skill invoked by user (skill prompt content injection).
    InvokedSkills,

    // === Hook-related (MainAgentOnly tier) ===
    /// Background hook completed and returned additional context.
    AsyncHookResponse,
    /// Hook blocked execution and returned context.
    HookBlockingError,
    /// Hook succeeded and added context for the model.
    HookAdditionalContext,

    // === Real-time steering ===
    /// Queued commands from user (Enter during streaming).
    /// Injected as "User sent: {message}" to steer model in real-time.
    QueuedCommands,

    // === Phase 2 (future) ===
    /// Tool result injection.
    ToolResult,
    /// Async agent task status.
    AsyncAgentStatus,
    /// Session memory from past sessions.
    SessionMemoryContent,
    /// Token usage stats.
    TokenUsage,
    /// Budget in USD.
    BudgetUsd,

    // === Already read files ===
    /// Already read file summaries (generates tool_use/tool_result pairs).
    AlreadyReadFile,

    // === Compact file reference ===
    /// References to large files that were compacted.
    CompactFileReference,
}

impl AttachmentType {
    /// Get the XML tag for this attachment type.
    pub fn xml_tag(&self) -> XmlTag {
        match self {
            // Most attachments use the standard system-reminder tag
            AttachmentType::SecurityGuidelines
            | AttachmentType::ChangedFiles
            | AttachmentType::PlanModeEnter
            | AttachmentType::PlanModeApproved
            | AttachmentType::PlanModeFileReference
            | AttachmentType::PlanToolReminder
            | AttachmentType::PlanModeExit
            | AttachmentType::CriticalInstruction
            | AttachmentType::NestedMemory
            | AttachmentType::AvailableSkills
            | AttachmentType::BackgroundTask
            | AttachmentType::OutputStyle
            | AttachmentType::TodoReminders
            | AttachmentType::DelegateMode
            | AttachmentType::CollabNotifications
            | AttachmentType::PlanVerification
            | AttachmentType::AtMentionedFiles
            | AttachmentType::AgentMentions
            | AttachmentType::InvokedSkills
            | AttachmentType::AsyncHookResponse
            | AttachmentType::HookBlockingError
            | AttachmentType::HookAdditionalContext
            | AttachmentType::QueuedCommands
            | AttachmentType::ToolResult
            | AttachmentType::AsyncAgentStatus
            | AttachmentType::TokenUsage
            | AttachmentType::BudgetUsd
            | AttachmentType::CompactFileReference => XmlTag::SystemReminder,

            // Already read files don't use XML tags (uses tool_use/tool_result)
            AttachmentType::AlreadyReadFile => XmlTag::None,

            // LSP diagnostics have their own tag
            AttachmentType::LspDiagnostics => XmlTag::NewDiagnostics,

            // Session memory has its own tag
            AttachmentType::SessionMemoryContent => XmlTag::SessionMemory,
        }
    }

    /// Get the reminder tier for this attachment type.
    pub fn tier(&self) -> ReminderTier {
        match self {
            // Core tier - always run
            AttachmentType::SecurityGuidelines
            | AttachmentType::ChangedFiles
            | AttachmentType::PlanModeEnter
            | AttachmentType::PlanModeApproved
            | AttachmentType::PlanModeFileReference
            | AttachmentType::PlanToolReminder
            | AttachmentType::PlanModeExit
            | AttachmentType::CriticalInstruction
            | AttachmentType::NestedMemory => ReminderTier::Core,

            // MainAgentOnly tier
            AttachmentType::AvailableSkills
            | AttachmentType::BackgroundTask
            | AttachmentType::LspDiagnostics
            | AttachmentType::OutputStyle
            | AttachmentType::TodoReminders
            | AttachmentType::DelegateMode
            | AttachmentType::CollabNotifications
            | AttachmentType::PlanVerification
            | AttachmentType::AsyncHookResponse
            | AttachmentType::HookBlockingError
            | AttachmentType::HookAdditionalContext
            | AttachmentType::QueuedCommands
            | AttachmentType::ToolResult
            | AttachmentType::AsyncAgentStatus
            | AttachmentType::SessionMemoryContent
            | AttachmentType::TokenUsage
            | AttachmentType::BudgetUsd
            | AttachmentType::AlreadyReadFile
            | AttachmentType::CompactFileReference => ReminderTier::MainAgentOnly,

            // UserPrompt tier
            AttachmentType::AtMentionedFiles
            | AttachmentType::AgentMentions
            | AttachmentType::InvokedSkills => ReminderTier::UserPrompt,
        }
    }

    /// Get the display name for this attachment type.
    pub fn name(&self) -> &'static str {
        match self {
            AttachmentType::SecurityGuidelines => "security_guidelines",
            AttachmentType::ChangedFiles => "changed_files",
            AttachmentType::PlanModeEnter => "plan_mode_enter",
            AttachmentType::PlanModeApproved => "plan_mode_approved",
            AttachmentType::PlanModeFileReference => "plan_mode_file_reference",
            AttachmentType::PlanToolReminder => "plan_tool_reminder",
            AttachmentType::PlanModeExit => "plan_mode_exit",
            AttachmentType::CriticalInstruction => "critical_instruction",
            AttachmentType::NestedMemory => "nested_memory",
            AttachmentType::AvailableSkills => "available_skills",
            AttachmentType::BackgroundTask => "background_task",
            AttachmentType::LspDiagnostics => "lsp_diagnostics",
            AttachmentType::OutputStyle => "output_style",
            AttachmentType::TodoReminders => "todo_reminders",
            AttachmentType::DelegateMode => "delegate_mode",
            AttachmentType::CollabNotifications => "collab_notifications",
            AttachmentType::PlanVerification => "plan_verification",
            AttachmentType::AtMentionedFiles => "at_mentioned_files",
            AttachmentType::AgentMentions => "agent_mentions",
            AttachmentType::InvokedSkills => "invoked_skills",
            AttachmentType::AsyncHookResponse => "async_hook_response",
            AttachmentType::HookBlockingError => "hook_blocking_error",
            AttachmentType::HookAdditionalContext => "hook_additional_context",
            AttachmentType::QueuedCommands => "queued_commands",
            AttachmentType::ToolResult => "tool_result",
            AttachmentType::AsyncAgentStatus => "async_agent_status",
            AttachmentType::SessionMemoryContent => "session_memory",
            AttachmentType::TokenUsage => "token_usage",
            AttachmentType::BudgetUsd => "budget_usd",
            AttachmentType::AlreadyReadFile => "already_read_file",
            AttachmentType::CompactFileReference => "compact_file_reference",
        }
    }
}

impl std::fmt::Display for AttachmentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// ReminderOutput and related types
// ============================================================================

/// Generator output - supports multiple message types.
///
/// This enum allows generators to produce either simple text content
/// or multiple messages (used for tool_use/tool_result pairs).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReminderOutput {
    /// Single text content (most common case).
    Text(String),
    /// Multiple messages (used for tool_use/tool_result pairs).
    Messages(Vec<ReminderMessage>),
}

impl ReminderOutput {
    /// Get the text content if this is a Text variant.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ReminderOutput::Text(s) => Some(s),
            ReminderOutput::Messages(_) => None,
        }
    }

    /// Get the messages if this is a Messages variant.
    pub fn as_messages(&self) -> Option<&[ReminderMessage]> {
        match self {
            ReminderOutput::Text(_) => None,
            ReminderOutput::Messages(msgs) => Some(msgs),
        }
    }

    /// Check if this is a text output.
    pub fn is_text(&self) -> bool {
        matches!(self, ReminderOutput::Text(_))
    }

    /// Check if this is a messages output.
    pub fn is_messages(&self) -> bool {
        matches!(self, ReminderOutput::Messages(_))
    }
}

/// A message within a reminder output.
///
/// Used when generating tool_use/tool_result pairs or other multi-message content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderMessage {
    /// The role of this message (user or assistant).
    pub role: MessageRole,
    /// Content blocks within this message.
    pub blocks: Vec<ContentBlock>,
    /// Whether this is metadata (hidden from user, visible to model).
    pub is_meta: bool,
}

impl ReminderMessage {
    /// Create a new user message.
    pub fn user(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: MessageRole::User,
            blocks,
            is_meta: true,
        }
    }

    /// Create a new assistant message.
    pub fn assistant(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: MessageRole::Assistant,
            blocks,
            is_meta: true,
        }
    }
}

/// Role of a message within a reminder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// User message (typically contains tool_result).
    User,
    /// Assistant message (typically contains tool_use).
    Assistant,
}

/// Content block within a reminder message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Text content.
    Text { text: String },
    /// Tool use block (synthetic tool call).
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Tool result block.
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

impl ContentBlock {
    /// Create a text content block.
    pub fn text(text: impl Into<String>) -> Self {
        ContentBlock::Text { text: text.into() }
    }

    /// Create a tool use content block.
    pub fn tool_use(
        id: impl Into<String>,
        name: impl Into<String>,
        input: serde_json::Value,
    ) -> Self {
        ContentBlock::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }
    }

    /// Create a tool result content block.
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        ContentBlock::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
        }
    }
}

// ============================================================================
// SystemReminder
// ============================================================================

/// A generated system reminder ready for injection.
///
/// This represents the output of a generator after processing.
/// Supports both simple text content and multi-message outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemReminder {
    /// The type of attachment this reminder represents.
    pub attachment_type: AttachmentType,
    /// The output content (text or messages).
    pub output: ReminderOutput,
    /// The tier this reminder belongs to (derived from attachment_type).
    pub tier: ReminderTier,
    /// Whether this is metadata (hidden from user, visible to model).
    pub is_meta: bool,
}

impl SystemReminder {
    /// Create a new text-based system reminder.
    ///
    /// This is the most common case for simple text reminders.
    pub fn text(attachment_type: AttachmentType, content: impl Into<String>) -> Self {
        Self {
            tier: attachment_type.tier(),
            attachment_type,
            output: ReminderOutput::Text(content.into()),
            is_meta: true,
        }
    }

    /// Create a new multi-message system reminder.
    ///
    /// Used for generating tool_use/tool_result pairs and other
    /// multi-message content.
    pub fn messages(attachment_type: AttachmentType, messages: Vec<ReminderMessage>) -> Self {
        Self {
            tier: attachment_type.tier(),
            attachment_type,
            output: ReminderOutput::Messages(messages),
            is_meta: true,
        }
    }

    /// Create a new system reminder (legacy API, creates text output).
    ///
    /// For backwards compatibility. Prefer `SystemReminder::text()` for new code.
    pub fn new(attachment_type: AttachmentType, content: impl Into<String>) -> Self {
        Self::text(attachment_type, content)
    }

    /// Get the XML tag for this reminder.
    pub fn xml_tag(&self) -> XmlTag {
        self.attachment_type.xml_tag()
    }

    /// Get the text content if this is a text reminder.
    pub fn content(&self) -> Option<&str> {
        self.output.as_text()
    }

    /// Get the wrapped content with XML tags.
    ///
    /// Returns `None` for multi-message reminders (they don't use XML wrapping).
    pub fn wrapped_content(&self) -> Option<String> {
        match &self.output {
            ReminderOutput::Text(content) => {
                Some(crate::xml::wrap_with_tag(content, self.xml_tag()))
            }
            ReminderOutput::Messages(_) => None,
        }
    }

    /// Check if this is a text reminder.
    pub fn is_text(&self) -> bool {
        self.output.is_text()
    }

    /// Check if this is a multi-message reminder.
    pub fn is_messages(&self) -> bool {
        self.output.is_messages()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xml_tag_names() {
        assert_eq!(XmlTag::SystemReminder.tag_name(), Some("system-reminder"));
        assert_eq!(
            XmlTag::SystemNotification.tag_name(),
            Some("system-notification")
        );
        assert_eq!(XmlTag::NewDiagnostics.tag_name(), Some("new-diagnostics"));
        assert_eq!(XmlTag::SessionMemory.tag_name(), Some("session-memory"));
        assert_eq!(XmlTag::None.tag_name(), None);
    }

    #[test]
    fn test_attachment_type_tiers() {
        // Core tier
        assert_eq!(AttachmentType::ChangedFiles.tier(), ReminderTier::Core);
        assert_eq!(AttachmentType::PlanModeEnter.tier(), ReminderTier::Core);
        assert_eq!(AttachmentType::NestedMemory.tier(), ReminderTier::Core);

        // MainAgentOnly tier
        assert_eq!(
            AttachmentType::LspDiagnostics.tier(),
            ReminderTier::MainAgentOnly
        );
        assert_eq!(
            AttachmentType::TodoReminders.tier(),
            ReminderTier::MainAgentOnly
        );

        // UserPrompt tier
        assert_eq!(
            AttachmentType::AtMentionedFiles.tier(),
            ReminderTier::UserPrompt
        );
    }

    #[test]
    fn test_attachment_type_xml_tags() {
        assert_eq!(
            AttachmentType::ChangedFiles.xml_tag(),
            XmlTag::SystemReminder
        );
        assert_eq!(
            AttachmentType::LspDiagnostics.xml_tag(),
            XmlTag::NewDiagnostics
        );
        assert_eq!(
            AttachmentType::SessionMemoryContent.xml_tag(),
            XmlTag::SessionMemory
        );
    }

    #[test]
    fn test_system_reminder_creation() {
        let reminder = SystemReminder::new(
            AttachmentType::ChangedFiles,
            "File foo.rs has been modified",
        );

        assert_eq!(reminder.attachment_type, AttachmentType::ChangedFiles);
        assert_eq!(reminder.tier, ReminderTier::Core);
        assert!(reminder.is_meta);
        assert_eq!(reminder.content(), Some("File foo.rs has been modified"));
        assert!(reminder.is_text());
    }

    #[test]
    fn test_system_reminder_text() {
        let reminder = SystemReminder::text(
            AttachmentType::ChangedFiles,
            "File foo.rs has been modified",
        );

        assert_eq!(reminder.attachment_type, AttachmentType::ChangedFiles);
        assert!(reminder.is_text());
        assert!(!reminder.is_messages());
        assert_eq!(reminder.content(), Some("File foo.rs has been modified"));
    }

    #[test]
    fn test_system_reminder_messages() {
        let messages = vec![
            ReminderMessage::assistant(vec![ContentBlock::tool_use(
                "test-id",
                "Read",
                serde_json::json!({"file_path": "/test.rs"}),
            )]),
            ReminderMessage::user(vec![ContentBlock::tool_result(
                "test-id",
                "file content here",
            )]),
        ];
        let reminder = SystemReminder::messages(AttachmentType::AlreadyReadFile, messages);

        assert_eq!(reminder.attachment_type, AttachmentType::AlreadyReadFile);
        assert!(!reminder.is_text());
        assert!(reminder.is_messages());
        assert!(reminder.content().is_none());
        assert!(reminder.wrapped_content().is_none());

        let msgs = reminder.output.as_messages().unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, MessageRole::Assistant);
        assert_eq!(msgs[1].role, MessageRole::User);
    }

    #[test]
    fn test_content_block_creation() {
        let text = ContentBlock::text("hello");
        assert!(matches!(text, ContentBlock::Text { text } if text == "hello"));

        let tool_use = ContentBlock::tool_use("id-1", "Read", serde_json::json!({}));
        assert!(
            matches!(tool_use, ContentBlock::ToolUse { id, name, .. } if id == "id-1" && name == "Read")
        );

        let tool_result = ContentBlock::tool_result("id-1", "result");
        assert!(
            matches!(tool_result, ContentBlock::ToolResult { tool_use_id, content } if tool_use_id == "id-1" && content == "result")
        );
    }

    #[test]
    fn test_attachment_type_display() {
        assert_eq!(format!("{}", AttachmentType::ChangedFiles), "changed_files");
        assert_eq!(
            format!("{}", AttachmentType::PlanModeEnter),
            "plan_mode_enter"
        );
    }

    #[test]
    fn test_already_read_file_type() {
        assert_eq!(
            AttachmentType::AlreadyReadFile.tier(),
            ReminderTier::MainAgentOnly
        );
        assert_eq!(AttachmentType::AlreadyReadFile.xml_tag(), XmlTag::None);
        assert_eq!(AttachmentType::AlreadyReadFile.name(), "already_read_file");
    }
}
