use std::sync::Arc;

use crate::ModelProviderInfo;
use crate::Prompt;
use crate::client::ModelClientSession;
use crate::client_common::ResponseEvent;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex::get_last_assistant_message_from_turn;
use crate::context_manager::ContextManager;
use crate::context_manager::is_user_turn_boundary;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::protocol::CompactedItem;
use crate::protocol::EventMsg;
use crate::protocol::TurnStartedEvent;
use crate::protocol::WarningEvent;
use crate::truncate::TruncationPolicy;
use crate::truncate::approx_token_count;
use crate::truncate::truncate_text;
use crate::user_shell_command::is_user_shell_command_text;
use crate::util::backoff;
use codex_protocol::items::ContextCompactionItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::user_input::UserInput;
use futures::prelude::*;
use tracing::error;

pub const SUMMARIZATION_PROMPT: &str = include_str!("../templates/compact/prompt.md");
pub const SUMMARY_PREFIX: &str = include_str!("../templates/compact/summary_prefix.md");
const COMPACT_USER_MESSAGE_MAX_TOKENS: usize = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompactCallsite {
    /// Manual `/compact` task.
    ManualCompact,
    /// Pre-turn auto-compaction where the incoming turn context + user message are included in
    /// the compaction request.
    PreTurnIncludingIncomingUserMessage,
    /// Reserved pre-turn auto-compaction strategy that compacts from the end of the previous turn
    /// only, excluding incoming turn context + user message. This is currently unused by the
    /// default pre-turn flow and retained for future model-specific strategies.
    #[allow(dead_code)]
    PreTurnExcludingIncomingUserMessage,
    /// Pre-sampling compaction triggered by model switch to a smaller context window.
    /// This compacts prior-turn history only and should reinsert previous-turn canonical context.
    PreSamplingModelSwitch,
    /// Mid-turn compaction between assistant responses in a follow-up loop.
    MidTurnContinuation,
}

pub(crate) fn should_use_remote_compact_task(provider: &ModelProviderInfo) -> bool {
    provider.is_openai()
}

pub(crate) fn extract_trailing_model_switch_update_for_compaction_request(
    history: &mut ContextManager,
) -> Option<ResponseItem> {
    let history_items = history.raw_items();
    let last_user_turn_boundary_index = history_items
        .iter()
        .rposition(crate::context_manager::is_user_turn_boundary);
    let model_switch_index = history_items
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, item)| {
            let is_trailing = last_user_turn_boundary_index.is_none_or(|boundary| i > boundary);
            if is_trailing && Session::is_model_switch_developer_message(item) {
                Some(i)
            } else {
                None
            }
        })?;
    let mut replacement = history_items.to_vec();
    let model_switch_item = replacement.remove(model_switch_index);
    history.replace(replacement);
    Some(model_switch_item)
}

pub(crate) fn extract_latest_model_switch_update_from_items(
    items: &mut Vec<ResponseItem>,
) -> Option<ResponseItem> {
    let model_switch_index = items
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, item)| Session::is_model_switch_developer_message(item).then_some(i))?;
    Some(items.remove(model_switch_index))
}

pub(crate) async fn run_inline_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    compact_callsite: CompactCallsite,
    incoming_items: Option<Vec<ResponseItem>>,
) -> CodexResult<()> {
    let prompt = turn_context.compact_prompt().to_string();
    let input = vec![UserInput::Text {
        text: prompt,
        // Compaction prompt is synthesized; no UI element ranges to preserve.
        text_elements: Vec::new(),
    }];

    run_compact_task_inner(sess, turn_context, input, compact_callsite, incoming_items).await?;
    Ok(())
}

pub(crate) async fn run_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    input: Vec<UserInput>,
) -> CodexResult<()> {
    let start_event = EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: turn_context.sub_id.clone(),
        model_context_window: turn_context.model_context_window(),
        collaboration_mode_kind: turn_context.collaboration_mode.mode,
    });
    sess.send_event(&turn_context, start_event).await;
    run_compact_task_inner(
        sess,
        turn_context,
        input,
        CompactCallsite::ManualCompact,
        None,
    )
    .await
}

