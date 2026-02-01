//! Message history management for conversations.
//!
//! This module provides [`MessageHistory`] which manages the conversation
//! history for the agent loop, including turn tracking and compaction.

use crate::normalization::NormalizationOptions;
use crate::normalization::estimate_tokens;
use crate::normalization::normalize_messages_for_api;
use crate::tracked::TrackedMessage;
use crate::turn::Turn;
use cocode_protocol::TokenUsage;
use hyper_sdk::Message;
use serde::Deserialize;
use serde::Serialize;

/// Configuration for message history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    /// Maximum turns to keep before compaction.
    #[serde(default = "default_max_turns")]
    pub max_turns: i32,
    /// Context window size for token budget.
    #[serde(default = "default_context_window")]
    pub context_window: i32,
    /// Threshold ratio for triggering compaction (0.0-1.0).
    #[serde(default = "default_compaction_threshold")]
    pub compaction_threshold: f32,
    /// Whether to enable automatic compaction.
    #[serde(default = "default_auto_compact")]
    pub auto_compact: bool,
}

/// Metadata about a compaction boundary.
///
/// This marks where compaction occurred in the conversation history,
/// helping distinguish between pre-compaction and post-compaction content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionBoundary {
    /// Turn ID where compaction occurred.
    pub turn_id: String,
    /// Turn number at compaction time.
    pub turn_number: i32,
    /// Number of turns that were compacted.
    pub turns_compacted: i32,
    /// Estimated tokens saved by compaction.
    pub tokens_saved: i32,
    /// Timestamp of compaction (Unix milliseconds).
    pub timestamp_ms: i64,
    /// Trigger type for the compaction.
    #[serde(default)]
    pub trigger: cocode_protocol::CompactTrigger,
    /// Pre-compaction token count.
    #[serde(default)]
    pub pre_tokens: i32,
    /// Post-compaction token count.
    #[serde(default)]
    pub post_tokens: Option<i32>,
    /// Path to full transcript file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<std::path::PathBuf>,
    /// Whether recent messages were preserved verbatim.
    #[serde(default)]
    pub recent_messages_preserved: bool,
}

fn default_max_turns() -> i32 {
    100
}
fn default_context_window() -> i32 {
    128000
}
fn default_compaction_threshold() -> f32 {
    0.8
}
fn default_auto_compact() -> bool {
    true
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_turns: default_max_turns(),
            context_window: default_context_window(),
            compaction_threshold: default_compaction_threshold(),
            auto_compact: default_auto_compact(),
        }
    }
}

/// Message history for a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHistory {
    /// System message (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    system_message: Option<TrackedMessage>,
    /// Turns in the conversation.
    turns: Vec<Turn>,
    /// Compacted summary of earlier turns.
    #[serde(skip_serializing_if = "Option::is_none")]
    compacted_summary: Option<String>,
    /// Compaction boundary marker.
    #[serde(skip_serializing_if = "Option::is_none")]
    compaction_boundary: Option<CompactionBoundary>,
    /// Total input tokens used.
    #[serde(default)]
    total_input_tokens: i64,
    /// Total output tokens used.
    #[serde(default)]
    total_output_tokens: i64,
    /// Configuration.
    #[serde(default)]
    config: HistoryConfig,
}

impl Default for MessageHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageHistory {
    /// Create a new empty history.
    pub fn new() -> Self {
        Self {
            system_message: None,
            turns: Vec::new(),
            compacted_summary: None,
            compaction_boundary: None,
            total_input_tokens: 0,
            total_output_tokens: 0,
            config: HistoryConfig::default(),
        }
    }

    /// Create history with custom configuration.
    pub fn with_config(config: HistoryConfig) -> Self {
        Self {
            config,
            ..Self::new()
        }
    }

    /// Set the system message.
    pub fn set_system_message(&mut self, message: TrackedMessage) {
        self.system_message = Some(message);
    }

    /// Get the system message.
    pub fn system_message(&self) -> Option<&TrackedMessage> {
        self.system_message.as_ref()
    }

    /// Add a turn to the history.
    pub fn add_turn(&mut self, turn: Turn) {
        self.total_input_tokens += turn.usage.input_tokens;
        self.total_output_tokens += turn.usage.output_tokens;
        self.turns.push(turn);
    }

    /// Get all turns.
    pub fn turns(&self) -> &[Turn] {
        &self.turns
    }

    /// Get the current turn (last turn).
    pub fn current_turn(&self) -> Option<&Turn> {
        self.turns.last()
    }

    /// Get mutable reference to current turn.
    pub fn current_turn_mut(&mut self) -> Option<&mut Turn> {
        self.turns.last_mut()
    }

