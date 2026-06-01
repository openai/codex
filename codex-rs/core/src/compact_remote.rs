use std::sync::Arc;

use crate::Prompt;
use crate::client::CompactConversationRequestSettings;
use crate::compact::CompactionAnalyticsAttempt;
use crate::compact::InitialContextInjection;
use crate::compact::compaction_status_from_result;
use crate::compact::insert_initial_context_before_last_real_user_or_summary;
use crate::context_manager::ContextManager;
use crate::context_manager::TotalTokenUsageBreakdown;
use crate::context_manager::estimate_response_item_model_visible_bytes;
use crate::context_manager::is_codex_generated_item;
use crate::context_manager::is_user_turn_boundary;
use crate::event_mapping::is_contextual_user_message_content;
use crate::hook_runtime::PostCompactHookOutcome;
use crate::hook_runtime::PreCompactHookOutcome;
use crate::hook_runtime::run_post_compact_hooks;
use crate::hook_runtime::run_pre_compact_hooks;
use crate::session::session::Session;
use crate::session::turn::built_tools;
use crate::session::turn_context::TurnContext;
use codex_analytics::CompactionImplementation;
use codex_analytics::CompactionPhase;
use codex_analytics::CompactionReason;
use codex_analytics::CompactionTrigger;
use codex_app_server_protocol::AuthMode;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::items::ContextCompactionItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::CompactedItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnStartedEvent;
use codex_rollout_trace::CompactionCheckpointTracePayload;
use futures::TryFutureExt;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;

pub(crate) async fn run_inline_remote_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    initial_context_injection: InitialContextInjection,
    reason: CompactionReason,
    phase: CompactionPhase,
) -> CodexResult<()> {
    run_remote_compact_task_inner(
        &sess,
        &turn_context,
        initial_context_injection,
        CompactionTrigger::Auto,
        reason,
        phase,
    )
    .await?;
    Ok(())
}

pub(crate) async fn run_remote_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) -> CodexResult<()> {
    let start_event = EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: turn_context.sub_id.clone(),
        trace_id: turn_context.trace_id.clone(),
        started_at: turn_context.turn_timing_state.started_at_unix_secs().await,
        model_context_window: turn_context.model_context_window(),
        collaboration_mode_kind: turn_context.collaboration_mode.mode,
    });
    sess.send_event(&turn_context, start_event).await;

    run_remote_compact_task_inner(
        &sess,
        &turn_context,
        InitialContextInjection::DoNotInject,
        CompactionTrigger::Manual,
        CompactionReason::UserRequested,
        CompactionPhase::StandaloneTurn,
    )
    .await?;
    Ok(())
}

async fn run_remote_compact_task_inner(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    initial_context_injection: InitialContextInjection,
    trigger: CompactionTrigger,
    reason: CompactionReason,
    phase: CompactionPhase,
) -> CodexResult<()> {
    let attempt = CompactionAnalyticsAttempt::begin(
        sess.as_ref(),
        turn_context.as_ref(),
        trigger,
        reason,
        CompactionImplementation::ResponsesCompact,
        phase,
    )
    .await;
    let pre_compact_outcome = run_pre_compact_hooks(sess, turn_context, trigger).await;
    match pre_compact_outcome {
        PreCompactHookOutcome::Continue => {}
        PreCompactHookOutcome::Stopped { reason } => {
            let error = reason.unwrap_or_else(|| "PreCompact hook stopped execution".to_string());
            attempt
                .track(
                    sess.as_ref(),
                    codex_analytics::CompactionStatus::Interrupted,
                    Some(error),
                )
                .await;
            return Err(CodexErr::TurnAborted);
        }
    }
    let result =
        run_remote_compact_task_inner_impl(sess, turn_context, initial_context_injection).await;
    let status = compaction_status_from_result(&result);
    let error = result.as_ref().err().map(ToString::to_string);
    if result.is_ok() {
        let post_compact_outcome = run_post_compact_hooks(sess, turn_context, trigger).await;
        if let PostCompactHookOutcome::Stopped = post_compact_outcome {
            attempt.track(sess.as_ref(), status, error).await;
            return Err(CodexErr::TurnAborted);
        }
    }
    attempt.track(sess.as_ref(), status, error.clone()).await;
    if let Err(err) = result {
        let event = EventMsg::Error(
            err.to_error_event(Some("Error running remote compact task".to_string())),
        );
        sess.send_event(turn_context, event).await;
        return Err(err);
    }
    Ok(())
}

