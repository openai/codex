//! Hook input schemas aligned with Claude Code.

use serde::Deserialize;
use serde::Serialize;

use crate::types::HookEventType;

/// Base context provided to ALL hooks.
///
/// Aligned with Claude Code's `tE()` function which injects these fields
/// into every hook input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    /// Current session identifier.
    pub session_id: String,

    /// Path to session transcript file.
    pub transcript_path: String,

    /// Current working directory.
    pub cwd: String,

    /// Permission context from application state.
    pub permission_mode: serde_json::Value,
}

impl Default for HookContext {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            transcript_path: String::new(),
            cwd: ".".to_string(),
            permission_mode: serde_json::Value::Null,
        }
    }
}

/// Complete hook input (event-specific fields + base context).
///
/// This is the JSON structure passed to command hooks via stdin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookInput {
    /// The type of hook event.
    pub hook_event_name: HookEventType,

    /// Session ID (from context).
    pub session_id: String,

    /// Path to transcript file (from context).
    pub transcript_path: String,

    /// Current working directory (from context).
    pub cwd: String,

    /// Permission mode configuration (from context).
    pub permission_mode: serde_json::Value,

    /// Event-specific fields (flattened).
    #[serde(flatten)]
    pub event_data: HookEventData,
}

impl HookInput {
    /// Create a new hook input with context and event data.
    pub fn new(event_type: HookEventType, context: HookContext, event_data: HookEventData) -> Self {
        Self {
            hook_event_name: event_type,
            session_id: context.session_id,
            transcript_path: context.transcript_path,
            cwd: context.cwd,
            permission_mode: context.permission_mode,
            event_data,
        }
    }

    /// Get the match value for this input based on the event type.
    ///
    /// Returns the value that should be matched against hook matchers.
    pub fn match_value(&self) -> Option<&str> {
        self.event_data.match_value()
    }
}

/// Event-specific input data.
///
/// Each variant contains the fields specific to that hook event type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HookEventData {
    /// Tool-related events: PreToolUse, PostToolUse, PostToolUseFailure, PermissionRequest.
    ToolEvent {
        /// Name of the tool being used.
        tool_name: String,

        /// Input parameters for the tool.
        tool_input: serde_json::Value,

        /// Unique identifier for this tool use.
        tool_use_id: String,

        /// Tool response (PostToolUse only).
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_response: Option<String>,

        /// Error message (PostToolUseFailure only).
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,

        /// Whether failure was due to user interrupt (PostToolUseFailure only).
        #[serde(skip_serializing_if = "Option::is_none")]
        is_interrupt: Option<bool>,

        /// Permission suggestions (PermissionRequest only).
        #[serde(skip_serializing_if = "Option::is_none")]
        permission_suggestions: Option<Vec<serde_json::Value>>,
    },

    /// SessionStart event.
    SessionStart {
        /// Source of the session (e.g., "cli", "ide", "api").
        source: String,
    },

    /// SessionEnd event.
    SessionEnd {
        /// Reason for ending the session.
        reason: String,
    },

    /// SubagentStart event.
    SubagentStart {
        /// Unique identifier for the subagent.
        agent_id: String,

        /// Type of subagent (e.g., "Explore", "Plan").
        agent_type: String,
    },

    /// SubagentStop event.
    SubagentStop {
        /// Whether stop was user-initiated.
        stop_hook_active: bool,

        /// Unique identifier for the subagent.
        agent_id: String,

        /// Path to the subagent's transcript file.
        agent_transcript_path: String,
    },

    /// Stop event (user interruption).
    Stop {
        /// Always true for Stop hooks.
        stop_hook_active: bool,
    },

    /// UserPromptSubmit event.
    UserPromptSubmit {
        /// The user's prompt text.
        prompt: String,
    },

    /// Notification event.
    Notification {
        /// Notification message content.
        message: String,

        /// Notification title.
        title: String,

        /// Type of notification (e.g., "error", "info", "warning").
        notification_type: String,
    },

    /// PreCompact event.
    PreCompact {
        /// What triggered compaction: "auto" or "manual".
        trigger: String,

        /// Current custom instructions.
        #[serde(skip_serializing_if = "Option::is_none")]
        custom_instructions: Option<String>,
    },

    /// Empty event data for events with no specific fields.
    Empty {},
}