    /// Get turn count.
    pub fn turn_count(&self) -> i32 {
        self.turns.len() as i32
    }

    /// Get total token usage.
    pub fn total_usage(&self) -> TokenUsage {
        TokenUsage {
            input_tokens: self.total_input_tokens,
            output_tokens: self.total_output_tokens,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            reasoning_tokens: None,
        }
    }

    /// Collect all messages for API request.
    pub fn messages_for_api(&self) -> Vec<Message> {
        let mut messages = Vec::new();

        // Add system message first
        if let Some(system) = &self.system_message {
            messages.push(system.inner.clone());
        }

        // Add compacted summary if present
        if let Some(summary) = &self.compacted_summary {
            messages.push(Message::user(format!(
                "<conversation_summary>\n{summary}\n</conversation_summary>\n\nContinuing from the summary above:"
            )));
        }

        // Collect all messages from turns
        let mut tracked: Vec<TrackedMessage> = Vec::new();
        for turn in &self.turns {
            tracked.push(turn.user_message.clone());
            if let Some(assistant_msg) = &turn.assistant_message {
                tracked.push(assistant_msg.clone());
            }
        }

        // Normalize and add
        let normalized = normalize_messages_for_api(&tracked, &NormalizationOptions::for_api());
        messages.extend(normalized);

        messages
    }

    /// Estimate total tokens in current history.
    pub fn estimate_tokens(&self) -> i32 {
        let messages = self.messages_for_api();
        estimate_tokens(&messages)
    }

    /// Get messages as JSON values for compaction calculations.
    ///
    /// This is used by the keep window algorithm which operates on
    /// JSON message structures rather than typed Message objects.
    pub fn messages_for_api_json(&self) -> Vec<serde_json::Value> {
        self.messages_for_api()
            .iter()
            .filter_map(|m| serde_json::to_value(m).ok())
            .collect()
    }

    /// Check if compaction is needed.
    pub fn needs_compaction(&self) -> bool {
        if !self.config.auto_compact {
            return false;
        }

        let estimated = self.estimate_tokens();
        let threshold =
            (self.config.context_window as f32 * self.config.compaction_threshold) as i32;

        estimated > threshold || self.turn_count() > self.config.max_turns
    }

    /// Get the compaction threshold in tokens.
    pub fn compaction_threshold_tokens(&self) -> i32 {
        (self.config.context_window as f32 * self.config.compaction_threshold) as i32
    }

    /// Set compacted summary and clear old turns.
    ///
    /// This method adjusts token accounting by deducting the tokens from
    /// removed turns to maintain accurate running totals.
    ///
    /// # Arguments
    /// * `summary` - The compacted summary text
    /// * `keep_turns` - Number of recent turns to keep
    /// * `turn_id` - ID of the turn where compaction occurred
    /// * `tokens_saved` - Estimated tokens saved by compaction
    pub fn apply_compaction(
        &mut self,
        summary: String,
        keep_turns: i32,
        turn_id: impl Into<String>,
        tokens_saved: i32,
    ) {
        self.apply_compaction_with_metadata(
            summary,
            keep_turns,
            turn_id,
            tokens_saved,
            cocode_protocol::CompactTrigger::Auto,
            0,
            None,
            false,
        );
    }

    /// Set compacted summary with full metadata.
    ///
    /// Extended version of `apply_compaction` that includes additional metadata
    /// for compact boundary tracking.
    ///
    /// # Arguments
    /// * `summary` - The compacted summary text
    /// * `keep_turns` - Number of recent turns to keep
    /// * `turn_id` - ID of the turn where compaction occurred
    /// * `tokens_saved` - Estimated tokens saved by compaction
    /// * `trigger` - Trigger type (auto or manual)
    /// * `pre_tokens` - Token count before compaction
    /// * `transcript_path` - Optional path to full transcript file
    /// * `recent_messages_preserved` - Whether recent messages were kept verbatim
    #[allow(clippy::too_many_arguments)]
    pub fn apply_compaction_with_metadata(
        &mut self,
        summary: String,
        keep_turns: i32,
        turn_id: impl Into<String>,
        tokens_saved: i32,
        trigger: cocode_protocol::CompactTrigger,
        pre_tokens: i32,
        transcript_path: Option<std::path::PathBuf>,
        recent_messages_preserved: bool,
    ) {
        let turns_compacted = self.turns.len().saturating_sub(keep_turns.max(1) as usize) as i32;
        let turn_number = self.turn_count();

        // Record the compaction boundary
        self.compaction_boundary = Some(CompactionBoundary {
            turn_id: turn_id.into(),
            turn_number,
            turns_compacted,
            tokens_saved,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
            trigger,
            pre_tokens,
            post_tokens: None, // Will be set after compaction completes
            transcript_path,
            recent_messages_preserved,
        });

        self.compacted_summary = Some(summary);

        // Keep only the most recent turns
        let keep = keep_turns.max(1) as usize;
        if self.turns.len() > keep {
            // Calculate tokens to deduct from removed turns
            let remove_count = self.turns.len() - keep;
            for turn in self.turns.iter().take(remove_count) {
                self.total_input_tokens -= turn.usage.input_tokens;
                self.total_output_tokens -= turn.usage.output_tokens;
            }

            self.turns = self.turns.split_off(self.turns.len() - keep);
        }
    }