async fn run_remote_compact_task_inner_impl(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    initial_context_injection: InitialContextInjection,
) -> CodexResult<()> {
    let context_compaction_item = ContextCompactionItem::new();
    // Use the UI compaction item ID as the trace compaction ID so protocol lifecycle events,
    // endpoint attempts, and the installed history checkpoint all have one join key.
    let compaction_trace = sess.services.rollout_thread_trace.compaction_trace_context(
        turn_context.sub_id.as_str(),
        context_compaction_item.id.as_str(),
        turn_context.model_info.slug.as_str(),
        turn_context.provider.info().name.as_str(),
    );
    let compaction_item = TurnItem::ContextCompaction(context_compaction_item);
    sess.emit_turn_item_started(turn_context, &compaction_item)
        .await;
    let mut history = sess.clone_history().await;
    let base_instructions = sess.get_base_instructions().await;
    let deleted_items = trim_function_call_history_to_fit_context_window(
        &mut history,
        turn_context.as_ref(),
        &base_instructions,
    );
    if deleted_items > 0 {
        info!(
            turn_id = %turn_context.sub_id,
            deleted_items,
            "trimmed history items before remote compaction"
        );
    }
    // This is the history selected for remote compaction, after any trimming required to fit the
    // compact endpoint. The checkpoint below records it separately from the next sampling request,
    // whose prompt will repeat current developer/context prefix items.
    let trace_input_history = history.raw_items().to_vec();
    let prompt_input = history.for_prompt(&turn_context.model_info.input_modalities);
    let tool_router = built_tools(
        sess.as_ref(),
        turn_context.as_ref(),
        &CancellationToken::new(),
    )
    .await?;
    let prompt = Prompt {
        input: prompt_input,
        tools: tool_router.model_visible_specs(),
        parallel_tool_calls: turn_context.model_info.supports_parallel_tool_calls,
        base_instructions,
        personality: turn_context.personality,
        output_schema: None,
        output_schema_strict: true,
    };
    let mut new_history = sess
        .services
        .model_client
        .compact_conversation_history(
            &prompt,
            &turn_context.model_info,
            CompactConversationRequestSettings {
                effort: turn_context.reasoning_effort,
                summary: turn_context.reasoning_summary,
                service_tier: if sess.services.auth_manager.auth_mode() == Some(AuthMode::ApiKey) {
                    None
                } else {
                    turn_context.config.service_tier.clone()
                },
            },
            &turn_context.session_telemetry,
            &compaction_trace,
        )
        .or_else(|err| async {
            let total_usage_breakdown = sess.get_total_token_usage_breakdown().await;
            let compact_request_log_data =
                build_compact_request_log_data(&prompt.input, &prompt.base_instructions.text);
            log_remote_compact_failure(
                turn_context,
                &compact_request_log_data,
                total_usage_breakdown,
                &err,
            );
            Err(err)
        })
        .await?;
    new_history = process_compacted_history(
        sess.as_ref(),
        turn_context.as_ref(),
        new_history,
        initial_context_injection,
    )
    .await;

    let reference_context_item = match initial_context_injection {
        InitialContextInjection::DoNotInject => None,
        InitialContextInjection::BeforeLastUserMessage => Some(turn_context.to_turn_context_item()),
    };
    let compacted_item = CompactedItem {
        message: String::new(),
        replacement_history: Some(new_history.clone()),
    };
    // Install is the semantic boundary where the compact endpoint's output becomes live
    // thread history. Keep it distinct from the later inference request so the reducer can
    // still represent repeated developer/context prefix items exactly as the model saw them.
    compaction_trace.record_installed(&CompactionCheckpointTracePayload {
        input_history: &trace_input_history,
        replacement_history: &new_history,
    });
    sess.replace_compacted_history(new_history, reference_context_item, compacted_item)
        .await;
    sess.recompute_token_usage(turn_context).await;

    sess.emit_turn_item_completed(turn_context, compaction_item)
        .await;
    Ok(())
}

