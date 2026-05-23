//! Samples a hidden next-prompt prediction from an already-loaded session.
//!
//! This module owns the model-facing half of next-prompt suggestions. It reuses
//! the visible thread history, appends one synthetic user instruction, samples a
//! short assistant reply, and filters the reply into text that is safe to show as
//! composer ghost text. It intentionally does not create a child thread, expose
//! tools, mutate transcript state, or decide whether the TUI should render the
//! result.
//!
//! Suggestions are best-effort. The caller should treat `None` as an
//! expected silent outcome for early conversations, active turns, incomplete
//! tool flow, model silence, or filtered output.

use crate::TurnContext;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::context::ContextualUserFragment;
use crate::context::NextPromptSuggestionInstructions;
use crate::context_manager::ContextManager;
use crate::session::session::Session;
use codex_async_utils::OrCancelExt;
use codex_features::Feature;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TokenCountEvent;
use codex_rollout_trace::InferenceTraceContext;
use futures::StreamExt;
use std::collections::HashSet;
use std::time::Duration;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[cfg(test)]
#[path = "next_prompt_suggestion_tests.rs"]
mod next_prompt_suggestion_tests;

const NEXT_PROMPT_SUGGESTION_TOKEN_HEADROOM: i64 = 1_024;
const NEXT_PROMPT_SUGGESTION_SAMPLE_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone, Copy)]
struct HistorySnapshot {
    version: u64,
    len: usize,
}