    /// Update the compaction boundary with post-compaction token count.
    pub fn update_boundary_post_tokens(&mut self, post_tokens: i32) {
        if let Some(boundary) = &mut self.compaction_boundary {
            boundary.post_tokens = Some(post_tokens);
        }
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.turns.clear();
        self.compacted_summary = None;
        self.compaction_boundary = None;
        self.total_input_tokens = 0;
        self.total_output_tokens = 0;
    }

    /// Get the compaction boundary (if any).
    pub fn compaction_boundary(&self) -> Option<&CompactionBoundary> {
        self.compaction_boundary.as_ref()
    }

    /// Get the compacted summary (if any).
    pub fn compacted_summary(&self) -> Option<&str> {
        self.compacted_summary.as_deref()
    }

    /// Micro-compact: Clear old tool result content to save tokens.
    ///
    /// This removes the content of tool results in older turns while keeping
    /// the most recent `keep_recent` tool results intact.
    ///
    /// Returns the number of tool results that were compacted.
    pub fn micro_compact(&mut self, keep_recent: i32) -> i32 {
        let mut compacted_count = 0;
        let total_turns = self.turns.len();

        // Count total tool calls across all turns
        let total_tool_calls: i32 = self.turns.iter().map(|t| t.tool_calls.len() as i32).sum();

        if total_tool_calls <= keep_recent {
            return 0; // Nothing to compact
        }

        // Process turns from oldest to newest, clearing tool outputs
        // until we've kept only the most recent `keep_recent` results
        let mut kept = 0;
        let skip_count = (total_tool_calls - keep_recent).max(0) as usize;
        let mut processed = 0;

        for turn in self.turns.iter_mut() {
            for tool_call in turn.tool_calls.iter_mut() {
                if processed < skip_count {
                    // Clear this tool's output
                    if tool_call.output.is_some() {
                        tool_call.output = Some(cocode_protocol::ToolResultContent::Text(
                            "[micro-compacted]".to_string(),
                        ));
                        compacted_count += 1;
                    }
                    processed += 1;
                } else {
                    kept += 1;
                }
            }
        }

        tracing::debug!(
            total_turns,
            total_tool_calls,
            compacted_count,
            kept,
            "Micro-compaction complete"
        );

        compacted_count
    }

    /// Add a tool result to the current turn.
    pub fn add_tool_result(
        &mut self,
        call_id: impl Into<String>,
        name: impl Into<String>,
        output: cocode_protocol::ToolResultContent,
        is_error: bool,
    ) {
        if let Some(turn) = self.current_turn_mut() {
            let mut tool_call =
                crate::turn::TrackedToolCall::from_parts(call_id, name, serde_json::Value::Null);
            if is_error {
                match &output {
                    cocode_protocol::ToolResultContent::Text(t) => tool_call.fail(t.clone()),
                    cocode_protocol::ToolResultContent::Structured(v) => {
                        tool_call.fail(v.to_string())
                    }
                }
            } else {
                tool_call.complete(output);
            }
            turn.add_tool_call(tool_call);
        }
    }

    /// Get mutable access to turns (for adding tool results).
    pub fn turns_mut(&mut self) -> &mut Vec<Turn> {
        &mut self.turns
    }

    /// Get configuration.
    pub fn config(&self) -> &HistoryConfig {
        &self.config
    }

    /// Update configuration.
    pub fn set_config(&mut self, config: HistoryConfig) {
        self.config = config;
    }
}

/// Builder for creating message history.
pub struct HistoryBuilder {
    config: HistoryConfig,
    system_message: Option<String>,
}

