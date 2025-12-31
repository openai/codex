//! Full compact V2 implementation with LLM-based summarization.
//!
//! This module implements the 8-phase full compact flow:
//! 1. Validation & Setup
//! 2. Get compact client (reuses session client if no custom provider)
//! 3. Filter messages for summarization
//! 4. Generate summary via LLM
//! 5. Validate response
//! 6. Restore context
//! 7. Build new history
//! 8. Recompute tokens and emit events

use std::sync::Arc;
use std::time::Instant;

/// Default context window size when model context window is unknown.
pub const DEFAULT_CONTEXT_WINDOW: i64 = 200_000;

use crate::Prompt;
use crate::client::ModelClient;
use crate::client_common::ResponseEvent;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::error::CodexErr;
use crate::model_provider_info::ModelProviderInfo;
use crate::models_manager::model_family::find_family_for_model;
use crate::protocol::CompactCompletedEvent;
use crate::protocol::CompactFailedEvent;
use crate::protocol::CompactThresholdExceededEvent;
use crate::protocol::CompactedItem;
use crate::protocol::ContextCompactedEvent;
use crate::protocol::EventMsg;
use crate::protocol::ExtEventMsg;
use crate::protocol::WarningEvent;
use crate::util::backoff;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;
use futures::prelude::*;
use tracing::debug;
use tracing::error;
use tracing::info;

use super::CompactBoundary;
use super::CompactConfig;
use super::CompactMetrics;
use super::CompactStrategy;
use super::CompactTrigger;
use super::RestoredContext;
use super::SUMMARY_PREFIX_V2;
use super::TokenCounter;
use super::collect_user_message_texts;
use super::create_summary_message;
use super::filter_for_summarization;
use super::format_restored_context;
use super::generate_summarization_prompt;
use super::get_auto_compact_threshold;
use super::is_valid_summary;
use super::restore_context;