async fn run_compact_task_inner(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    input: Vec<UserInput>,
    compact_callsite: CompactCallsite,
    incoming_items: Option<Vec<ResponseItem>>,
) -> CodexResult<()> {
    let compaction_item = TurnItem::ContextCompaction(ContextCompactionItem::new());
    sess.emit_turn_item_started(&turn_context, &compaction_item)
        .await;
    let initial_input_for_turn: ResponseInputItem = ResponseInputItem::from(input);

    let mut history = sess.clone_history().await;
    let mut incoming_items = incoming_items;
    // Keep compaction prompts in-distribution: if a model-switch update was injected at the
    // tail of incoming turn items (pre-turn path) or between turns in history, exclude it from
    // the compaction request payload.
    let stripped_model_switch_item = incoming_items
        .as_mut()
        .and_then(extract_latest_model_switch_update_from_items)
        .or_else(|| extract_trailing_model_switch_update_for_compaction_request(&mut history));
    if let Some(incoming_items) = incoming_items.as_ref() {
        history.record_items(incoming_items.iter(), turn_context.truncation_policy);
    }
    if !history.raw_items().iter().any(is_user_turn_boundary) {
        // Nothing to compact: do not rewrite history when there is no user-turn boundary.
        sess.emit_turn_item_completed(&turn_context, compaction_item)
            .await;
        return Ok(());
    }
    history.record_items(
        &[initial_input_for_turn.into()],
        turn_context.truncation_policy,
    );
    // Keep incoming turn items and the compaction prompt pinned at the tail while trimming.
    // Pre-turn compaction should fail with ContextWindowExceeded rather than dropping incoming
    // items to force compaction to succeed.
    let protected_tail_items = incoming_items
        .as_ref()
        .map_or(1_usize, |items| items.len().saturating_add(1));

    let mut truncated_count = 0usize;

    let max_retries = turn_context.provider.stream_max_retries();
    let mut retries = 0;
    let mut client_session = sess.services.model_client.new_session();
    // Reuse one client session so turn-scoped state (sticky routing, websocket append tracking)
    // survives retries within this compact turn.

    // TODO: If we need to guarantee the persisted mode always matches the prompt used for this
    // turn, capture it in TurnContext at creation time. Using SessionConfiguration here avoids
    // duplicating model settings on TurnContext, but an Op after turn start could update the
    // session config before this write occurs.
    let collaboration_mode = sess.current_collaboration_mode().await;
    let rollout_item =
        RolloutItem::TurnContext(turn_context.to_turn_context_item(collaboration_mode));
    sess.persist_rollout_items(&[rollout_item]).await;

    loop {
        // Clone is required because of the loop
        let turn_input = history
            .clone()
            .for_prompt(&turn_context.model_info.input_modalities);
        let turn_input_len = turn_input.len();
        let prompt = Prompt {
            input: turn_input,
            base_instructions: sess.get_base_instructions().await,
            personality: turn_context.personality,
            ..Default::default()
        };
        let turn_metadata_header = turn_context.turn_metadata_state.current_header_value();
        let attempt_result = drain_to_completed(
            &sess,
            turn_context.as_ref(),
            &mut client_session,
            turn_metadata_header.as_deref(),
            &prompt,
        )
        .await;

        match attempt_result {
            Ok(()) => {
                if truncated_count > 0 {
                    sess.notify_background_event(
                        turn_context.as_ref(),
                        format!(
                            "Trimmed {truncated_count} older thread item(s) before compacting so the prompt fits the model context window."
                        ),
                    )
                    .await;
                }
                break;
            }
            Err(CodexErr::Interrupted) => {
                return Err(CodexErr::Interrupted);
            }
            Err(e @ CodexErr::ContextWindowExceeded) => {
                if turn_input_len > 1 && history.raw_items().len() > protected_tail_items {
                    // Trim from the beginning to preserve cache (prefix-based) and keep recent
                    // messages intact.
                    error!(
                        turn_id = %turn_context.sub_id,
                        compact_callsite = ?compact_callsite,
                        "Context window exceeded while compacting; removing oldest history item. Error: {e}"
                    );
                    history.remove_first_item();
                    truncated_count += 1;
                    retries = 0;
                    continue;
                }
                sess.set_total_tokens_full(turn_context.as_ref()).await;
                error!(
                    turn_id = %turn_context.sub_id,
                    compact_callsite = ?compact_callsite,
                    compact_error = %e,
                    "compaction failed after history truncation could not proceed"
                );
                return Err(e);
            }
            Err(e) => {
                if retries < max_retries {
                    retries += 1;
                    let delay = backoff(retries);
                    sess.notify_stream_error(
                        turn_context.as_ref(),
                        format!("Reconnecting... {retries}/{max_retries}"),
                        e,
                    )
                    .await;
                    tokio::time::sleep(delay).await;
                    continue;
                }
                error!(
                    turn_id = %turn_context.sub_id,
                    compact_callsite = ?compact_callsite,
                    retries,
                    max_retries,
                    compact_error = %e,
                    "compaction failed after retry exhaustion"
                );
                return Err(e);
            }
        }
    }

    let history_snapshot = sess.clone_history().await;
    let history_items = history_snapshot.raw_items();
    let summary_suffix = get_last_assistant_message_from_turn(history_items).unwrap_or_default();
    let summary_text = format!("{SUMMARY_PREFIX}\n{summary_suffix}");
    let user_messages = collect_user_messages(history_items);
    let compacted_history = build_compacted_history_with_limit(
        &user_messages,
        &summary_text,
        COMPACT_USER_MESSAGE_MAX_TOKENS,
    );
    let mut new_history = process_compacted_history(compacted_history);
    match compact_callsite {
        CompactCallsite::MidTurnContinuation | CompactCallsite::PreSamplingModelSwitch => {
            // These callsites do not get a later post-compaction canonical-context write in
            // `run_turn`, so replacement history must carry canonical context directly.
            let initial_context = sess.build_initial_context(turn_context.as_ref()).await;
            insert_initial_context_before_last_user_anchor(&mut new_history, initial_context);
        }
        CompactCallsite::ManualCompact => {
            // Manual `/compact` intentionally rewrites transcript history without reseeding turn
            // context here; the task marks initial context unseeded for the next user turn.
        }
        CompactCallsite::PreTurnIncludingIncomingUserMessage
        | CompactCallsite::PreTurnExcludingIncomingUserMessage => {
            // Pre-turn compaction persists canonical context directly above the incoming user
            // message in `run_turn`, not inside compacted replacement history.
        }
    }
    // Reattach stripped model-switch updates into replacement history so post-compaction
    // sampling keeps model-switch guidance regardless of compaction callsite.
    if let Some(model_switch_item) = stripped_model_switch_item {
        new_history.push(model_switch_item);
    }
    let ghost_snapshots: Vec<ResponseItem> = history_items
        .iter()
        .filter(|item| matches!(item, ResponseItem::GhostSnapshot { .. }))
        .cloned()
        .collect();
    new_history.extend(ghost_snapshots);
    sess.replace_history(new_history).await;
    sess.recompute_token_usage(&turn_context).await;

    let rollout_item = RolloutItem::Compacted(CompactedItem {
        message: summary_text.clone(),
        replacement_history: Some(sess.clone_history().await.raw_items().to_vec()),
    });
    sess.persist_rollout_items(&[rollout_item]).await;

    sess.emit_turn_item_completed(&turn_context, compaction_item)
        .await;
    let warning = EventMsg::Warning(WarningEvent {
        message: "Heads up: Long threads and multiple compactions can cause the model to be less accurate. Start a new thread when possible to keep threads small and targeted.".to_string(),
    });
    sess.send_event(&turn_context, warning).await;
    Ok(())
}