pub(crate) async fn process_compacted_history(
    sess: &Session,
    turn_context: &TurnContext,
    mut compacted_history: Vec<ResponseItem>,
    initial_context_injection: InitialContextInjection,
) -> Vec<ResponseItem> {
    // Mid-turn compaction is the only path that must inject initial context above the last user
    // message in the replacement history. Pre-turn compaction instead injects context after the
    // compaction item, but mid-turn compaction keeps the compaction item last for model training.
    let initial_context = if matches!(
        initial_context_injection,
        InitialContextInjection::BeforeLastUserMessage
    ) {
        sess.build_initial_context(turn_context).await
    } else {
        Vec::new()
    };

    compacted_history.retain(should_keep_compacted_history_item);
    insert_initial_context_before_last_real_user_or_summary(compacted_history, initial_context)
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
///   while preserving real user messages and persisted hook prompts.
///
/// This intentionally keeps:
/// - `assistant` messages (future remote compaction models may emit them)
/// - `user`-role warnings that parse as `TurnItem::UserMessage` and compaction-generated summary
///   messages. Legacy warning fragments are filtered by `parse_turn_item` before they reach this
///   check.
pub(crate) fn should_keep_compacted_history_item(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { role, .. } if role == "developer" => false,
        ResponseItem::Message { role, .. } if role == "user" => {
            matches!(
                crate::event_mapping::parse_turn_item(item),
                Some(TurnItem::UserMessage(_) | TurnItem::HookPrompt(_))
            )
        }
        ResponseItem::Message { role, .. } if role == "assistant" => true,
        ResponseItem::Message { .. } => false,
        ResponseItem::Compaction { .. } | ResponseItem::ContextCompaction { .. } => true,
        ResponseItem::CompactionTrigger => false,
        ResponseItem::Reasoning { .. }
        | ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::Other => false,
    }
}

#[derive(Debug)]
pub(crate) struct CompactRequestLogData {
    failing_compaction_request_model_visible_bytes: i64,
}

pub(crate) fn build_compact_request_log_data(
    input: &[ResponseItem],
    instructions: &str,
) -> CompactRequestLogData {
    let failing_compaction_request_model_visible_bytes = input
        .iter()
        .map(estimate_response_item_model_visible_bytes)
        .fold(
            i64::try_from(instructions.len()).unwrap_or(i64::MAX),
            i64::saturating_add,
        );

    CompactRequestLogData {
        failing_compaction_request_model_visible_bytes,
    }
}

pub(crate) fn log_remote_compact_failure(
    turn_context: &TurnContext,
    log_data: &CompactRequestLogData,
    total_usage_breakdown: TotalTokenUsageBreakdown,
    err: &CodexErr,
) {
    error!(
        turn_id = %turn_context.sub_id,
        last_api_response_total_tokens = total_usage_breakdown.last_api_response_total_tokens,
        all_history_items_model_visible_bytes = total_usage_breakdown.all_history_items_model_visible_bytes,
        estimated_tokens_of_items_added_since_last_successful_api_response = total_usage_breakdown.estimated_tokens_of_items_added_since_last_successful_api_response,
        estimated_bytes_of_items_added_since_last_successful_api_response = total_usage_breakdown.estimated_bytes_of_items_added_since_last_successful_api_response,
        model_context_window_tokens = ?turn_context.model_context_window(),
        failing_compaction_request_model_visible_bytes = log_data.failing_compaction_request_model_visible_bytes,
        compact_error = %err,
        "remote compaction failed"
    );
}

pub(crate) fn trim_function_call_history_to_fit_context_window(
    history: &mut ContextManager,
    turn_context: &TurnContext,
    base_instructions: &BaseInstructions,
) -> usize {
    let mut deleted_items = 0usize;
    let Some(context_window) = turn_context.model_context_window() else {
        return deleted_items;
    };

    while history
        .estimate_token_count_with_base_instructions(base_instructions)
        .is_some_and(|estimated_tokens| estimated_tokens > context_window)
    {
        let Some(last_item) = history.raw_items().last() else {
            break;
        };
        if is_codex_generated_item(last_item) {
            if !history.remove_last_item() {
                break;
            }
            deleted_items += 1;
            continue;
        }

        let Some(chunk_start) = last_runtime_owned_continuation_chunk_start(history.raw_items())
        else {
            break;
        };
        let removed_items = history.raw_items().len().saturating_sub(chunk_start);
        if removed_items == 0 {
            break;
        }
        history.replace(history.raw_items()[..chunk_start].to_vec());
        deleted_items += removed_items;
    }

    deleted_items
}

