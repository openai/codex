//! Samples a hidden next-prompt prediction from an already-loaded session.
//!
//! This module owns the model-facing half of next-prompt suggestions. It reuses
//! the visible thread history, appends one synthetic user instruction, samples a
//! short assistant reply, and filters the reply into text that is safe to show as
//! composer ghost text. It intentionally does not create a child thread, expose
//! tools, mutate transcript state, or decide whether the TUI should render the
//! result.
//!
//! Suggestions are best-effort. The caller should treat `Ok(None)` as an
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
use codex_protocol::error::Result as CodexResult;
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
/// stable completed conversation boundary. Returning `Ok(None)` means there is
/// no suggestion worth showing, not that the request failed.
pub(crate) async fn suggest_next_prompt(
    sess: &Session,
    cancellation_token: CancellationToken,
) -> CodexResult<Option<String>> {
    if cancellation_token.is_cancelled() {
        tracing::debug!("next prompt suggestion skipped after cancellation");
        return Ok(None);
    }
    if !session_is_idle_for_suggestion(sess).await {
        return Ok(None);
    }

    let started_at = Instant::now();
    let mut turn_context = sess.new_lightweight_turn().await;
    prefer_fast_suggestion_profile(&mut turn_context);
    if !suggestion_prompt_fits_context_window(sess, &turn_context).await {
        return Ok(None);
    }

    let history = sess.clone_history().await;
    let history_snapshot = HistorySnapshot::from_history(&history);
    if has_unpaired_tool_flow(history.raw_items()) {
        tracing::debug!("next prompt suggestion skipped for incomplete tool flow");
        return Ok(None);
    }
    let mut prompt_input = history.for_prompt(&turn_context.model_info.input_modalities);
    if assistant_message_count(&prompt_input) < 2 {
        return Ok(None);
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
        return Ok(None);
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
            return Ok(None);
        }
        Err(codex_async_utils::CancelErr::Cancelled) => {
            tracing::debug!("next prompt suggestion canceled before sampling started");
            return Ok(None);
        }
    };
    let mut streamed_text = String::new();
    let mut completed_text = None;
    let mut latest_rate_limits = None;
    let sample_deadline = tokio::time::sleep(NEXT_PROMPT_SUGGESTION_SAMPLE_TIMEOUT);
    tokio::pin!(sample_deadline);
    let completed_response_id = loop {
        if !session_is_idle_for_suggestion(sess).await {
            return Ok(None);
        }
        let Some(event) = (tokio::select! {
            event = stream.next().or_cancel(&cancellation_token) => match event {
                Ok(event) => event,
                Err(codex_async_utils::CancelErr::Cancelled) => {
                    tracing::debug!("next prompt suggestion canceled while sampling");
                    return Ok(None);
                }
            },
            _ = tokio::time::sleep(Duration::from_millis(100)) => continue,
            _ = &mut sample_deadline => {
                tracing::debug!("next prompt suggestion timed out while sampling");
                return Ok(None);
            },
        }) else {
            tracing::debug!("next prompt suggestion stream closed before completion");
            return Ok(None);
        };
        let event = match event {
            Ok(event) => event,
            Err(err) => {
                tracing::debug!(
                    error = ?err,
                    "next prompt suggestion stream failed while sampling"
                );
                return Ok(None);
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
            ResponseEvent::Completed {
                response_id,
                token_usage,
                ..
            } => {
                let should_emit_token_count = token_usage.is_some() || latest_rate_limits.is_some();
                sess.record_token_usage_info(&turn_context, token_usage.as_ref())
                    .await;
                if should_emit_token_count {
                    let token_usage_info = sess.token_usage_info().await;
                    // App-server clients key usage updates by turn id. Attribute hidden
                    // suggestion sampling to the latest real turn instead of the ephemeral
                    // lightweight turn used to configure the request.
                    if let Some(turn_id) = sess
                        .reference_context_item()
                        .await
                        .and_then(|item| item.turn_id)
                    {
                        sess.send_event_raw(Event {
                            id: turn_id,
                            msg: EventMsg::TokenCount(TokenCountEvent {
                                info: token_usage_info,
                                rate_limits: latest_rate_limits,
                            }),
                        })
                        .await;
                    } else if let Some(rate_limits) = latest_rate_limits {
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
    if !session_history_matches_snapshot(sess, history_snapshot).await {
        tracing::debug!("next prompt suggestion skipped after history changed");
        return Ok(None);
    }
    if !session_is_idle_for_suggestion(sess).await {
        return Ok(None);
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
    Ok(suggestion)
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
    let model_context_window = turn_context.model_context_window();
    let estimated_token_count = sess.get_estimated_token_count(turn_context).await;
    if suggestion_prompt_has_headroom(estimated_token_count, model_context_window) {
        return true;
    }
    let Some(model_context_window) = model_context_window else {
        return true;
    };
    let Some(estimated_token_count) = estimated_token_count else {
        return true;
    };
    let suggestion_prompt_limit =
        model_context_window.saturating_sub(NEXT_PROMPT_SUGGESTION_TOKEN_HEADROOM);

    tracing::debug!(
        estimated_token_count,
        model_context_window,
        suggestion_prompt_limit,
        "next prompt suggestion skipped near context window"
    );
    false
}

fn suggestion_prompt_has_headroom(
    estimated_token_count: Option<i64>,
    model_context_window: Option<i64>,
) -> bool {
    let (Some(estimated_token_count), Some(model_context_window)) =
        (estimated_token_count, model_context_window)
    else {
        return true;
    };
    estimated_token_count
        < model_context_window.saturating_sub(NEXT_PROMPT_SUGGESTION_TOKEN_HEADROOM)
}

fn assistant_message_count(items: &[ResponseItem]) -> usize {
    items
        .iter()
        .filter(|item| matches!(item, ResponseItem::Message { role, .. } if role == "assistant"))
        .count()
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
            ResponseItem::ToolSearchOutput { execution, .. } if execution == "server" => {}
            ResponseItem::ToolSearchOutput {
                call_id: Some(call_id),
                ..
            } => {
                tool_search_outputs.insert(call_id.clone());
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
        || tool_search_calls != tool_search_outputs
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

#[cfg(test)]
mod tests {
    use super::HistorySnapshot;
    use super::filter_next_prompt_suggestion;
    use super::has_unpaired_tool_flow;
    use super::history_matches_snapshot;
    use super::suggestion_prompt_has_headroom;
    use crate::context_manager::ContextManager;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::models::ResponseItem;
    use codex_utils_output_truncation::TruncationPolicy;
    use pretty_assertions::assert_eq;

    #[test]
    fn filter_keeps_specific_prompt() {
        assert_eq!(
            filter_next_prompt_suggestion("run the tests"),
            Some("run the tests".to_string())
        );
    }

    #[test]
    fn filter_keeps_allowed_single_word_prompt() {
        assert_eq!(
            filter_next_prompt_suggestion("commit"),
            Some("commit".to_string())
        );
    }

    #[test]
    fn filter_keeps_code_identifier_prompt() {
        assert_eq!(
            filter_next_prompt_suggestion("set CODEX_HOME"),
            Some("set CODEX_HOME".to_string())
        );
    }

    #[test]
    fn filter_keeps_dotted_file_prompt() {
        assert_eq!(
            filter_next_prompt_suggestion("update Cargo.toml"),
            Some("update Cargo.toml".to_string())
        );
        assert_eq!(
            filter_next_prompt_suggestion("open app-server/README.md"),
            Some("open app-server/README.md".to_string())
        );
    }

    #[test]
    fn history_snapshot_detects_appends_and_rewrites() {
        let mut history = ContextManager::new();
        let snapshot = HistorySnapshot::from_history(&history);
        assert!(history_matches_snapshot(&history, snapshot));

        let item = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "next".to_string(),
            }],
            phase: None,
        };
        history.record_items([&item], TruncationPolicy::Tokens(10_000));
        assert!(!history_matches_snapshot(&history, snapshot));

        let appended_snapshot = HistorySnapshot::from_history(&history);
        history.replace(history.raw_items().to_vec());
        assert!(!history_matches_snapshot(&history, appended_snapshot));
    }

    #[test]
    fn suggestion_prompt_skips_near_context_window() {
        assert!(!suggestion_prompt_has_headroom(
            /*estimated_token_count*/ Some(127_100),
            /*model_context_window*/ Some(128_000)
        ));
    }

    #[test]
    fn incomplete_custom_tool_flow_is_suppressed() {
        assert!(has_unpaired_tool_flow(&[ResponseItem::CustomToolCall {
            id: None,
            status: None,
            call_id: "call-1".to_string(),
            name: "exec".to_string(),
            input: "{}".to_string(),
        }]));
    }

    #[test]
    fn completed_custom_tool_flow_is_allowed() {
        assert!(!has_unpaired_tool_flow(&[
            ResponseItem::CustomToolCall {
                id: None,
                status: None,
                call_id: "call-1".to_string(),
                name: "exec".to_string(),
                input: "{}".to_string(),
            },
            ResponseItem::CustomToolCallOutput {
                call_id: "call-1".to_string(),
                name: Some("exec".to_string()),
                output: FunctionCallOutputPayload::from_text("done".to_string()),
            },
        ]));
    }

    #[test]
    fn server_tool_search_output_without_call_is_allowed() {
        assert!(!has_unpaired_tool_flow(&[ResponseItem::ToolSearchOutput {
            call_id: Some("call-1".to_string()),
            status: "completed".to_string(),
            execution: "server".to_string(),
            tools: Vec::new(),
        }]));
    }

    #[test]
    fn client_tool_search_output_without_call_is_suppressed() {
        assert!(has_unpaired_tool_flow(&[ResponseItem::ToolSearchOutput {
            call_id: Some("call-1".to_string()),
            status: "completed".to_string(),
            execution: "client".to_string(),
            tools: Vec::new(),
        }]));
    }

    #[test]
    fn filter_rejects_invalid_prompts() {
        for suggestion in [
            "",
            "done",
            "Suggestion: run the tests",
            "(stay silent)",
            "looks good",
            "thanks",
            "let me run tests",
            "what about tests?",
            "run tests.",
            "run\ntests",
            "continue with every possible next step in this project and explain every detail now",
        ] {
            assert_eq!(
                filter_next_prompt_suggestion(suggestion),
                None,
                "expected {suggestion:?} to be filtered"
            );
        }
    }
}
