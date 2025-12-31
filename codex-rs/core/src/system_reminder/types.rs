//! Core types for the system reminder module.
//!
//! Matches Claude Code v2.0.59 attachment system (chunks.153.mjs).

use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;

// ============================================
// XML Tag Constants
// ============================================

/// Primary wrapper tag for most reminders.
pub const SYSTEM_REMINDER_OPEN_TAG: &str = "<system-reminder>";
pub const SYSTEM_REMINDER_CLOSE_TAG: &str = "</system-reminder>";

/// Specialized tag for async agent status.
pub const SYSTEM_NOTIFICATION_OPEN_TAG: &str = "<system-notification>";
pub const SYSTEM_NOTIFICATION_CLOSE_TAG: &str = "</system-notification>";

/// Tag for diagnostic issues.
pub const NEW_DIAGNOSTICS_OPEN_TAG: &str = "<new-diagnostics>";
pub const NEW_DIAGNOSTICS_CLOSE_TAG: &str = "</new-diagnostics>";

/// Tag for past session summaries.
pub const SESSION_MEMORY_OPEN_TAG: &str = "<session-memory>";
pub const SESSION_MEMORY_CLOSE_TAG: &str = "</session-memory>";

// ============================================
// XML Tag Enum
// ============================================

/// XML tag selection for different attachment types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XmlTag {
    /// `<system-reminder>` - Default for most types.
    SystemReminder,
    /// `<system-notification>` - For async agent status.
    SystemNotification,
    /// `<new-diagnostics>` - For diagnostic issues.
    NewDiagnostics,
    /// `<session-memory>` - For past session summaries.
    SessionMemory,
    /// No wrapping (direct content).
    None,
}

impl XmlTag {
    /// Wrap content with the appropriate XML tag.
    pub fn wrap(&self, content: &str) -> String {
        match self {
            XmlTag::SystemReminder => {
                format!("{SYSTEM_REMINDER_OPEN_TAG}\n{content}\n{SYSTEM_REMINDER_CLOSE_TAG}")
            }
            XmlTag::SystemNotification => {
                format!("{SYSTEM_NOTIFICATION_OPEN_TAG}{content}{SYSTEM_NOTIFICATION_CLOSE_TAG}")
            }
            XmlTag::NewDiagnostics => {
                format!("{NEW_DIAGNOSTICS_OPEN_TAG}{content}{NEW_DIAGNOSTICS_CLOSE_TAG}")
            }
            XmlTag::SessionMemory => {
                format!("{SESSION_MEMORY_OPEN_TAG}\n{content}\n{SESSION_MEMORY_CLOSE_TAG}")
            }
            XmlTag::None => content.to_string(),
        }
    }
}

// ============================================
// Tier and Type Enums
// ============================================

/// Categories of system reminders (matching Claude Code 3-tier system).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReminderTier {
    /// Always checked, available to all agents (sub-agents too).
    Core,
    /// Only for main agent, not sub-agents.
    MainAgentOnly,
    /// Only when user input exists.
    UserPrompt,
}

/// Types of system reminder attachments.
///
/// Matches Claude Code's 34+ types (implementing subset for Phase 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentType {
    // === Core tier (Phase 1) ===
    /// Periodic plan tool reminder (update_plan tool usage).
    PlanToolReminder,
    /// Plan mode instructions.
    PlanMode,
    /// Plan mode re-entry instructions.
    PlanModeReentry,
    /// Approved plan injection (one-time after ExitPlanMode approval).
    PlanApproved,
    /// File change notification.
    ChangedFiles,
    /// User-defined critical instruction.
    CriticalInstruction,

    // === Main agent only (Phase 1) ===
    /// Background shell task status.
    BackgroundTask,
    /// LSP diagnostics notification.
    LspDiagnostics,

    // === User prompt tier ===
    /// Files mentioned via @file, @"path", @file#L10-20 in user prompt.
    AtMentionedFiles,
    /// Agent mentions via @agent-type in user prompt.
    AgentMentions,

    // === Main agent only (Phase 1) ===
    /// Output style instructions (e.g., Explanatory, Learning).
    OutputStyle,

    // === Phase 2 (Future) ===
    /// Tool call result metadata.
    ToolResult,
    /// Auto-included related files (CLAUDE.md, etc.).
    NestedMemory,
    /// Async agent completion notification.
    AsyncAgentStatus,
    /// Past session summaries.
    SessionMemory,
    /// Token usage tracking.
    TokenUsage,
    /// Budget tracking.
    BudgetUsd,
}