impl HistoryBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: HistoryConfig::default(),
            system_message: None,
        }
    }

    /// Set the context window size.
    pub fn context_window(mut self, size: i32) -> Self {
        self.config.context_window = size;
        self
    }

    /// Set the compaction threshold.
    pub fn compaction_threshold(mut self, threshold: f32) -> Self {
        self.config.compaction_threshold = threshold;
        self
    }

    /// Set max turns.
    pub fn max_turns(mut self, max: i32) -> Self {
        self.config.max_turns = max;
        self
    }

    /// Enable or disable auto compaction.
    pub fn auto_compact(mut self, enabled: bool) -> Self {
        self.config.auto_compact = enabled;
        self
    }

    /// Set the system message.
    pub fn system_message(mut self, message: impl Into<String>) -> Self {
        self.system_message = Some(message.into());
        self
    }

    /// Build the history.
    pub fn build(self) -> MessageHistory {
        let mut history = MessageHistory::with_config(self.config);

        if let Some(content) = self.system_message {
            history.set_system_message(TrackedMessage::system(content, "system"));
        }

        history
    }
}

impl Default for HistoryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_turn(number: i32) -> Turn {
        let user_msg = TrackedMessage::user(format!("Message {number}"), format!("turn-{number}"));
        let mut turn = Turn::new(number, user_msg);
        turn.set_assistant_message(TrackedMessage::assistant(
            format!("Response {number}"),
            format!("turn-{number}"),
            None,
        ));
        turn.update_usage(TokenUsage::new(10, 5));
        turn
    }

    #[test]
    fn test_empty_history() {
        let history = MessageHistory::new();
        assert_eq!(history.turn_count(), 0);
        assert!(history.current_turn().is_none());
    }

    #[test]
    fn test_add_turns() {
        let mut history = MessageHistory::new();

        history.add_turn(make_turn(1));
        assert_eq!(history.turn_count(), 1);

        history.add_turn(make_turn(2));
        assert_eq!(history.turn_count(), 2);
    }

    #[test]
    fn test_system_message() {
        let mut history = MessageHistory::new();
        history.set_system_message(TrackedMessage::system("You are helpful", "system"));

        let messages = history.messages_for_api();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, hyper_sdk::Role::System);
    }

    #[test]
    fn test_messages_for_api() {
        let mut history = MessageHistory::new();
        history.set_system_message(TrackedMessage::system("You are helpful", "system"));
        history.add_turn(make_turn(1));

        let messages = history.messages_for_api();
        // System + user + assistant
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn test_total_usage() {
        let mut history = MessageHistory::new();
        history.add_turn(make_turn(1));
        history.add_turn(make_turn(2));

        let usage = history.total_usage();
        assert_eq!(usage.input_tokens, 20);
        assert_eq!(usage.output_tokens, 10);
    }

    #[test]
    fn test_compaction() {
        let mut history = MessageHistory::new();
        for i in 1..=10 {
            history.add_turn(make_turn(i));
        }

        assert_eq!(history.turn_count(), 10);

        history.apply_compaction("Summary of turns 1-8".to_string(), 2, "turn-10", 5000);
        assert_eq!(history.turn_count(), 2);
        assert!(history.compacted_summary().is_some());

        // Verify compaction boundary
        let boundary = history.compaction_boundary().unwrap();
        assert_eq!(boundary.turn_id, "turn-10");
        assert_eq!(boundary.turn_number, 10);
        assert_eq!(boundary.turns_compacted, 8);
        assert_eq!(boundary.tokens_saved, 5000);
        assert!(boundary.timestamp_ms > 0);
    }

    #[test]
    fn test_builder() {
        let history = HistoryBuilder::new()
            .context_window(64000)
            .compaction_threshold(0.7)
            .max_turns(50)
            .system_message("You are helpful")
            .build();

        assert_eq!(history.config.context_window, 64000);
        assert_eq!(history.config.compaction_threshold, 0.7);
        assert_eq!(history.config.max_turns, 50);
        assert!(history.system_message.is_some());
    }

    #[test]
    fn test_needs_compaction_by_turns() {
        let config = HistoryConfig {
            max_turns: 5,
            auto_compact: true,
            ..Default::default()
        };
        let mut history = MessageHistory::with_config(config);

        for i in 1..=6 {
            history.add_turn(make_turn(i));
        }

        assert!(history.needs_compaction());
    }

    #[test]
    fn test_clear() {
        let mut history = MessageHistory::new();
        history.add_turn(make_turn(1));
        history.apply_compaction("Summary".to_string(), 1, "turn-1", 100);

        // Verify compaction was applied
        assert!(history.compacted_summary().is_some());
        assert!(history.compaction_boundary().is_some());

        history.clear();
        assert_eq!(history.turn_count(), 0);
        assert!(history.compacted_summary().is_none());
        assert!(history.compaction_boundary().is_none());
    }
}