impl HookEventData {
    /// Get the match value for this event data.
    ///
    /// Returns the value that should be matched against hook matchers.
    pub fn match_value(&self) -> Option<&str> {
        match self {
            Self::ToolEvent { tool_name, .. } => Some(tool_name),
            Self::SessionStart { source } => Some(source),
            Self::SessionEnd { reason } => Some(reason),
            Self::SubagentStart { agent_type, .. } => Some(agent_type),
            Self::Notification {
                notification_type, ..
            } => Some(notification_type),
            Self::PreCompact { trigger, .. } => Some(trigger),
            // No matcher for these events
            Self::Stop { .. } | Self::SubagentStop { .. } | Self::UserPromptSubmit { .. } => None,
            Self::Empty {} => None,
        }
    }

    /// Create tool event data for PreToolUse.
    pub fn pre_tool_use(
        tool_name: String,
        tool_input: serde_json::Value,
        tool_use_id: String,
    ) -> Self {
        Self::ToolEvent {
            tool_name,
            tool_input,
            tool_use_id,
            tool_response: None,
            error: None,
            is_interrupt: None,
            permission_suggestions: None,
        }
    }

    /// Create tool event data for PostToolUse.
    pub fn post_tool_use(
        tool_name: String,
        tool_input: serde_json::Value,
        tool_use_id: String,
        tool_response: String,
    ) -> Self {
        Self::ToolEvent {
            tool_name,
            tool_input,
            tool_use_id,
            tool_response: Some(tool_response),
            error: None,
            is_interrupt: None,
            permission_suggestions: None,
        }
    }

    /// Create tool event data for PostToolUseFailure.
    pub fn post_tool_use_failure(
        tool_name: String,
        tool_input: serde_json::Value,
        tool_use_id: String,
        error: String,
        is_interrupt: bool,
    ) -> Self {
        Self::ToolEvent {
            tool_name,
            tool_input,
            tool_use_id,
            tool_response: None,
            error: Some(error),
            is_interrupt: Some(is_interrupt),
            permission_suggestions: None,
        }
    }

    /// Create tool event data for PermissionRequest.
    pub fn permission_request(
        tool_name: String,
        tool_input: serde_json::Value,
        tool_use_id: String,
        permission_suggestions: Vec<serde_json::Value>,
    ) -> Self {
        Self::ToolEvent {
            tool_name,
            tool_input,
            tool_use_id,
            tool_response: None,
            error: None,
            is_interrupt: None,
            permission_suggestions: Some(permission_suggestions),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_input_serialization() {
        let input = HookInput {
            hook_event_name: HookEventType::PreToolUse,
            session_id: "session-123".to_string(),
            transcript_path: "/tmp/transcript.json".to_string(),
            cwd: "/home/user/project".to_string(),
            permission_mode: serde_json::json!({"mode": "ask"}),
            event_data: HookEventData::pre_tool_use(
                "Bash".to_string(),
                serde_json::json!({"command": "ls"}),
                "tool-456".to_string(),
            ),
        };

        let json = serde_json::to_string_pretty(&input).expect("serialization should succeed");
        assert!(json.contains("PreToolUse"));
        assert!(json.contains("Bash"));
        assert!(json.contains("session-123"));
    }

    #[test]
    fn test_event_data_match_value() {
        let tool_event = HookEventData::pre_tool_use(
            "Write".to_string(),
            serde_json::Value::Null,
            "id".to_string(),
        );
        assert_eq!(tool_event.match_value(), Some("Write"));

        let session_start = HookEventData::SessionStart {
            source: "cli".to_string(),
        };
        assert_eq!(session_start.match_value(), Some("cli"));

        let stop = HookEventData::Stop {
            stop_hook_active: true,
        };
        assert_eq!(stop.match_value(), None);
    }
}
