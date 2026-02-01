//! Hook event types.
//!
//! Defines the lifecycle points at which hooks can be triggered.

use serde::Deserialize;
use serde::Serialize;

/// Type of hook event that triggers hook execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEventType {
    /// Before a tool is used.
    PreToolUse,
    /// After a tool completes successfully.
    PostToolUse,
    /// After a tool use fails.
    PostToolUseFailure,
    /// When the user submits a prompt.
    UserPromptSubmit,
    /// When a session starts.
    SessionStart,
    /// When a session ends.
    SessionEnd,
    /// When the agent stops.
    Stop,
    /// When a sub-agent starts.
    SubagentStart,
    /// When a sub-agent stops.
    SubagentStop,
    /// Before context compaction occurs.
    PreCompact,
    /// A notification event (informational, no blocking).
    Notification,
    /// When a permission is requested.
    PermissionRequest,
}

impl HookEventType {
    /// Returns the string representation of this event type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PreToolUse => "pre_tool_use",
            Self::PostToolUse => "post_tool_use",
            Self::PostToolUseFailure => "post_tool_use_failure",
            Self::UserPromptSubmit => "user_prompt_submit",
            Self::SessionStart => "session_start",
            Self::SessionEnd => "session_end",
            Self::Stop => "stop",
            Self::SubagentStart => "subagent_start",
            Self::SubagentStop => "subagent_stop",
            Self::PreCompact => "pre_compact",
            Self::Notification => "notification",
            Self::PermissionRequest => "permission_request",
        }
    }
}

impl std::fmt::Display for HookEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_str() {
        assert_eq!(HookEventType::PreToolUse.as_str(), "pre_tool_use");
        assert_eq!(HookEventType::PostToolUse.as_str(), "post_tool_use");
        assert_eq!(
            HookEventType::PostToolUseFailure.as_str(),
            "post_tool_use_failure"
        );
        assert_eq!(
            HookEventType::UserPromptSubmit.as_str(),
            "user_prompt_submit"
        );
        assert_eq!(HookEventType::SessionStart.as_str(), "session_start");
        assert_eq!(HookEventType::SessionEnd.as_str(), "session_end");
        assert_eq!(HookEventType::Stop.as_str(), "stop");
        assert_eq!(HookEventType::SubagentStart.as_str(), "subagent_start");
        assert_eq!(HookEventType::SubagentStop.as_str(), "subagent_stop");
        assert_eq!(HookEventType::PreCompact.as_str(), "pre_compact");
        assert_eq!(HookEventType::Notification.as_str(), "notification");
        assert_eq!(
            HookEventType::PermissionRequest.as_str(),
            "permission_request"
        );
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", HookEventType::PreToolUse), "pre_tool_use");
        assert_eq!(format!("{}", HookEventType::Stop), "stop");
    }

    #[test]
    fn test_serde_roundtrip() {
        let event = HookEventType::PostToolUse;
        let json = serde_json::to_string(&event).expect("serialize");
        assert_eq!(json, "\"post_tool_use\"");
        let parsed: HookEventType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, event);
    }

    #[test]
    fn test_clone_eq_hash() {
        let a = HookEventType::SessionStart;
        let b = a.clone();
        assert_eq!(a, b);

        // Test Hash by inserting into a HashSet
        let mut set = std::collections::HashSet::new();
        set.insert(a.clone());
        assert!(set.contains(&b));
    }
}