pub fn content_items_to_text(content: &[ContentItem]) -> Option<String> {
    let mut pieces = Vec::new();
    for item in content {
        match item {
            ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                if !text.is_empty() {
                    pieces.push(text.as_str());
                }
            }
            ContentItem::InputImage { .. } => {}
        }
    }
    if pieces.is_empty() {
        None
    } else {
        Some(pieces.join("\n"))
    }
}

pub(crate) fn collect_user_messages(items: &[ResponseItem]) -> Vec<String> {
    items
        .iter()
        .filter_map(parsed_user_message_text)
        .filter(|message| !is_summary_message(message))
        .collect()
}

pub(crate) fn is_summary_message(message: &str) -> bool {
    message.starts_with(format!("{SUMMARY_PREFIX}\n").as_str())
}

pub(crate) fn process_compacted_history(
    mut compacted_history: Vec<ResponseItem>,
) -> Vec<ResponseItem> {
    // Keep only model-visible transcript items that we allow from remote compaction output.
    compacted_history.retain(should_keep_compacted_history_item);

    compacted_history
}

/// Inserts canonical initial context immediately before the latest user anchor in compacted
/// replacement history:
/// - prefer the last real (non-summary) user message;
/// - otherwise fall back to the last summary user message.
///
/// If no user anchor exists, this is a no-op.
pub(crate) fn insert_initial_context_before_last_user_anchor(
    compacted_history: &mut Vec<ResponseItem>,
    initial_context: Vec<ResponseItem>,
) {
    if initial_context.is_empty() {
        return;
    }
    let insertion_index = compacted_history
        .iter()
        .rposition(is_real_user_message)
        .or_else(|| {
            compacted_history
                .iter()
                .rposition(is_summary_user_message_item)
        });
    if let Some(index) = insertion_index {
        compacted_history.splice(index..index, initial_context);
    }
}

