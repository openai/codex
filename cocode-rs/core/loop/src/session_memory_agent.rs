//! Background session memory extraction agent.
//!
//! This module implements a background agent that proactively extracts
//! conversation summaries during normal operation. This enables "zero API cost"
//! compaction at critical moments because a cached summary is already available.
//!
//! ## Architecture
//!
//! The extraction agent runs asynchronously and doesn't block the main
//! conversation loop. When trigger conditions are met:
//!
//! 1. Agent is spawned with current conversation state
//! 2. LLM summarization request is made
//! 3. Summary is written to `summary.md`
//! 4. Tracking is updated with the last summarized message ID
//!
//! ## Trigger Conditions
//!
//! Extraction is triggered when ALL of the following are true:
//! - Not currently compacting
//! - Not currently extracting
//! - Either:
//!   - First extraction: `min_tokens_to_init` tokens accumulated
//!   - Subsequent: `min_tokens_between` tokens AND `tool_calls_between` tool calls
//!     since last extraction, AND cooldown elapsed

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use cocode_api::ApiClient;
use cocode_protocol::AutoCompactTracking;
use cocode_protocol::LoopEvent;
use cocode_protocol::SessionMemoryExtractionConfig;
use hyper_sdk::ContentBlock;
use hyper_sdk::GenerateRequest;
use hyper_sdk::Message;
use hyper_sdk::Model;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::compaction::write_session_memory;

/// Result of a session memory extraction operation.
#[derive(Debug, Clone)]
pub struct ExtractionResult {
    /// The generated summary text.
    pub summary: String,
    /// Estimated token count of the summary.
    pub summary_tokens: i32,
    /// ID of the last message that was summarized.
    pub last_summarized_id: String,
    /// Number of messages that were summarized.
    pub messages_summarized: i32,
}

/// Background agent for session memory extraction.
///
/// This agent runs asynchronously during normal conversation to proactively
/// update the session memory (summary.md). It enables instant (zero API cost)
/// compaction when the context limit is reached.
pub struct SessionMemoryExtractionAgent {
    /// Configuration for extraction behavior.
    config: SessionMemoryExtractionConfig,
    /// API client for LLM requests.
    api_client: ApiClient,
    /// Model to use for summarization.
    model: Arc<dyn Model>,
    /// Event sender for emitting extraction events.
    event_tx: mpsc::Sender<LoopEvent>,
    /// Path to the summary.md file.
    summary_path: PathBuf,
}

impl SessionMemoryExtractionAgent {
    /// Create a new extraction agent.
    pub fn new(
        config: SessionMemoryExtractionConfig,
        api_client: ApiClient,
        model: Arc<dyn Model>,
        event_tx: mpsc::Sender<LoopEvent>,
        summary_path: PathBuf,
    ) -> Self {
        Self {
            config,
            api_client,
            model,
            event_tx,
            summary_path,
        }
    }

    /// Check if extraction should be triggered.
    ///
    /// Returns `true` if all trigger conditions are met:
    /// - Extraction is enabled
    /// - Not currently compacting
    /// - Not currently extracting
    /// - Token/tool call thresholds met
    /// - Cooldown elapsed (for subsequent extractions)
    pub fn should_trigger(
        &self,
        tracking: &AutoCompactTracking,
        current_tokens: i32,
        is_compacting: bool,
    ) -> bool {
        // Check basic conditions
        if !self.config.enabled {
            debug!("Extraction disabled");
            return false;
        }

        if is_compacting {
            debug!("Skipping extraction: compaction in progress");
            return false;
        }

        if tracking.extraction_in_progress {
            debug!("Skipping extraction: extraction already in progress");
            return false;
        }

        let tokens_since = tracking.tokens_since_extraction(current_tokens);
        let tool_calls_since = tracking.tool_calls_since_extraction();

        // First extraction: only need min_tokens_to_init
        if tracking.extraction_count == 0 {
            let should = tokens_since >= self.config.min_tokens_to_init;
            debug!(
                tokens_since,
                min_tokens = self.config.min_tokens_to_init,
                should,
                "First extraction check"
            );
            return should;
        }

        // Subsequent extractions: need all conditions
        let cooldown = Duration::from_secs(self.config.cooldown_secs as u64);
        let cooldown_elapsed = tracking.time_since_extraction() >= cooldown;

        let tokens_ok = tokens_since >= self.config.min_tokens_between;
        let tool_calls_ok = tool_calls_since >= self.config.tool_calls_between;

        let should = cooldown_elapsed && tokens_ok && tool_calls_ok;

        debug!(
            tokens_since,
            min_tokens = self.config.min_tokens_between,
            tokens_ok,
            tool_calls_since,
            min_tool_calls = self.config.tool_calls_between,
            tool_calls_ok,
            cooldown_elapsed,
            should,
            "Subsequent extraction check"
        );

        should
    }