/// Run full compact V2 with LLM-based summarization.
///
/// This is the Tier 2 compaction strategy, used when micro-compact is
/// insufficient or disabled.
pub async fn run_full_compact_v2(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    config: &CompactConfig,
    is_auto_compact: bool,
) -> Result<CompactMetrics, CodexErr> {
    info!("Full compact V2 started (auto={})", is_auto_compact);
    let start_time = Instant::now();

    // Phase 1: Validation & Setup
    let history = sess.clone_history().await.get_history();
    if history.is_empty() {
        emit_compact_failed(
            &sess,
            &turn_context,
            "Empty history, nothing to compact",
            is_auto_compact,
        )
        .await;
        return Err(CodexErr::Fatal("Empty history, nothing to compact".into()));
    }
    let pre_tokens = sess.get_total_token_usage().await;
    let context_limit = turn_context
        .client
        .get_model_context_window()
        .unwrap_or(DEFAULT_CONTEXT_WINDOW);
    debug!(
        "Pre-compact tokens: {}, context_limit: {}",
        pre_tokens, context_limit
    );

    // Phase 2: Get compact client
    let compact_client = get_compact_client(&sess, &turn_context, config);

    // Phase 3: Filter messages for summarization
    let messages_to_summarize = CompactBoundary::extract_messages_after_boundary(&history);
    let filtered = filter_for_summarization(&messages_to_summarize);
    debug!("Filtered {} messages for summarization", filtered.len());

    if filtered.is_empty() {
        emit_compact_failed(
            &sess,
            &turn_context,
            "No messages to summarize after filtering",
            is_auto_compact,
        )
        .await;
        return Err(CodexErr::Fatal(
            "No messages to summarize after filtering".into(),
        ));
    }

    // Phase 4: Generate summary via LLM
    let prompt_text = generate_summarization_prompt(None);
    let summarization_result =
        call_summarization_llm(&compact_client, &prompt_text, &filtered, &turn_context).await?;
    let summary_text = &summarization_result.text;

    // Phase 5: Validate response
    if !is_valid_summary(summary_text) {
        emit_compact_failed(
            &sess,
            &turn_context,
            "Invalid summary response from LLM",
            is_auto_compact,
        )
        .await;
        return Err(CodexErr::Fatal("Invalid summary response from LLM".into()));
    }
    debug!("Summary generated: {} chars", summary_text.len());

    // Phase 6: Restore context
    let conversation_id = sess.conversation_id.to_string();
    let restored_context = restore_context(&conversation_id, config);
    debug!(
        "Context restored: {} files, todos={}, plan={}",
        restored_context.files.len(),
        restored_context.todos.is_some(),
        restored_context.plan.is_some()
    );

    // Phase 7: Build new history
    let boundary = CompactBoundary::create(
        if is_auto_compact {
            CompactTrigger::Auto
        } else {
            CompactTrigger::Manual
        },
        pre_tokens,
    );
    let mut new_history = build_compacted_history(
        sess.build_initial_context(turn_context.as_ref()),
        &summary_text,
        &restored_context,
        boundary,
        is_auto_compact,
        &messages_to_summarize,
        config,
    );

    // Preserve GhostSnapshots from original history (for undo capability)
    let ghost_snapshots: Vec<ResponseItem> = history
        .iter()
        .filter(|item| matches!(item, ResponseItem::GhostSnapshot { .. }))
        .cloned()
        .collect();
    new_history.extend(ghost_snapshots);

    sess.replace_history(new_history).await;

    // Phase 8: Recompute tokens and emit events
    sess.recompute_token_usage(&turn_context).await;
    let post_tokens = sess.get_total_token_usage().await;
    let duration_ms = start_time.elapsed().as_millis() as i64;

    // Persist rollout item for audit trail
    let rollout_item = RolloutItem::Compacted(CompactedItem {
        message: format!("{}\n{}", SUMMARY_PREFIX_V2, summary_text),
        replacement_history: None,
    });
    sess.persist_rollout_items(&[rollout_item]).await;

    // Emit legacy ContextCompacted event for backwards compatibility
    let event = EventMsg::ContextCompacted(ContextCompactedEvent {});
    sess.send_event(&turn_context, event).await;

    // Emit detailed CompactCompleted event
    let completed_event = EventMsg::Ext(ExtEventMsg::CompactCompleted(CompactCompletedEvent {
        pre_compact_tokens: pre_tokens,
        post_compact_tokens: post_tokens,
        compaction_input_tokens: summarization_result.input_tokens,
        compaction_output_tokens: summarization_result.output_tokens,
        files_restored: restored_context.files.len() as i32,
        duration_ms,
        is_auto: is_auto_compact,
    }));
    sess.send_event(&turn_context, completed_event).await;

    // Check if still above threshold after compact and emit warning
    let threshold = get_auto_compact_threshold(context_limit, config);
    if post_tokens >= threshold {
        let usage_percent = if context_limit > 0 {
            (post_tokens as f64 / context_limit as f64) * 100.0
        } else {
            0.0
        };
        let exceeded_event = EventMsg::Ext(ExtEventMsg::CompactThresholdExceeded(
            CompactThresholdExceededEvent {
                current_tokens: post_tokens,
                threshold_tokens: threshold,
                usage_percent,
            },
        ));
        sess.send_event(&turn_context, exceeded_event).await;
        tracing::warn!(
            "Post-compact tokens ({}) still exceed threshold ({})",
            post_tokens,
            threshold
        );
    }

    // Emit warning about multiple compactions
    let warning = EventMsg::Warning(WarningEvent {
        message: "Heads up: Long conversations and multiple compactions can cause the model to be less accurate. Start a new conversation when possible to keep conversations small and targeted.".to_string(),
    });
    sess.send_event(&turn_context, warning).await;

    info!(
        "Full compact V2 completed: {} -> {} tokens in {}ms",
        pre_tokens, post_tokens, duration_ms
    );

    Ok(CompactMetrics {
        pre_compact_tokens: pre_tokens,
        post_compact_tokens: post_tokens,
        strategy_used: CompactStrategy::FullCompact,
        files_restored: restored_context.files.len() as i32,
        duration_ms,
        compaction_input_tokens: summarization_result.input_tokens,
        compaction_output_tokens: summarization_result.output_tokens,
        ..Default::default()
    })
}

/// Get the ModelClient for compact operations.
///
/// Returns existing client if no custom provider, or creates new client if configured.
fn get_compact_client(
    sess: &Session,
    turn_context: &TurnContext,
    config: &CompactConfig,
) -> ModelClient {
    // If no custom compact provider, reuse session's client
    let Some(provider_id) = &config.compact_model_provider else {
        return turn_context.client.clone();
    };

    // Try to get custom provider from config
    let session_config = turn_context.client.config();
    let Some(provider) = session_config.model_providers.get(provider_id) else {
        tracing::warn!(
            "compact_model_provider '{}' not found in model_providers, using session client",
            provider_id
        );
        return turn_context.client.clone();
    };

    // Create new ModelClient with compact provider
    create_model_client_with_provider(sess, turn_context, provider)
}