/// Predicts the user's likely next prompt without mutating the session.
///
/// The sample uses the prompt-visible history from `sess` plus one synthetic
/// suggestion instruction. Active turns and histories with unmatched tool call
/// pairs are suppressed before sampling because those states do not represent a
/// stable completed conversation boundary. Returning `None` means there is
/// no suggestion worth showing, not that the request failed.
pub(crate) async fn suggest_next_prompt(
    sess: &Session,
    cancellation_token: CancellationToken,
) -> Option<String> {
    if cancellation_token.is_cancelled() {
        tracing::debug!("next prompt suggestion skipped after cancellation");
        return None;
    }
    if !session_is_idle_for_suggestion(sess).await {
        return None;
    }

    let started_at = Instant::now();
    let mut turn_context = sess.new_lightweight_turn().await;
    prefer_fast_suggestion_profile(&mut turn_context);
    if !suggestion_prompt_fits_context_window(sess, &turn_context).await {
        return None;
    }

    let history = sess.clone_history().await;
    let history_snapshot = HistorySnapshot::from_history(&history);
    if has_unpaired_tool_flow(history.raw_items()) {
        tracing::debug!("next prompt suggestion skipped for incomplete tool flow");
        return None;
    }
    let mut prompt_input = history.for_prompt(&turn_context.model_info.input_modalities);
    if !history_ends_at_assistant_response(&prompt_input) {
        tracing::debug!("next prompt suggestion skipped before assistant boundary");
        return None;
    }
    if assistant_message_count(&prompt_input) < 2 {
        return None;
    }
    prompt_input.push(ContextualUserFragment::into(
        NextPromptSuggestionInstructions,
    ));

    let prompt = Prompt {
        input: prompt_input,
        tools: Vec::new(),
        parallel_tool_calls: false,
        base_instructions: sess.get_base_instructions().await,
        personality: turn_context.personality,
        output_schema: None,
        output_schema_strict: true,
    };
    if !session_is_idle_for_suggestion(sess).await {
        return None;
    }
    let mut client_session = sess.services.model_client.new_session();
    let mut stream = match client_session
        .stream(
            &prompt,
            &turn_context.model_info,
            &turn_context.session_telemetry,
            turn_context.reasoning_effort,
            turn_context.reasoning_summary,
            turn_context.config.service_tier.clone(),
            /*turn_metadata_header*/ None,
            &InferenceTraceContext::disabled(),
        )
        .or_cancel(&cancellation_token)
        .await
    {
        Ok(Ok(stream)) => stream,
        Ok(Err(err)) => {
            tracing::debug!(
                error = ?err,
                "next prompt suggestion failed before sampling started"
            );
            return None;
        }
        Err(codex_async_utils::CancelErr::Cancelled) => {
            tracing::debug!("next prompt suggestion canceled before sampling started");
            return None;
        }
    };
    let mut streamed_text = String::new();
    let mut completed_text = None;
    let mut latest_rate_limits = None;
    let sample_deadline = tokio::time::sleep(NEXT_PROMPT_SUGGESTION_SAMPLE_TIMEOUT);
    tokio::pin!(sample_deadline);
    let completed_response_id = loop {
        if !session_is_idle_for_suggestion(sess).await {
            client_session.reset_websocket_session();
            return None;
        }
        let Some(event) = (tokio::select! {
            event = stream.next().or_cancel(&cancellation_token) => match event {
                Ok(event) => event,
                Err(codex_async_utils::CancelErr::Cancelled) => {
                    tracing::debug!("next prompt suggestion canceled while sampling");
                    client_session.reset_websocket_session();
                    return None;
                }
            },
            _ = tokio::time::sleep(Duration::from_millis(100)) => continue,
            _ = &mut sample_deadline => {
                tracing::debug!("next prompt suggestion timed out while sampling");
                client_session.reset_websocket_session();
                return None;
            },
        }) else {
            tracing::debug!("next prompt suggestion stream closed before completion");
            client_session.reset_websocket_session();
            return None;
        };
        let event = match event {
            Ok(event) => event,
            Err(err) => {
                tracing::debug!(
                    error = ?err,
                    "next prompt suggestion stream failed while sampling"
                );
                client_session.reset_websocket_session();
                return None;
            }
        };
        match event {
            ResponseEvent::OutputItemDone(item) => {
                if let Some(text) = assistant_output_text(&item) {
                    completed_text = Some(text);
                }
            }
            ResponseEvent::OutputTextDelta(delta) => streamed_text.push_str(&delta),
            ResponseEvent::RateLimits(snapshot) => {
                latest_rate_limits = Some(snapshot.clone());
                sess.record_rate_limits_info(snapshot).await;
            }
            ResponseEvent::Completed { response_id, .. } => {
                if let Some(rate_limits) = latest_rate_limits {
                    // App-server clients key updates by turn id. Attribute hidden
                    // suggestion rate limits to the latest real turn instead of the
                    // ephemeral lightweight turn used to configure the request.
                    if let Some(turn_id) = sess
                        .reference_context_item()
                        .await
                        .and_then(|item| item.turn_id)
                    {
                        sess.deliver_event_raw(Event {
                            id: turn_id,
                            msg: EventMsg::TokenCount(TokenCountEvent {
                                info: None,
                                rate_limits: Some(rate_limits),
                            }),
                        })
                        .await;
                    } else {
                        sess.send_event(
                            &turn_context,
                            EventMsg::TokenCount(TokenCountEvent {
                                info: None,
                                rate_limits: Some(rate_limits),
                            }),
                        )
                        .await;
                    }
                }
                break response_id;
            }
            _ => {}
        }
    };
    if sess.enabled(Feature::ResponsesWebsocketResponseProcessed) {
        client_session
            .send_response_processed(&completed_response_id)
            .await;
    }
    client_session.reset_websocket_session();
    if !session_history_matches_snapshot(sess, history_snapshot).await {
        tracing::debug!("next prompt suggestion skipped after history changed");
        return None;
    }
    if !session_is_idle_for_suggestion(sess).await {
        return None;
    }

    let raw = completed_text.unwrap_or(streamed_text);
    let suggestion = filter_next_prompt_suggestion(&raw);
    tracing::debug!(
        latency_ms = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
        model = %turn_context.model_info.slug,
        effort = ?turn_context.reasoning_effort,
        service_tier = ?turn_context.config.service_tier,
        has_suggestion = suggestion.is_some(),
        "next prompt suggestion sampled"
    );
    suggestion
}

impl HistorySnapshot {
    fn from_history(history: &ContextManager) -> Self {
        Self {
            version: history.history_version(),
            len: history.raw_items().len(),
        }
    }
}

async fn session_is_idle_for_suggestion(sess: &Session) -> bool {
    if sess.active_turn.lock().await.is_some() {
        tracing::debug!("next prompt suggestion skipped while a turn is active");
        return false;
    }
    true
}