fn last_runtime_owned_continuation_chunk_start(items: &[ResponseItem]) -> Option<usize> {
    let search_start = items
        .iter()
        .rposition(is_user_turn_boundary)
        .map(|idx| idx.saturating_add(1))?;

    if search_start >= items.len() {
        return None;
    }

    let mut saw_non_contextual_after = false;
    for idx in (search_start..items.len()).rev() {
        if is_contextual_user_item(&items[idx]) {
            if saw_non_contextual_after {
                return Some(idx);
            }
        } else {
            saw_non_contextual_after = true;
        }
    }

    let trailing_contextual_start = items[search_start..]
        .iter()
        .rposition(|item| !is_contextual_user_item(item))
        .map(|idx| search_start + idx + 1)
        .unwrap_or(search_start);

    (trailing_contextual_start < items.len()).then_some(trailing_contextual_start)
}

fn is_contextual_user_item(item: &ResponseItem) -> bool {
    matches!(
        item,
        ResponseItem::Message { role, content, .. }
            if role == "user" && is_contextual_user_message_content(content)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ContextualUserFragment;
    use crate::context::GoalContext;
    use crate::context::TurnAborted;
    use crate::session::tests::make_session_and_context;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::protocol::TruncationPolicy;
    use pretty_assertions::assert_eq;

    fn assistant_message(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: text.to_string(),
            }],
            phase: None,
        }
    }

    fn user_message(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
            phase: None,
        }
    }

    fn function_call(call_id: &str) -> ResponseItem {
        ResponseItem::FunctionCall {
            id: None,
            call_id: call_id.to_string(),
            name: "update_plan".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
        }
    }

    fn function_call_output(call_id: &str, text: &str) -> ResponseItem {
        ResponseItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload::from_text(text.to_string()),
        }
    }

    fn create_history(items: Vec<ResponseItem>) -> ContextManager {
        let mut history = ContextManager::new();
        history.record_items(items.iter(), TruncationPolicy::Tokens(100_000));
        history
    }

    #[tokio::test]
    async fn trim_history_drops_runtime_owned_continuation_chunk() {
        let base_instructions = BaseInstructions {
            text: String::new(),
        };
        let (session, mut turn_context) = make_session_and_context().await;
        turn_context.model_info.context_window = Some(32);
        turn_context.model_info.effective_context_window_percent = 100;

        let retained_prefix = vec![
            user_message("real user request"),
            assistant_message("reply before goal continuation"),
        ];
        let trimmed_chunk = vec![
            ContextualUserFragment::into(GoalContext::new(
                "Continue working toward the active thread goal.",
            )),
            assistant_message("goal follow-up"),
            function_call("call-1"),
            function_call_output("call-1", "plan updated"),
            ContextualUserFragment::into(TurnAborted::new(TurnAborted::INTERRUPTED_GUIDANCE)),
        ];
        let mut history = create_history(
            retained_prefix
                .iter()
                .chain(trimmed_chunk.iter())
                .cloned()
                .collect(),
        );

        let deleted_items = trim_function_call_history_to_fit_context_window(
            &mut history,
            &turn_context,
            &base_instructions,
        );

        assert_eq!(deleted_items, trimmed_chunk.len());
        assert_eq!(history.raw_items(), retained_prefix);
        drop(session);
    }

    #[test]
    fn continuation_chunk_start_prefers_contextual_anchor_before_runtime_suffix() {
        let items = vec![
            user_message("real user request"),
            assistant_message("reply before goal continuation"),
            ContextualUserFragment::into(GoalContext::new(
                "Continue working toward the active thread goal.",
            )),
            assistant_message("goal follow-up"),
            function_call("call-1"),
            function_call_output("call-1", "plan updated"),
            ContextualUserFragment::into(TurnAborted::new(TurnAborted::INTERRUPTED_GUIDANCE)),
        ];

        assert_eq!(last_runtime_owned_continuation_chunk_start(&items), Some(2));
    }
}