fn is_real_user_message(item: &ResponseItem) -> bool {
    parsed_user_message_text(item).is_some_and(|message| !is_summary_message(&message))
}

fn is_summary_user_message_item(item: &ResponseItem) -> bool {
    parsed_user_message_text(item).is_some_and(|message| is_summary_message(&message))
}

fn parsed_user_message_text(item: &ResponseItem) -> Option<String> {
    match crate::event_mapping::parse_turn_item(item) {
        Some(TurnItem::UserMessage(user_message)) => Some(user_message.message()),
        _ => None,
    }
}

fn is_user_shell_command_record(item: &ResponseItem) -> bool {
    matches!(
        item,
        ResponseItem::Message { role, content, .. }
            if role == "user"
                && matches!(
                    content.as_slice(),
                    [ContentItem::InputText { text }] if is_user_shell_command_text(text)
                )
    )
}

/// Returns whether an item from remote compaction output should be preserved.
///
/// Called while processing the model-provided compacted transcript, before we
/// append fresh canonical context from the current session.
///
/// We drop:
/// - `developer` messages because remote output can include stale/duplicated
///   instruction content.
/// - non-user-content `user` messages (session prefix/instruction wrappers),
///   keeping only real user messages as parsed by `parse_turn_item`.
/// - all non-user transcript items except compaction records.
///
/// This intentionally keeps compaction-generated summary messages because they
/// parse as `TurnItem::UserMessage`.
pub(crate) fn should_keep_compacted_history_item(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { role, .. } => {
            if role != "user" {
                return false;
            }
            if is_user_shell_command_record(item) {
                // TODO(ccunningham): Truncate preserved user shell-command records so they cannot
                // cause repeated context-window overflows across compaction attempts.
                return true;
            }

            parsed_user_message_text(item).is_some()
        }
        // Keep compaction records for local/remote history continuity and token accounting.
        ResponseItem::Compaction { .. } => true,
        ResponseItem::Reasoning { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::GhostSnapshot { .. }
        | ResponseItem::Other => false,
    }
}

pub(crate) fn build_compacted_history(
    user_messages: &[String],
    summary_text: &str,
) -> Vec<ResponseItem> {
    build_compacted_history_with_limit(user_messages, summary_text, COMPACT_USER_MESSAGE_MAX_TOKENS)
}

fn build_compacted_history_with_limit(
    user_messages: &[String],
    summary_text: &str,
    max_tokens: usize,
) -> Vec<ResponseItem> {
    let mut history = Vec::new();
    let mut selected_messages: Vec<String> = Vec::new();
    if max_tokens > 0 {
        let mut remaining = max_tokens;
        for message in user_messages.iter().rev() {
            if remaining == 0 {
                break;
            }
            let tokens = approx_token_count(message);
            if tokens <= remaining {
                selected_messages.push(message.clone());
                remaining = remaining.saturating_sub(tokens);
            } else {
                let truncated = truncate_text(message, TruncationPolicy::Tokens(remaining));
                selected_messages.push(truncated);
                break;
            }
        }
        selected_messages.reverse();
    }

    for message in &selected_messages {
        history.push(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: message.clone(),
            }],
            end_turn: None,
            phase: None,
        });
    }

    let summary_text = if summary_text.is_empty() {
        "(no summary available)".to_string()
    } else {
        summary_text.to_string()
    };

    history.push(ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText { text: summary_text }],
        end_turn: None,
        phase: None,
    });

    history
}