async fn session_history_matches_snapshot(sess: &Session, snapshot: HistorySnapshot) -> bool {
    let history = sess.clone_history().await;
    history_matches_snapshot(&history, snapshot)
}

fn history_matches_snapshot(history: &ContextManager, snapshot: HistorySnapshot) -> bool {
    history.history_version() == snapshot.version && history.raw_items().len() == snapshot.len
}

async fn suggestion_prompt_fits_context_window(sess: &Session, turn_context: &TurnContext) -> bool {
    let Some(model_context_window) = turn_context.model_context_window() else {
        tracing::debug!("next prompt suggestion skipped without model context window");
        return false;
    };
    if let Some(estimated_token_count) = sess.get_estimated_token_count(turn_context).await
        && !suggestion_prompt_has_headroom(estimated_token_count, model_context_window)
    {
        let suggestion_prompt_limit =
            model_context_window.saturating_sub(NEXT_PROMPT_SUGGESTION_TOKEN_HEADROOM);
        tracing::debug!(
            estimated_token_count,
            model_context_window,
            suggestion_prompt_limit,
            "next prompt suggestion skipped near context window"
        );
        return false;
    }
    true
}

fn suggestion_prompt_has_headroom(estimated_token_count: i64, model_context_window: i64) -> bool {
    estimated_token_count
        < model_context_window.saturating_sub(NEXT_PROMPT_SUGGESTION_TOKEN_HEADROOM)
}

fn assistant_message_count(items: &[ResponseItem]) -> usize {
    items
        .iter()
        .filter(|item| matches!(item, ResponseItem::Message { role, .. } if role == "assistant"))
        .count()
}

fn history_ends_at_assistant_response(items: &[ResponseItem]) -> bool {
    matches!(items.last(), Some(ResponseItem::Message { role, .. }) if role == "assistant")
}

fn assistant_output_text(item: &ResponseItem) -> Option<String> {
    let ResponseItem::Message { role, content, .. } = item else {
        return None;
    };
    if role != "assistant" {
        return None;
    }
    let text = content
        .iter()
        .filter_map(|content| match content {
            ContentItem::OutputText { text } => Some(text.as_str()),
            ContentItem::InputText { .. } | ContentItem::InputImage { .. } => None,
        })
        .collect::<String>();
    (!text.is_empty()).then_some(text)
}

/// Reports whether prompt-visible tool calls are missing their corresponding outputs.
///
/// Resume can expose a transcript while a prior tool flow is still incomplete.
/// Sampling that history would either produce malformed input or predict from a
/// boundary the user has not actually seen completed yet, so those sessions stay
/// silent until the call/output sets match again.
fn has_unpaired_tool_flow(items: &[ResponseItem]) -> bool {
    let mut function_calls = HashSet::new();
    let mut function_outputs = HashSet::new();
    let mut custom_tool_calls = HashSet::new();
    let mut custom_tool_outputs = HashSet::new();
    let mut tool_search_calls = HashSet::new();
    let mut tool_search_outputs = HashSet::new();
    let mut client_tool_search_outputs = HashSet::new();

    for item in items {
        match item {
            ResponseItem::FunctionCall { call_id, .. } => {
                function_calls.insert(call_id.clone());
            }
            ResponseItem::FunctionCallOutput { call_id, .. } => {
                function_outputs.insert(call_id.clone());
            }
            ResponseItem::ToolSearchCall {
                call_id: Some(call_id),
                ..
            } => {
                tool_search_calls.insert(call_id.clone());
            }
            ResponseItem::ToolSearchOutput {
                call_id: Some(call_id),
                execution,
                ..
            } => {
                tool_search_outputs.insert(call_id.clone());
                if execution != "server" {
                    client_tool_search_outputs.insert(call_id.clone());
                }
            }
            ResponseItem::CustomToolCall { call_id, .. } => {
                custom_tool_calls.insert(call_id.clone());
            }
            ResponseItem::CustomToolCallOutput { call_id, .. } => {
                custom_tool_outputs.insert(call_id.clone());
            }
            ResponseItem::LocalShellCall {
                call_id: Some(call_id),
                ..
            } => {
                function_calls.insert(call_id.clone());
            }
            ResponseItem::Message { .. }
            | ResponseItem::Reasoning { .. }
            | ResponseItem::ToolSearchCall { call_id: None, .. }
            | ResponseItem::ToolSearchOutput { call_id: None, .. }
            | ResponseItem::WebSearchCall { .. }
            | ResponseItem::ImageGenerationCall { .. }
            | ResponseItem::LocalShellCall { call_id: None, .. }
            | ResponseItem::Compaction { .. }
            | ResponseItem::CompactionTrigger
            | ResponseItem::ContextCompaction { .. }
            | ResponseItem::Other => {}
        }
    }

    function_calls != function_outputs
        || custom_tool_calls != custom_tool_outputs
        || !tool_search_calls.is_subset(&tool_search_outputs)
        || !client_tool_search_outputs.is_subset(&tool_search_calls)
}