/// Create a ModelClient with a specific provider for compact operations.
///
/// Reuses auth/otel infrastructure from the existing client.
fn create_model_client_with_provider(
    sess: &Session,
    turn_context: &TurnContext,
    provider: &ModelProviderInfo,
) -> ModelClient {
    let existing_client = &turn_context.client;
    let existing_config = existing_client.config();

    // Get model family for the provider's model
    // Use model_name from provider.ext if available, otherwise use existing client's model family
    let model_family = provider
        .ext
        .model_name
        .as_ref()
        .map(|name| find_family_for_model(name))
        .unwrap_or_else(|| existing_client.get_model_family());

    // Reuse session source from parent client
    let session_source = existing_client.get_session_source();

    ModelClient::new(
        existing_config,
        existing_client.get_auth_manager(),
        model_family,
        existing_client.get_otel_manager(),
        provider.clone(),
        None, // effort - None for compact
        existing_client.get_reasoning_summary(),
        sess.conversation_id,
        session_source,
    )
}

/// Call LLM for summarization with retry logic.
async fn call_summarization_llm(
    client: &ModelClient,
    prompt_text: &str,
    messages_to_summarize: &[ResponseItem],
    turn_context: &TurnContext,
) -> Result<SummarizationResult, CodexErr> {
    // Build the conversation to summarize as context
    let mut input_messages = messages_to_summarize.to_vec();

    // Add the summarization request as the final user message
    input_messages.push(ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: prompt_text.to_string(),
        }],
    });

    let prompt = Prompt {
        input: input_messages,
        ..Default::default()
    };

    // Retry logic with exponential backoff
    let max_retries = turn_context.client.get_provider().stream_max_retries();
    let mut retries = 0;

    loop {
        match stream_and_collect_response(client, &prompt).await {
            Ok(result) => return Ok(result),
            Err(CodexErr::ContextWindowExceeded) => {
                error!("Context window exceeded during compact summarization");
                return Err(CodexErr::ContextWindowExceeded);
            }
            Err(e) => {
                if retries < max_retries {
                    retries += 1;
                    let delay = backoff(retries);
                    tracing::warn!(
                        "Compact LLM call failed, retrying {}/{}: {}",
                        retries,
                        max_retries,
                        e
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(e);
            }
        }
    }
}

/// Result of LLM summarization including token usage.
struct SummarizationResult {
    text: String,
    input_tokens: i64,
    output_tokens: i64,
}

/// Stream LLM response and collect text with token usage.
async fn stream_and_collect_response(
    client: &ModelClient,
    prompt: &Prompt,
) -> Result<SummarizationResult, CodexErr> {
    let mut stream = client.clone().stream(prompt).await?;
    let mut response_text = String::new();
    let mut input_tokens: i64 = 0;
    let mut output_tokens: i64 = 0;

    loop {
        let event = stream.next().await;
        match event {
            Some(Ok(ResponseEvent::OutputItemDone(item))) => {
                if let Some(text) = extract_text_from_item(&item) {
                    response_text.push_str(&text);
                }
            }
            Some(Ok(ResponseEvent::Completed { token_usage, .. })) => {
                // Extract token usage from the completion event
                if let Some(usage) = token_usage {
                    input_tokens = usage.input_tokens;
                    output_tokens = usage.output_tokens;
                }
                break;
            }
            Some(Err(e)) => return Err(e),
            None => {
                return Err(CodexErr::Stream(
                    "Stream closed before response.completed".into(),
                    None,
                ));
            }
            _ => continue,
        }
    }

    Ok(SummarizationResult {
        text: response_text,
        input_tokens,
        output_tokens,
    })
}

/// Extract text content from a ResponseItem.
fn extract_text_from_item(item: &ResponseItem) -> Option<String> {
    match item {
        ResponseItem::Message { content, .. } => {
            let texts: Vec<String> = content
                .iter()
                .filter_map(|c| match c {
                    ContentItem::OutputText { text } => Some(text.clone()),
                    _ => None,
                })
                .collect();
            if texts.is_empty() {
                None
            } else {
                Some(texts.join(""))
            }
        }
        _ => None,
    }
}

/// Build the compacted history with boundary, summary, restored context, and preserved user messages.
fn build_compacted_history(
    initial_context: Vec<ResponseItem>,
    summary_text: &str,
    restored_context: &RestoredContext,
    boundary: ResponseItem,
    is_auto_compact: bool,
    original_messages: &[ResponseItem],
    config: &CompactConfig,
) -> Vec<ResponseItem> {
    let mut new_history = initial_context;

    // 1. Add boundary marker
    new_history.push(boundary);

    // 2. Add summary message
    new_history.push(create_summary_message(summary_text, is_auto_compact));

    // 3. Add restored context (files, todos, plan)
    let restored_messages = format_restored_context(restored_context);
    for msg in restored_messages {
        new_history.push(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: msg }],
        });
    }

    // 4. Preserve user messages up to token budget
    let user_messages = collect_user_message_texts(original_messages);
    if !user_messages.is_empty() && config.user_message_max_tokens > 0 {
        let token_counter = TokenCounter::from(config);
        let preserved = truncate_to_token_budget(
            &user_messages,
            &token_counter,
            config.user_message_max_tokens,
        );
        if !preserved.is_empty() {
            let preserved_text = format!(
                "<preserved_user_context>\n{}\n</preserved_user_context>",
                preserved.join("\n---\n")
            );
            new_history.push(ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: preserved_text,
                }],
            });
        }
    }

    new_history
}