impl AttachmentType {
    /// Get the XML tag for this attachment type.
    pub fn xml_tag(&self) -> XmlTag {
        match self {
            AttachmentType::AsyncAgentStatus => XmlTag::SystemNotification,
            AttachmentType::SessionMemory => XmlTag::SessionMemory,
            AttachmentType::LspDiagnostics => XmlTag::NewDiagnostics,
            _ => XmlTag::SystemReminder,
        }
    }

    /// Get the tier for this attachment type.
    pub fn tier(&self) -> ReminderTier {
        match self {
            AttachmentType::BackgroundTask
            | AttachmentType::AsyncAgentStatus
            | AttachmentType::SessionMemory
            | AttachmentType::TokenUsage
            | AttachmentType::BudgetUsd
            | AttachmentType::LspDiagnostics
            | AttachmentType::OutputStyle => ReminderTier::MainAgentOnly,
            AttachmentType::AtMentionedFiles | AttachmentType::AgentMentions => {
                ReminderTier::UserPrompt
            }
            _ => ReminderTier::Core,
        }
    }
}

impl fmt::Display for AttachmentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            AttachmentType::PlanToolReminder => "plan_tool_reminder",
            AttachmentType::PlanMode => "plan_mode",
            AttachmentType::PlanModeReentry => "plan_mode_reentry",
            AttachmentType::PlanApproved => "plan_approved",
            AttachmentType::ChangedFiles => "changed_files",
            AttachmentType::CriticalInstruction => "critical_instruction",
            AttachmentType::ToolResult => "tool_result",
            AttachmentType::NestedMemory => "nested_memory",
            AttachmentType::BackgroundTask => "background_task",
            AttachmentType::LspDiagnostics => "lsp_diagnostics",
            AttachmentType::AsyncAgentStatus => "async_agent_status",
            AttachmentType::SessionMemory => "session_memory",
            AttachmentType::TokenUsage => "token_usage",
            AttachmentType::BudgetUsd => "budget_usd",
            AttachmentType::AtMentionedFiles => "at_mentioned_files",
            AttachmentType::AgentMentions => "agent_mentions",
            AttachmentType::OutputStyle => "output_style",
        };
        write!(f, "{name}")
    }
}

// ============================================
// SystemReminder Struct
// ============================================

/// A system reminder attachment.
///
/// Matches structure from Claude Code's kb3() output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemReminder {
    /// Type of this attachment.
    pub attachment_type: AttachmentType,
    /// Content to be injected (before XML wrapping).
    pub content: String,
    /// Which tier this belongs to (derived from attachment_type).
    pub tier: ReminderTier,
    /// Whether this is metadata (always true for system reminders).
    /// Matches isMeta: true in Claude Code.
    pub is_meta: bool,
}

impl SystemReminder {
    /// Create a new system reminder.
    pub fn new(attachment_type: AttachmentType, content: String) -> Self {
        Self {
            tier: attachment_type.tier(),
            attachment_type,
            content,
            is_meta: true,
        }
    }

    /// Wrap content with appropriate XML tag.
    pub fn wrap_xml(&self) -> String {
        // LspDiagnostics content already includes XML tags from format_for_system_reminder()
        if self.attachment_type == AttachmentType::LspDiagnostics {
            return self.content.clone();
        }
        self.attachment_type.xml_tag().wrap(&self.content)
    }

    /// Check if a message content is a system reminder.
    pub fn is_system_reminder(content: &[ContentItem]) -> bool {
        if let [ContentItem::InputText { text }] = content {
            text.starts_with(SYSTEM_REMINDER_OPEN_TAG)
                || text.starts_with(SYSTEM_NOTIFICATION_OPEN_TAG)
                || text.starts_with(SESSION_MEMORY_OPEN_TAG)
                || text.starts_with(NEW_DIAGNOSTICS_OPEN_TAG)
        } else {
            false
        }
    }
}

