//! Turn tracking for user-assistant exchanges.
//!
//! This module provides [`Turn`] and [`TrackedToolCall`] for tracking
//! complete user-assistant exchanges including tool executions.

use crate::tracked::TrackedMessage;
use chrono::DateTime;
use chrono::Utc;
use cocode_protocol::AbortReason;
use cocode_protocol::TokenUsage;
use cocode_protocol::ToolResultContent;
use hyper_sdk::ToolCall;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

/// Status of a tool call execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ToolCallStatus {
    /// Tool call is queued but not started.
    Pending,
    /// Tool call is currently executing.
    Running,
    /// Tool call completed successfully.
    Completed,
    /// Tool call failed with an error.
    Failed {
        /// Error message.
        error: String,
    },
    /// Tool call was aborted.
    Aborted {
        /// Reason for abortion.
        reason: AbortReason,
    },
}

impl ToolCallStatus {
    /// Check if the tool call is terminal (completed, failed, or aborted).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ToolCallStatus::Completed
                | ToolCallStatus::Failed { .. }
                | ToolCallStatus::Aborted { .. }
        )
    }

    /// Check if the tool call succeeded.
    pub fn is_success(&self) -> bool {
        matches!(self, ToolCallStatus::Completed)
    }

    /// Check if the tool call is still running.
    pub fn is_running(&self) -> bool {
        matches!(self, ToolCallStatus::Running)
    }
}

/// A tracked tool call with execution metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedToolCall {
    /// Tool call ID.
    pub call_id: String,
    /// Tool name.
    pub name: String,
    /// Tool input.
    pub input: Value,
    /// Tool output (once completed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<ToolResultContent>,
    /// Execution status.
    pub status: ToolCallStatus,
    /// When the tool call started.
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub started_at: DateTime<Utc>,
    /// When the tool call completed (if terminal).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "chrono::serde::ts_milliseconds_option"
    )]
    pub completed_at: Option<DateTime<Utc>>,
}

impl TrackedToolCall {
    /// Create a new pending tool call.
    pub fn new(tool_call: &ToolCall) -> Self {
        Self {
            call_id: tool_call.id.clone(),
            name: tool_call.name.clone(),
            input: tool_call.arguments.clone(),
            output: None,
            status: ToolCallStatus::Pending,
            started_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Create from individual fields.
    pub fn from_parts(call_id: impl Into<String>, name: impl Into<String>, input: Value) -> Self {
        Self {
            call_id: call_id.into(),
            name: name.into(),
            input,
            output: None,
            status: ToolCallStatus::Pending,
            started_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Mark as running.
    pub fn start(&mut self) {
        self.status = ToolCallStatus::Running;
    }

    /// Mark as completed with output.
    pub fn complete(&mut self, output: ToolResultContent) {
        self.output = Some(output);
        self.status = ToolCallStatus::Completed;
        self.completed_at = Some(Utc::now());
    }

    /// Mark as failed with error.
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = ToolCallStatus::Failed {
            error: error.into(),
        };
        self.completed_at = Some(Utc::now());
    }

    /// Mark as aborted.
    pub fn abort(&mut self, reason: AbortReason) {
        self.status = ToolCallStatus::Aborted { reason };
        self.completed_at = Some(Utc::now());
    }

    /// Get execution duration if completed.
    pub fn duration(&self) -> Option<chrono::Duration> {
        self.completed_at.map(|end| end - self.started_at)
    }

    /// Check if this is a safe tool (concurrent execution allowed).
    /// By default, we don't know - this should be checked against the registry.
    pub fn is_safe(&self) -> bool {
        // Default assumption - specific tools should be checked against registry
        false
    }
}

/// A complete turn in the conversation.
///
/// A turn represents a user-assistant exchange, potentially including
/// multiple tool calls and their results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    /// Unique turn identifier.
    pub id: String,
    /// Turn number (1-indexed).
    pub number: i32,
    /// User message that started this turn.
    pub user_message: TrackedMessage,
    /// Assistant response (once received).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_message: Option<TrackedMessage>,
    /// Tool calls made during this turn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<TrackedToolCall>,
    /// Token usage for this turn.
    #[serde(default)]
    pub usage: TokenUsage,
    /// When the turn started.
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub started_at: DateTime<Utc>,
    /// When the turn completed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "chrono::serde::ts_milliseconds_option"
    )]
    pub completed_at: Option<DateTime<Utc>>,
}

impl Turn {
    /// Create a new turn.
    pub fn new(number: i32, user_message: TrackedMessage) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            number,
            user_message,
            assistant_message: None,
            tool_calls: Vec::new(),
            usage: TokenUsage::default(),
            started_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Set the assistant message.
    pub fn set_assistant_message(&mut self, message: TrackedMessage) {
        self.assistant_message = Some(message);
    }

    /// Add a tool call.
    pub fn add_tool_call(&mut self, tool_call: TrackedToolCall) {
        self.tool_calls.push(tool_call);
    }