/// Truncate user messages to fit within a token budget.
fn truncate_to_token_budget(
    messages: &[String],
    counter: &TokenCounter,
    max_tokens: i64,
) -> Vec<String> {
    let mut result = Vec::new();
    let mut total_tokens: i64 = 0;

    // Process messages in reverse order (most recent first)
    for msg in messages.iter().rev() {
        let msg_tokens = counter.approximate(msg);
        if total_tokens + msg_tokens <= max_tokens {
            result.push(msg.clone());
            total_tokens += msg_tokens;
        } else if result.is_empty() {
            // Always include at least a truncated version of the most recent message
            let truncated = truncate_text_to_tokens(msg, counter, max_tokens);
            result.push(truncated);
            break;
        } else {
            break;
        }
    }

    // Reverse back to original order
    result.reverse();
    result
}

/// Truncate a single text to fit within token limit.
fn truncate_text_to_tokens(text: &str, counter: &TokenCounter, max_tokens: i64) -> String {
    let chars_per_token = counter.bytes_per_token as i64;
    let max_chars = (max_tokens * chars_per_token) as usize;

    if text.len() <= max_chars {
        return text.to_string();
    }

    // Truncate and add ellipsis
    let truncated: String = text.chars().take(max_chars.saturating_sub(3)).collect();
    format!("{}...", truncated)
}

/// Emit a CompactFailed event.
async fn emit_compact_failed(
    sess: &Session,
    turn_context: &TurnContext,
    message: &str,
    is_auto: bool,
) {
    let event = EventMsg::Ext(ExtEventMsg::CompactFailed(CompactFailedEvent {
        message: message.to_string(),
        is_auto,
    }));
    sess.send_event(turn_context, event).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_text_from_output_item() {
        let item = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "Hello world".to_string(),
            }],
        };
        let text = extract_text_from_item(&item);
        assert_eq!(text, Some("Hello world".to_string()));
    }

    #[test]
    fn extract_text_from_non_message_returns_none() {
        let item = ResponseItem::Other;
        let text = extract_text_from_item(&item);
        assert_eq!(text, None);
    }

    #[test]
    fn build_compacted_history_includes_all_sections() {
        let initial = vec![ResponseItem::Message {
            id: None,
            role: "system".to_string(),
            content: vec![ContentItem::InputText {
                text: "System prompt".to_string(),
            }],
        }];

        let boundary = ResponseItem::Message {
            id: Some("compact_boundary_123".to_string()),
            role: "system".to_string(),
            content: vec![ContentItem::InputText {
                text: "Conversation compacted".to_string(),
            }],
        };

        let restored = RestoredContext {
            files: vec![],
            todos: None,
            plan: None,
        };

        let original_messages = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "User question".to_string(),
            }],
        }];
        let config = CompactConfig::default();

        let history = build_compacted_history(
            initial,
            "Summary text",
            &restored,
            boundary,
            true,
            &original_messages,
            &config,
        );

        // Should have: initial context + boundary + summary + preserved user context
        assert!(history.len() >= 4);
    }

    #[test]
    fn truncate_to_token_budget_respects_limit() {
        let counter = TokenCounter::default();
        let messages = vec![
            "Short msg".to_string(),
            "Another short one".to_string(),
            "Most recent message".to_string(),
        ];

        // With a very small budget (4 tokens), should only get most recent (truncated)
        // "Most recent message" = 19 chars → ~5 tokens, so 4 tokens means truncated
        let result = truncate_to_token_budget(&messages, &counter, 4);
        assert_eq!(result.len(), 1);

        // With large budget, should get all
        let result = truncate_to_token_budget(&messages, &counter, 1000);
        assert_eq!(result.len(), 3);
    }
}