/// Convert SystemReminder to ResponseItem (API message format).
///
/// Matches R0() function in Claude Code chunks.153.mjs:2179-2204.
impl From<SystemReminder> for ResponseItem {
    fn from(sr: SystemReminder) -> Self {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: sr.wrap_xml(),
            }],
        }
    }
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xml_tag_wrap_system_reminder() {
        let tag = XmlTag::SystemReminder;
        let result = tag.wrap("test content");
        assert!(result.starts_with("<system-reminder>"));
        assert!(result.ends_with("</system-reminder>"));
        assert!(result.contains("test content"));
    }

    #[test]
    fn test_xml_tag_wrap_system_notification() {
        let tag = XmlTag::SystemNotification;
        let result = tag.wrap("agent completed");
        assert_eq!(
            result,
            "<system-notification>agent completed</system-notification>"
        );
    }

    #[test]
    fn test_xml_tag_wrap_session_memory() {
        let tag = XmlTag::SessionMemory;
        let result = tag.wrap("session data");
        assert!(result.starts_with("<session-memory>"));
        assert!(result.ends_with("</session-memory>"));
    }

    #[test]
    fn test_xml_tag_wrap_none() {
        let tag = XmlTag::None;
        let result = tag.wrap("raw content");
        assert_eq!(result, "raw content");
    }

    #[test]
    fn test_system_reminder_is_meta() {
        let reminder = SystemReminder::new(AttachmentType::PlanToolReminder, "content".to_string());
        assert!(reminder.is_meta);
    }

    #[test]
    fn test_attachment_type_tier_mapping() {
        assert_eq!(AttachmentType::PlanToolReminder.tier(), ReminderTier::Core);
        assert_eq!(AttachmentType::PlanMode.tier(), ReminderTier::Core);
        assert_eq!(AttachmentType::ChangedFiles.tier(), ReminderTier::Core);
        assert_eq!(
            AttachmentType::CriticalInstruction.tier(),
            ReminderTier::Core
        );
        assert_eq!(
            AttachmentType::BackgroundTask.tier(),
            ReminderTier::MainAgentOnly
        );
        assert_eq!(
            AttachmentType::AsyncAgentStatus.tier(),
            ReminderTier::MainAgentOnly
        );
    }

    #[test]
    fn test_attachment_type_xml_tag() {
        assert_eq!(
            AttachmentType::PlanToolReminder.xml_tag(),
            XmlTag::SystemReminder
        );
        assert_eq!(
            AttachmentType::AsyncAgentStatus.xml_tag(),
            XmlTag::SystemNotification
        );
        assert_eq!(
            AttachmentType::SessionMemory.xml_tag(),
            XmlTag::SessionMemory
        );
    }

    #[test]
    fn test_system_reminder_wrap_xml() {
        let reminder = SystemReminder::new(
            AttachmentType::PlanToolReminder,
            "reminder content".to_string(),
        );
        let wrapped = reminder.wrap_xml();
        assert!(wrapped.starts_with("<system-reminder>"));
        assert!(wrapped.contains("reminder content"));
    }

    #[test]
    fn test_system_reminder_into_response_item() {
        let reminder =
            SystemReminder::new(AttachmentType::CriticalInstruction, "critical".to_string());
        let item: ResponseItem = reminder.into();

        match item {
            ResponseItem::Message { role, content, .. } => {
                assert_eq!(role, "user");
                assert_eq!(content.len(), 1);
                if let ContentItem::InputText { text } = &content[0] {
                    assert!(text.starts_with("<system-reminder>"));
                    assert!(text.contains("critical"));
                } else {
                    panic!("Expected InputText");
                }
            }
            _ => panic!("Expected Message variant"),
        }
    }

    #[test]
    fn test_is_system_reminder() {
        let reminder_content = vec![ContentItem::InputText {
            text: "<system-reminder>\ntest\n</system-reminder>".to_string(),
        }];
        assert!(SystemReminder::is_system_reminder(&reminder_content));

        let notification_content = vec![ContentItem::InputText {
            text: "<system-notification>test</system-notification>".to_string(),
        }];
        assert!(SystemReminder::is_system_reminder(&notification_content));

        let normal_content = vec![ContentItem::InputText {
            text: "regular message".to_string(),
        }];
        assert!(!SystemReminder::is_system_reminder(&normal_content));
    }

    #[test]
    fn test_attachment_type_display() {
        assert_eq!(
            format!("{}", AttachmentType::PlanToolReminder),
            "plan_tool_reminder"
        );
        assert_eq!(format!("{}", AttachmentType::PlanMode), "plan_mode");
        assert_eq!(
            format!("{}", AttachmentType::BackgroundTask),
            "background_task"
        );
        assert_eq!(
            format!("{}", AttachmentType::LspDiagnostics),
            "lsp_diagnostics"
        );
    }

    #[test]
    fn test_lsp_diagnostics_tier() {
        assert_eq!(
            AttachmentType::LspDiagnostics.tier(),
            ReminderTier::MainAgentOnly
        );
    }

    #[test]
    fn test_lsp_diagnostics_xml_tag() {
        assert_eq!(
            AttachmentType::LspDiagnostics.xml_tag(),
            XmlTag::NewDiagnostics
        );
    }

    #[test]
    fn test_lsp_diagnostics_wrap_xml_passthrough() {
        // LspDiagnostics should NOT double-wrap since content already has tags
        let content = "<new-diagnostics>\nTest diagnostics\n</new-diagnostics>".to_string();
        let reminder = SystemReminder {
            attachment_type: AttachmentType::LspDiagnostics,
            content: content.clone(),
            tier: ReminderTier::MainAgentOnly,
            is_meta: true,
        };
        assert_eq!(reminder.wrap_xml(), content);
    }
}
