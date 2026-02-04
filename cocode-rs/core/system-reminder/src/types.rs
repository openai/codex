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
            | AttachmentType::BudgetUsd => XmlTag::SystemReminder,

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
            | AttachmentType::BudgetUsd => ReminderTier::MainAgentOnly,

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
        }
    }
}

impl std::fmt::Display for AttachmentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A generated system reminder ready for injection.
///
/// This represents the output of a generator after processing.
/// The content is pre-formatted but not yet wrapped in XML tags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemReminder {
    /// The type of attachment this reminder represents.
    pub attachment_type: AttachmentType,
    /// The content of the reminder (before XML wrapping).
    pub content: String,
    /// The tier this reminder belongs to (derived from attachment_type).
    pub tier: ReminderTier,
    /// Whether this is metadata (hidden from user, visible to model).
    pub is_meta: bool,
}

impl SystemReminder {
    /// Create a new system reminder.
    pub fn new(attachment_type: AttachmentType, content: impl Into<String>) -> Self {
        Self {
            tier: attachment_type.tier(),
            attachment_type,
            content: content.into(),
            is_meta: true, // System reminders are always meta by default
        }
    }

    /// Get the XML tag for this reminder.
    pub fn xml_tag(&self) -> XmlTag {
        self.attachment_type.xml_tag()
    }

    /// Get the wrapped content with XML tags.
    pub fn wrapped_content(&self) -> String {
        crate::xml::wrap_with_tag(&self.content, self.xml_tag())
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
        assert_eq!(reminder.content, "File foo.rs has been modified");
    }

    #[test]
    fn test_attachment_type_display() {
        assert_eq!(format!("{}", AttachmentType::ChangedFiles), "changed_files");
        assert_eq!(
            format!("{}", AttachmentType::PlanModeEnter),
            "plan_mode_enter"
        );
    }
}