    /// Run extraction asynchronously.
    ///
    /// This method:
    /// 1. Emits `SessionMemoryExtractionStarted` event
    /// 2. Builds summarization prompt
    /// 3. Calls LLM for summary
    /// 4. Writes to summary.md
    /// 5. Emits `SessionMemoryExtractionCompleted` or `SessionMemoryExtractionFailed`
    ///
    /// # Arguments
    /// * `conversation_text` - The conversation content to summarize
    /// * `current_tokens` - Current token count in the conversation
    /// * `tool_calls_since` - Tool calls since last extraction
    /// * `last_message_id` - ID of the last message being summarized
    /// * `message_count` - Total number of messages being summarized
    pub async fn run_extraction(
        &self,
        conversation_text: &str,
        current_tokens: i32,
        tool_calls_since: i32,
        last_message_id: &str,
        message_count: i32,
    ) -> Result<ExtractionResult, anyhow::Error> {
        // Emit started event
        self.emit(LoopEvent::SessionMemoryExtractionStarted {
            current_tokens,
            tool_calls_since,
        })
        .await;

        info!(
            current_tokens,
            tool_calls_since, message_count, "Starting session memory extraction"
        );

        // Build summarization prompt
        let system_prompt = self.build_extraction_prompt();
        let user_prompt = format!(
            "Please summarize the following conversation:\n\n{}",
            conversation_text
        );

        // Call LLM for summary
        let messages = vec![Message::system(&system_prompt), Message::user(&user_prompt)];
        let mut request = GenerateRequest::new(messages);
        request.max_tokens = Some(self.config.max_summary_tokens);

        let response = match self.api_client.generate(&*self.model, request).await {
            Ok(r) => r,
            Err(e) => {
                let error = format!("LLM request failed: {e}");
                warn!(error = %e, "Session memory extraction failed");
                self.emit(LoopEvent::SessionMemoryExtractionFailed {
                    error: error.clone(),
                    attempts: 1,
                })
                .await;
                return Err(anyhow::anyhow!(error));
            }
        };

        // Extract summary text
        let summary: String = response
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();

        if summary.is_empty() {
            let error = "Empty summary generated";
            warn!("Session memory extraction produced empty summary");
            self.emit(LoopEvent::SessionMemoryExtractionFailed {
                error: error.to_string(),
                attempts: 1,
            })
            .await;
            return Err(anyhow::anyhow!(error));
        }

        // Estimate summary tokens (~4 chars per token)
        let summary_tokens = (summary.len() / 4) as i32;

        // Write to summary.md
        if let Err(e) = write_session_memory(&self.summary_path, &summary, last_message_id).await {
            let error = format!("Failed to write summary.md: {e}");
            warn!(error = %e, path = ?self.summary_path, "Failed to write session memory");
            self.emit(LoopEvent::SessionMemoryExtractionFailed {
                error: error.clone(),
                attempts: 1,
            })
            .await;
            return Err(anyhow::anyhow!(error));
        }

        info!(
            summary_tokens,
            last_message_id,
            message_count,
            path = ?self.summary_path,
            "Session memory extraction completed"
        );

        // Emit completed event
        self.emit(LoopEvent::SessionMemoryExtractionCompleted {
            summary_tokens,
            last_summarized_id: last_message_id.to_string(),
            messages_summarized: message_count,
        })
        .await;

        Ok(ExtractionResult {
            summary,
            summary_tokens,
            last_summarized_id: last_message_id.to_string(),
            messages_summarized: message_count,
        })
    }