/// Selects the fastest supported reasoning effort for an ephemeral suggestion sample.
///
/// This only adjusts the cloned lightweight turn context. It does not change the
/// parent thread's configured model, reasoning effort, or service tier.
fn prefer_fast_suggestion_profile(turn_context: &mut std::sync::Arc<crate::TurnContext>) {
    let Some(turn_context) = std::sync::Arc::get_mut(turn_context) else {
        return;
    };
    if let Some(preset) = turn_context
        .available_models
        .iter()
        .find(|preset| preset.model == turn_context.model_info.slug)
    {
        turn_context.reasoning_effort =
            preferred_suggestion_effort(preset, turn_context.reasoning_effort);
    }
}

fn preferred_suggestion_effort(
    preset: &ModelPreset,
    fallback: Option<ReasoningEffort>,
) -> Option<ReasoningEffort> {
    if preset_supports_effort(preset, ReasoningEffort::Minimal) {
        return Some(ReasoningEffort::Minimal);
    }
    if preset_supports_effort(preset, ReasoningEffort::Low) {
        return Some(ReasoningEffort::Low);
    }
    fallback
        .filter(|effort| preset_supports_effort(preset, *effort))
        .or(Some(preset.default_reasoning_effort))
}

fn preset_supports_effort(preset: &ModelPreset, effort: ReasoningEffort) -> bool {
    preset
        .supported_reasoning_efforts
        .iter()
        .any(|supported| supported.effort == effort)
}

/// Converts raw model text into a single composer-safe prompt candidate.
///
/// The model is allowed to stay silent. Formatting, meta labels, evaluative
/// replies, assistant-voice phrasing, and sentence-like outputs are rejected so
/// the UI only receives concise text the user could plausibly type verbatim.
fn filter_next_prompt_suggestion(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let suggestion = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if suggestion.is_empty()
        || raw.chars().any(|ch| matches!(ch, '\n' | '\r' | '\t'))
        || suggestion.len() >= 100
        || suggestion.chars().any(|ch| matches!(ch, '?' | '!'))
        || suggestion.ends_with('.')
        || suggestion.chars().any(|ch| matches!(ch, '`' | '*'))
        || suggestion.starts_with("- ")
    {
        return None;
    }

    let lower = suggestion.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "done" | "no suggestion" | "stay silent" | "silence"
    ) || lower.starts_with("suggestion:")
        || lower.starts_with("next prompt:")
        || is_wrapped_meta(&suggestion)
        || starts_with_any(&lower, &["looks good", "thanks", "thank you"])
        || starts_with_any(&lower, &["let me", "i'll", "i will", "here's"])
    {
        return None;
    }

    let word_count = suggestion.split_whitespace().count();
    if word_count > 12
        || (word_count < 2 && !matches!(lower.as_str(), "yes" | "commit" | "push" | "continue"))
    {
        return None;
    }

    Some(suggestion)
}

fn is_wrapped_meta(suggestion: &str) -> bool {
    (suggestion.starts_with('(') && suggestion.ends_with(')'))
        || (suggestion.starts_with('[') && suggestion.ends_with(']'))
}

fn starts_with_any(value: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| value.starts_with(prefix))
}