    /// Get a mutable reference to a tool call by ID.
    pub fn get_tool_call_mut(&mut self, call_id: &str) -> Option<&mut TrackedToolCall> {
        self.tool_calls.iter_mut().find(|tc| tc.call_id == call_id)
    }

    /// Get a reference to a tool call by ID.
    pub fn get_tool_call(&self, call_id: &str) -> Option<&TrackedToolCall> {
        self.tool_calls.iter().find(|tc| tc.call_id == call_id)
    }

    /// Update token usage.
    pub fn update_usage(&mut self, usage: TokenUsage) {
        self.usage.input_tokens += usage.input_tokens;
        self.usage.output_tokens += usage.output_tokens;
        if let Some(cache_read) = usage.cache_read_tokens {
            *self.usage.cache_read_tokens.get_or_insert(0) += cache_read;
        }
        if let Some(cache_creation) = usage.cache_creation_tokens {
            *self.usage.cache_creation_tokens.get_or_insert(0) += cache_creation;
        }
        if let Some(reasoning) = usage.reasoning_tokens {
            *self.usage.reasoning_tokens.get_or_insert(0) += reasoning;
        }
    }

    /// Complete the turn.
    pub fn complete(&mut self) {
        self.completed_at = Some(Utc::now());
    }

    /// Check if the turn is complete.
    pub fn is_complete(&self) -> bool {
        self.completed_at.is_some()
    }

    /// Check if all tool calls are terminal.
    pub fn all_tools_complete(&self) -> bool {
        self.tool_calls.iter().all(|tc| tc.status.is_terminal())
    }

    /// Get the number of pending tool calls.
    pub fn pending_tool_count(&self) -> usize {
        self.tool_calls
            .iter()
            .filter(|tc| !tc.status.is_terminal())
            .count()
    }

    /// Get turn duration if complete.
    pub fn duration(&self) -> Option<chrono::Duration> {
        self.completed_at.map(|end| end - self.started_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_user_message() -> TrackedMessage {
        TrackedMessage::user("Hello", "turn-1")
    }

    fn make_tool_call() -> TrackedToolCall {
        TrackedToolCall::from_parts("call-1", "get_weather", serde_json::json!({"city": "NYC"}))
    }

    #[test]
    fn test_tool_call_lifecycle() {
        let mut tc = make_tool_call();
        assert!(matches!(tc.status, ToolCallStatus::Pending));
        assert!(tc.completed_at.is_none());

        tc.start();
        assert!(tc.status.is_running());

        tc.complete(ToolResultContent::Text("Sunny, 72°F".to_string()));
        assert!(tc.status.is_success());
        assert!(tc.completed_at.is_some());
        assert!(tc.output.is_some());
    }

    #[test]
    fn test_tool_call_failure() {
        let mut tc = make_tool_call();
        tc.start();
        tc.fail("Network error");

        assert!(matches!(tc.status, ToolCallStatus::Failed { .. }));
        assert!(tc.status.is_terminal());
    }

    #[test]
    fn test_tool_call_abort() {
        let mut tc = make_tool_call();
        tc.start();
        tc.abort(AbortReason::UserInterrupted);

        assert!(matches!(tc.status, ToolCallStatus::Aborted { .. }));
        assert!(tc.status.is_terminal());
    }

    #[test]
    fn test_turn_creation() {
        let user_msg = make_user_message();
        let turn = Turn::new(1, user_msg);

        assert_eq!(turn.number, 1);
        assert!(!turn.is_complete());
        assert!(turn.assistant_message.is_none());
        assert!(turn.tool_calls.is_empty());
    }

    #[test]
    fn test_turn_with_tool_calls() {
        let user_msg = make_user_message();
        let mut turn = Turn::new(1, user_msg);

        turn.add_tool_call(make_tool_call());
        assert_eq!(turn.tool_calls.len(), 1);
        assert_eq!(turn.pending_tool_count(), 1);

        // Complete the tool call
        let tc = turn.get_tool_call_mut("call-1").unwrap();
        tc.complete(ToolResultContent::Text("done".to_string()));

        assert_eq!(turn.pending_tool_count(), 0);
        assert!(turn.all_tools_complete());
    }

    #[test]
    fn test_turn_usage() {
        let user_msg = make_user_message();
        let mut turn = Turn::new(1, user_msg);

        turn.update_usage(TokenUsage::new(100, 50));
        turn.update_usage(TokenUsage::new(50, 25));

        assert_eq!(turn.usage.input_tokens, 150);
        assert_eq!(turn.usage.output_tokens, 75);
    }

    #[test]
    fn test_turn_completion() {
        let user_msg = make_user_message();
        let mut turn = Turn::new(1, user_msg);

        assert!(!turn.is_complete());
        turn.complete();
        assert!(turn.is_complete());
        assert!(turn.duration().is_some());
    }
}