async fn drain_to_completed(
    sess: &Session,
    turn_context: &TurnContext,
    client_session: &mut ModelClientSession,
    turn_metadata_header: Option<&str>,
    prompt: &Prompt,
) -> CodexResult<()> {
    let mut stream = client_session
        .stream(
            prompt,
            &turn_context.model_info,
            &turn_context.otel_manager,
            turn_context.reasoning_effort,
            turn_context.reasoning_summary,
            turn_metadata_header,
        )
        .await?;
    loop {
        let maybe_event = stream.next().await;
        let Some(event) = maybe_event else {
            return Err(CodexErr::Stream(
                "stream closed before response.completed".into(),
                None,
            ));
        };
        match event {
            Ok(ResponseEvent::OutputItemDone(item)) => {
                sess.record_into_history(std::slice::from_ref(&item), turn_context)
                    .await;
            }
            Ok(ResponseEvent::ServerReasoningIncluded(included)) => {
                sess.set_server_reasoning_included(included).await;
            }
            Ok(ResponseEvent::RateLimits(snapshot)) => {
                sess.update_rate_limits(turn_context, snapshot).await;
            }
            Ok(ResponseEvent::Completed { token_usage, .. }) => {
                sess.update_token_usage_info(turn_context, token_usage.as_ref())
                    .await;
                return Ok(());
            }
            Ok(_) => continue,
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn content_items_to_text_joins_non_empty_segments() {
        let items = vec![
            ContentItem::InputText {
                text: "hello".to_string(),
            },
            ContentItem::OutputText {
                text: String::new(),
            },
            ContentItem::OutputText {
                text: "world".to_string(),
            },
        ];

        let joined = content_items_to_text(&items);

        assert_eq!(Some("hello\nworld".to_string()), joined);
    }

    #[test]
    fn content_items_to_text_ignores_image_only_content() {
        let items = vec![ContentItem::InputImage {
            image_url: "file://image.png".to_string(),
        }];

        let joined = content_items_to_text(&items);

        assert_eq!(None, joined);
    }

    #[test]
    fn extract_trailing_model_switch_update_for_compaction_request_removes_trailing_item() {
        let mut history = ContextManager::new();
        history.replace(vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "USER_MESSAGE".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "ASSISTANT_REPLY".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<model_switch>\nNEW_MODEL_INSTRUCTIONS".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
        ]);

        let model_switch_item =
            extract_trailing_model_switch_update_for_compaction_request(&mut history);

        assert_eq!(history.raw_items().len(), 2);
        assert!(model_switch_item.is_some());
        assert!(
            history
                .raw_items()
                .iter()
                .all(|item| !Session::is_model_switch_developer_message(item))
        );
    }

    #[test]
    fn extract_trailing_model_switch_update_for_compaction_request_keeps_historical_item() {
        let mut history = ContextManager::new();
        history.replace(vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "FIRST_USER_MESSAGE".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<model_switch>\nOLDER_MODEL_INSTRUCTIONS".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "ASSISTANT_REPLY".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "SECOND_USER_MESSAGE".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
        ]);

        let model_switch_item =
            extract_trailing_model_switch_update_for_compaction_request(&mut history);

        assert_eq!(history.raw_items().len(), 4);
        assert!(model_switch_item.is_none());
        assert!(
            history
                .raw_items()
                .iter()
                .any(Session::is_model_switch_developer_message)
        );
    }

    #[test]
    fn extract_model_switch_update_for_compaction_request_prefers_incoming_items() {
        let mut history = ContextManager::new();
        history.replace(vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "USER_MESSAGE".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "ASSISTANT_REPLY".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<model_switch>\nHISTORY_MODEL_INSTRUCTIONS".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
        ]);
        let mut incoming_items = vec![
            ResponseItem::Message {
                id: None,
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<model_switch>\nINCOMING_MODEL_INSTRUCTIONS".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "INCOMING_USER_MESSAGE".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
        ];

        let model_switch_item = Some(&mut incoming_items)
            .and_then(extract_latest_model_switch_update_from_items)
            .or_else(|| extract_trailing_model_switch_update_for_compaction_request(&mut history));

        assert_eq!(
            model_switch_item,
            Some(ResponseItem::Message {
                id: None,
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<model_switch>\nINCOMING_MODEL_INSTRUCTIONS".to_string(),
                }],
                end_turn: None,
                phase: None,
            })
        );
        assert_eq!(
            incoming_items,
            vec![ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "INCOMING_USER_MESSAGE".to_string(),
                }],
                end_turn: None,
                phase: None,
            }]
        );
        assert!(
            history
                .raw_items()
                .iter()
                .any(Session::is_model_switch_developer_message)
        );
    }

    #[test]
    fn collect_user_messages_extracts_user_text_only() {
        let items = vec![
            ResponseItem::Message {
                id: Some("assistant".to_string()),
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "ignored".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: Some("user".to_string()),
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "first".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Other,
        ];

        let collected = collect_user_messages(&items);

        assert_eq!(vec!["first".to_string()], collected);
    }

    #[test]
    fn collect_user_messages_filters_session_prefix_entries() {
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: r#"# AGENTS.md instructions for project

<INSTRUCTIONS>
do things
</INSTRUCTIONS>"#
                        .to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<ENVIRONMENT_CONTEXT>cwd=/tmp</ENVIRONMENT_CONTEXT>".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "real user message".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
        ];

        let collected = collect_user_messages(&items);

        assert_eq!(vec!["real user message".to_string()], collected);
    }

    #[test]
    fn build_token_limited_compacted_history_truncates_overlong_user_messages() {
        // Use a small truncation limit so the test remains fast while still validating
        // that oversized user content is truncated.
        let max_tokens = 16;
        let big = "word ".repeat(200);
        let history = super::build_compacted_history_with_limit(
            std::slice::from_ref(&big),
            "SUMMARY",
            max_tokens,
        );
        assert_eq!(history.len(), 2);

        let truncated_message = &history[0];
        let summary_message = &history[1];

        let truncated_text = match truncated_message {
            ResponseItem::Message { role, content, .. } if role == "user" => {
                content_items_to_text(content).unwrap_or_default()
            }
            other => panic!("unexpected item in history: {other:?}"),
        };

        assert!(
            truncated_text.contains("tokens truncated"),
            "expected truncation marker in truncated user message"
        );
        assert!(
            !truncated_text.contains(&big),
            "truncated user message should not include the full oversized user text"
        );

        let summary_text = match summary_message {
            ResponseItem::Message { role, content, .. } if role == "user" => {
                content_items_to_text(content).unwrap_or_default()
            }
            other => panic!("unexpected item in history: {other:?}"),
        };
        assert_eq!(summary_text, "SUMMARY");
    }

    #[test]
    fn build_token_limited_compacted_history_appends_summary_message() {
        let user_messages = vec!["first user message".to_string()];
        let summary_text = "summary text";

        let history = build_compacted_history(&user_messages, summary_text);
        assert!(
            !history.is_empty(),
            "expected compacted history to include summary"
        );

        let last = history.last().expect("history should have a summary entry");
        let summary = match last {
            ResponseItem::Message { role, content, .. } if role == "user" => {
                content_items_to_text(content).unwrap_or_default()
            }
            other => panic!("expected summary message, found {other:?}"),
        };
        assert_eq!(summary, summary_text);
    }

    #[test]
    fn build_compacted_history_preserves_user_message_structure() {
        let history =
            super::build_compacted_history_with_limit(&["older user".to_string()], "SUMMARY", 128);

        let expected = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "older user".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "SUMMARY".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
        ];

        assert_eq!(history, expected);
    }

    #[test]
    fn real_user_message_includes_image_only_user_messages() {
        let image_only_user = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputImage {
                image_url: "data:image/png;base64,AAAA".to_string(),
            }],
            end_turn: None,
            phase: None,
        };

        assert!(super::is_real_user_message(&image_only_user));
    }

    #[test]
    fn real_user_message_excludes_user_shell_command_records() {
        let shell_command_user = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "<user_shell_command>\necho hi\n</user_shell_command>".to_string(),
            }],
            end_turn: None,
            phase: None,
        };

        assert!(!super::is_real_user_message(&shell_command_user));
    }

    #[test]
    fn should_keep_compacted_history_item_drops_user_session_prefix_and_keeps_user_shell_command() {
        let session_prefix = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "<environment_context>\n  <cwd>/repo</cwd>\n</environment_context>"
                    .to_string(),
            }],
            end_turn: None,
            phase: None,
        };
        let shell_command_user = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "<user_shell_command>\necho hi\n</user_shell_command>".to_string(),
            }],
            end_turn: None,
            phase: None,
        };

        assert!(!super::should_keep_compacted_history_item(&session_prefix));
        assert!(super::should_keep_compacted_history_item(
            &shell_command_user
        ));
    }

    #[test]
    fn should_keep_compacted_history_item_keeps_compaction_item() {
        let compaction = ResponseItem::Compaction {
            encrypted_content: "abc123".to_string(),
        };

        assert!(super::should_keep_compacted_history_item(&compaction));
    }

    #[test]
    fn process_compacted_history_drops_developer_messages() {
        let compacted_history = vec![
            ResponseItem::Message {
                id: None,
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "stale permissions".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "summary".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "stale personality".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
        ];

        let refreshed = process_compacted_history(compacted_history);
        let expected = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "summary".to_string(),
            }],
            end_turn: None,
            phase: None,
        }];
        assert_eq!(refreshed, expected);
    }

    #[test]
    fn process_compacted_history_drops_non_user_content_messages() {
        let compacted_history = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: r#"# AGENTS.md instructions for /repo

<INSTRUCTIONS>
keep me updated
</INSTRUCTIONS>"#
                        .to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: r#"<environment_context>
  <cwd>/repo</cwd>
  <shell>zsh</shell>
</environment_context>"#
                        .to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: r#"<turn_aborted>
  <turn_id>turn-1</turn_id>
  <reason>interrupted</reason>
</turn_aborted>"#
                        .to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "summary".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "stale developer instructions".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
        ];

        let refreshed = process_compacted_history(compacted_history);
        let expected = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "summary".to_string(),
            }],
            end_turn: None,
            phase: None,
        }];
        assert_eq!(refreshed, expected);
    }

    #[test]
    fn process_compacted_history_preserves_summary_order() {
        let compacted_history = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "older user".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: format!("{SUMMARY_PREFIX}\nolder summary"),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "newer user".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: format!("{SUMMARY_PREFIX}\nlatest summary"),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "assistant after latest summary".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
        ];

        let refreshed = process_compacted_history(compacted_history);
        let expected = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "older user".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: format!("{SUMMARY_PREFIX}\nolder summary"),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "newer user".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: format!("{SUMMARY_PREFIX}\nlatest summary"),
                }],
                end_turn: None,
                phase: None,
            },
        ];
        assert_eq!(refreshed, expected);
    }

    #[test]
    fn process_compacted_history_keeps_summary_only_history() {
        let compacted_history = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!("{SUMMARY_PREFIX}\nsummary text"),
            }],
            end_turn: None,
            phase: None,
        }];

        let refreshed = process_compacted_history(compacted_history);
        let expected = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!("{SUMMARY_PREFIX}\nsummary text"),
            }],
            end_turn: None,
            phase: None,
        }];
        assert_eq!(refreshed, expected);
    }

    #[test]
    fn insert_initial_context_before_last_user_anchor_falls_back_to_last_summary() {
        let mut compacted_history = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: format!("{SUMMARY_PREFIX}\nolder summary"),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: format!("{SUMMARY_PREFIX}\nlatest summary"),
                }],
                end_turn: None,
                phase: None,
            },
        ];
        let initial_context = vec![ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: "fresh permissions".to_string(),
            }],
            end_turn: None,
            phase: None,
        }];

        insert_initial_context_before_last_user_anchor(&mut compacted_history, initial_context);

        let expected = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: format!("{SUMMARY_PREFIX}\nolder summary"),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "developer".to_string(),
                content: vec![ContentItem::InputText {
                    text: "fresh permissions".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: format!("{SUMMARY_PREFIX}\nlatest summary"),
                }],
                end_turn: None,
                phase: None,
            },
        ];
        assert_eq!(compacted_history, expected);
    }
}
