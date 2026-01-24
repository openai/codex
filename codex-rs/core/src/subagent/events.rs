//! Subagent activity events for UI feedback.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Activity event from a subagent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentActivityEvent {
    /// Unique identifier of the subagent instance.
    pub agent_id: String,
    /// Type of the agent (e.g., "Explore", "Plan").
    pub agent_type: String,
    /// Type of this event.
    pub event_type: SubagentEventType,
    /// Flexible event data.
    pub data: HashMap<String, JsonValue>,
}

impl SubagentActivityEvent {
    /// Create a new activity event.
    pub fn new(
        agent_id: impl Into<String>,
        agent_type: impl Into<String>,
        event_type: SubagentEventType,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            agent_type: agent_type.into(),
            event_type,
            data: HashMap::new(),
        }
    }

    /// Add data to the event.
    pub fn with_data(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.data.insert(key.into(), json_value);
        }
        self
    }

    /// Create a started event.
    pub fn started(agent_id: &str, agent_type: &str, description: &str) -> Self {
        Self::new(agent_id, agent_type, SubagentEventType::Started)
            .with_data("description", description)
    }

    /// Create a completed event.
    pub fn completed(
        agent_id: &str,
        agent_type: &str,
        turns_used: i32,
        duration_seconds: f32,
    ) -> Self {
        Self::new(agent_id, agent_type, SubagentEventType::Completed)
            .with_data("turns_used", turns_used)
            .with_data("duration_seconds", duration_seconds)
    }

    /// Create an error event.
    pub fn error(agent_id: &str, agent_type: &str, error: &str) -> Self {
        Self::new(agent_id, agent_type, SubagentEventType::Error).with_data("error", error)
    }

    /// Create a tool call start event.
    pub fn tool_call_start(agent_id: &str, agent_type: &str, tool_name: &str) -> Self {
        Self::new(agent_id, agent_type, SubagentEventType::ToolCallStart)
            .with_data("tool_name", tool_name)
    }

    /// Create a tool call end event.
    pub fn tool_call_end(
        agent_id: &str,
        agent_type: &str,
        tool_name: &str,
        success: bool,
        duration_ms: i64,
    ) -> Self {
        Self::new(agent_id, agent_type, SubagentEventType::ToolCallEnd)
            .with_data("tool_name", tool_name)
            .with_data("success", success)
            .with_data("duration_ms", duration_ms)
    }
}

/// Types of subagent activity events.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SubagentEventType {
    // Lifecycle events
    Started,
    Completed,
    Error,

    // Turn events
    TurnStart,
    TurnComplete,

    // Tool events
    ToolCallStart,
    ToolCallEnd,

    // Streaming events
    ThoughtChunk,

    // Grace period events
    GracePeriodStart,
    GracePeriodEnd,
}

/// Progress information for a running subagent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentProgress {
    /// Number of turns completed.
    pub turns_completed: i32,
    /// Maximum allowed turns.
    pub max_turns: i32,
    /// Elapsed time in seconds.
    pub elapsed_seconds: i32,
    /// Maximum allowed time in seconds.
    pub max_seconds: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = SubagentActivityEvent::started("agent-1", "Explore", "Finding files");
        assert_eq!(event.agent_id, "agent-1");
        assert_eq!(event.agent_type, "Explore");
        assert!(matches!(event.event_type, SubagentEventType::Started));
        assert!(event.data.contains_key("description"));
    }

    #[test]
    fn test_with_data() {
        let event = SubagentActivityEvent::new("agent-1", "Plan", SubagentEventType::TurnComplete)
            .with_data("turn_number", 5)
            .with_data("tool_calls", vec!["Read", "Glob"]);

        assert!(event.data.contains_key("turn_number"));
        assert!(event.data.contains_key("tool_calls"));
    }

    #[test]
    fn test_completed_event() {
        let event = SubagentActivityEvent::completed("agent-1", "Explore", 10, 45.5);
        assert!(matches!(event.event_type, SubagentEventType::Completed));
        assert_eq!(event.data.get("turns_used"), Some(&serde_json::json!(10)));
    }
}