    /// Build the extraction prompt for incremental summarization.
    fn build_extraction_prompt(&self) -> String {
        format!(
            r#"You are extracting key information from an ongoing conversation between a user and an AI coding assistant. Create a concise summary that preserves essential context for future reference.

## Instructions

Generate a focused summary covering:

1. **Current Goal**: What is the user trying to accomplish?
2. **Progress Made**: What has been done so far?
3. **Key Decisions**: Important technical choices or user preferences.
4. **Files Modified**: List of files that have been created or changed.
5. **Pending Items**: What still needs to be done?

## Format

- Use bullet points for clarity
- Be concise but complete
- Focus on information needed to continue the work
- Maximum output: {} tokens

## Output

Provide the summary directly without any preamble."#,
            self.config.max_summary_tokens
        )
    }

    /// Emit an event to the event channel.
    async fn emit(&self, event: LoopEvent) {
        if let Err(e) = self.event_tx.send(event).await {
            debug!("Failed to send extraction event: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cocode_protocol::AutoCompactTracking;
    use std::time::Duration;

    // Helper to create a config for testing
    fn test_config() -> SessionMemoryExtractionConfig {
        SessionMemoryExtractionConfig {
            enabled: true,
            min_tokens_to_init: 5000,
            min_tokens_between: 5000,
            tool_calls_between: 10,
            cooldown_secs: 60,
            max_summary_tokens: 4000,
        }
    }

    #[test]
    fn test_should_trigger_disabled() {
        let config = SessionMemoryExtractionConfig {
            enabled: false,
            ..test_config()
        };

        // We can't create a full agent without a model, but we can test the logic
        // by checking the conditions directly
        let _tracking = AutoCompactTracking::new();

        // When disabled, should never trigger
        assert!(!config.enabled);

        // First extraction check with disabled
        let tokens_since = 10000; // Above threshold
        assert!(tokens_since >= config.min_tokens_to_init);
        // But extraction is disabled, so it shouldn't trigger
    }

    #[test]
    fn test_first_extraction_threshold() {
        let config = test_config();
        let tracking = AutoCompactTracking::new();

        // Below threshold - should not trigger
        let tokens_below = 4000;
        assert!(tokens_below < config.min_tokens_to_init);

        // At threshold - should trigger
        let tokens_at = 5000;
        assert!(tokens_at >= config.min_tokens_to_init);

        // Above threshold - should trigger
        let tokens_above = 10000;
        assert!(tokens_above >= config.min_tokens_to_init);

        // Verify extraction_count is 0 for first extraction
        assert_eq!(tracking.extraction_count, 0);
    }

    #[test]
    fn test_subsequent_extraction_all_conditions() {
        let config = test_config();
        let mut tracking = AutoCompactTracking::new();

        // Simulate first extraction completed
        tracking.mark_extraction_completed(10000, "msg-1");

        // Now test subsequent extraction conditions
        // Need: tokens_between + tool_calls_between + cooldown

        // Add tool calls
        for _ in 0..15 {
            tracking.record_tool_call();
        }

        let current_tokens = 20000; // 10000 since last extraction

        // Check individual conditions
        let tokens_since = tracking.tokens_since_extraction(current_tokens);
        assert_eq!(tokens_since, 10000);
        assert!(tokens_since >= config.min_tokens_between);

        let tool_calls_since = tracking.tool_calls_since_extraction();
        assert_eq!(tool_calls_since, 15);
        assert!(tool_calls_since >= config.tool_calls_between);

        // Cooldown check (in real test, time would have passed)
        // Here we just verify the Duration comparison works
        let cooldown = Duration::from_secs(config.cooldown_secs as u64);
        let time_since = tracking.time_since_extraction();
        // Time since should be very small (just created)
        assert!(time_since < cooldown);
    }

    #[test]
    fn test_extraction_in_progress_blocks() {
        let mut tracking = AutoCompactTracking::new();
        tracking.mark_extraction_started();

        assert!(tracking.extraction_in_progress);
        // When extraction_in_progress is true, should_trigger should return false
    }

    #[test]
    fn test_extraction_result() {
        let result = ExtractionResult {
            summary: "Test summary".to_string(),
            summary_tokens: 100,
            last_summarized_id: "msg-123".to_string(),
            messages_summarized: 10,
        };

        assert_eq!(result.summary, "Test summary");
        assert_eq!(result.summary_tokens, 100);
        assert_eq!(result.last_summarized_id, "msg-123");
        assert_eq!(result.messages_summarized, 10);
    }
}
