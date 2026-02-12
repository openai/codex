use std::sync::Arc;

use crate::Prompt;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::compact::AutoCompactCallsite;
use crate::compact::TurnContextReinjection;
use crate::context_manager::ContextManager;
use crate::context_manager::TotalTokenUsageBreakdown;
use crate::context_manager::estimate_item_token_count;
use crate::context_manager::estimate_response_item_model_visible_bytes;
use crate::context_manager::is_codex_generated_item;
use crate::context_manager::is_user_turn_boundary;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::protocol::CompactedItem;
use crate::protocol::EventMsg;
use crate::protocol::RolloutItem;
use crate::protocol::TurnStartedEvent;
use codex_protocol::items::ContextCompactionItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ResponseItem;
use futures::TryFutureExt;
use tracing::error;
use tracing::info;

pub(crate) async fn run_inline_remote_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    auto_compact_callsite: AutoCompactCallsite,
    // Controls whether canonical turn context should be reinserted into compacted history.
    turn_context_reinjection: TurnContextReinjection,
    incoming_items: Option<Vec<ResponseItem>>,
) -> CodexResult<()> {
    run_remote_compact_task_inner(
        &sess,
        &turn_context,
        auto_compact_callsite,
        turn_context_reinjection,
        incoming_items,
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
        model_context_window: turn_context.model_context_window(),
        collaboration_mode_kind: turn_context.collaboration_mode.mode,
    });
    sess.send_event(&turn_context, start_event).await;

    run_remote_compact_task_inner(
        &sess,
        &turn_context,
        AutoCompactCallsite::PreTurnExcludingIncomingUserMessage,
        // Manual `/compact` should not reinsert turn context into compacted history; we reseed
        // canonical initial context before the next user turn.
        TurnContextReinjection::Skip,
        None,
    )
    .await
}

async fn run_remote_compact_task_inner(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    auto_compact_callsite: AutoCompactCallsite,
    turn_context_reinjection: TurnContextReinjection,
    incoming_items: Option<Vec<ResponseItem>>,
) -> CodexResult<()> {
    if let Err(err) = run_remote_compact_task_inner_impl(
        sess,
        turn_context,
        auto_compact_callsite,
        turn_context_reinjection,
        incoming_items,
    )
    .await
    {
        error!(
            turn_id = %turn_context.sub_id,
            auto_compact_callsite = ?auto_compact_callsite,
            compact_error = %err,
            "remote compaction task failed"
        );
        return Err(err);
    }
    Ok(())
}

async fn run_remote_compact_task_inner_impl(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    auto_compact_callsite: AutoCompactCallsite,
    turn_context_reinjection: TurnContextReinjection,
    incoming_items: Option<Vec<ResponseItem>>,
) -> CodexResult<()> {
    let compaction_item = TurnItem::ContextCompaction(ContextCompactionItem::new());
    sess.emit_turn_item_started(turn_context, &compaction_item)
        .await;
    let mut history = sess.clone_history().await;
    let base_instructions = sess.get_base_instructions().await;
    let deleted_items = trim_function_call_history_to_fit_context_window(
        &mut history,
        turn_context.as_ref(),
        &base_instructions,
        incoming_items.as_deref(),
    );
    if let Some(incoming_items) = incoming_items {
        history.record_items(incoming_items.iter(), turn_context.truncation_policy);
    }
    if !history.raw_items().iter().any(is_user_turn_boundary) {
        // Nothing to compact: do not rewrite history when there is no user-turn boundary.
        sess.emit_turn_item_completed(turn_context, compaction_item)
            .await;
        return Ok(());
    }
    if deleted_items > 0 {
        info!(
            turn_id = %turn_context.sub_id,
            auto_compact_callsite = ?auto_compact_callsite,
            deleted_items,
            "trimmed history items before remote compaction"
        );
    }

    // Required to keep `/undo` available after compaction
    let ghost_snapshots: Vec<ResponseItem> = history
        .raw_items()
        .iter()
        .filter(|item| matches!(item, ResponseItem::GhostSnapshot { .. }))
        .cloned()
        .collect();

    let prompt = Prompt {
        input: history.for_prompt(&turn_context.model_info.input_modalities),
        tools: vec![],
        parallel_tool_calls: false,
        base_instructions,
        personality: turn_context.personality,
        output_schema: None,
    };

    let mut new_history = sess
        .services
        .model_client
        .compact_conversation_history(
            &prompt,
            &turn_context.model_info,
            &turn_context.otel_manager,
        )
        .or_else(|err| async {
            let total_usage_breakdown = sess.get_total_token_usage_breakdown().await;
            let compact_request_log_data =
                build_compact_request_log_data(&prompt.input, &prompt.base_instructions.text);
            log_remote_compact_failure(
                turn_context,
                auto_compact_callsite,
                &compact_request_log_data,
                total_usage_breakdown,
                &err,
            );
            Err(err)
        })
        .await?;
    new_history = sess
        .process_compacted_history(turn_context, new_history, turn_context_reinjection)
        .await;

    if !ghost_snapshots.is_empty() {
        new_history.extend(ghost_snapshots);
    }
    sess.replace_history(new_history.clone()).await;
    sess.recompute_token_usage(turn_context).await;

    let compacted_item = CompactedItem {
        message: String::new(),
        replacement_history: Some(new_history),
    };
    sess.persist_rollout_items(&[RolloutItem::Compacted(compacted_item)])
        .await;

    sess.emit_turn_item_completed(turn_context, compaction_item)
        .await;
    Ok(())
}

#[derive(Debug)]
struct CompactRequestLogData {
    failing_compaction_request_model_visible_bytes: i64,
}

fn build_compact_request_log_data(
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

fn log_remote_compact_failure(
    turn_context: &TurnContext,
    auto_compact_callsite: AutoCompactCallsite,
    log_data: &CompactRequestLogData,
    total_usage_breakdown: TotalTokenUsageBreakdown,
    err: &CodexErr,
) {
    error!(
        turn_id = %turn_context.sub_id,
        auto_compact_callsite = ?auto_compact_callsite,
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

fn trim_function_call_history_to_fit_context_window(
    history: &mut ContextManager,
    turn_context: &TurnContext,
    base_instructions: &BaseInstructions,
    incoming_items: Option<&[ResponseItem]>,
) -> usize {
    let Some(context_window) = turn_context.model_context_window() else {
        return 0;
    };
    let incoming_items_tokens = incoming_items
        .unwrap_or_default()
        .iter()
        .map(estimate_item_token_count)
        .fold(0_i64, i64::saturating_add);
    trim_codex_generated_tail_items_to_fit_context_window(
        history,
        context_window,
        base_instructions,
        incoming_items_tokens,
    )
}

fn trim_codex_generated_tail_items_to_fit_context_window(
    history: &mut ContextManager,
    context_window: i64,
    base_instructions: &BaseInstructions,
    incoming_items_tokens: i64,
) -> usize {
    let mut deleted_items = 0usize;

    while history
        .estimate_token_count_with_base_instructions(base_instructions)
        .is_some_and(|estimated_tokens| {
            estimated_tokens.saturating_add(incoming_items_tokens) > context_window
        })
    {
        let Some(last_item) = history.raw_items().last() else {
            break;
        };
        if !is_codex_generated_item(last_item) {
            break;
        }
        if !history.remove_last_item() {
            break;
        }
        deleted_items += 1;
    }

    deleted_items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::truncate::TruncationPolicy;
    use codex_protocol::models::ContentItem;
    use pretty_assertions::assert_eq;

    fn user_message(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
            end_turn: None,
            phase: None,
        }
    }

    fn developer_message(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "developer".to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
            end_turn: None,
            phase: None,
        }
    }

    #[test]
    fn trim_accounts_for_incoming_items_tokens() {
        let base_instructions = BaseInstructions {
            text: String::new(),
        };
        let incoming_items = [user_message(
            "INCOMING_USER_MESSAGE_THAT_TIPS_OVER_THE_WINDOW",
        )];
        let incoming_items_tokens = incoming_items
            .iter()
            .map(estimate_item_token_count)
            .fold(0_i64, i64::saturating_add);
        assert!(
            incoming_items_tokens > 0,
            "expected incoming item token estimate to be positive"
        );

        let mut history = ContextManager::new();
        let history_items = [
            user_message("USER_ONE"),
            developer_message("TRAILING_CODEX_GENERATED_CONTEXT"),
        ];
        history.record_items(history_items.iter(), TruncationPolicy::Tokens(10_000));
        let history_tokens = history
            .estimate_token_count_with_base_instructions(&base_instructions)
            .unwrap_or_default();
        let context_window = history_tokens
            .saturating_add(incoming_items_tokens)
            .saturating_sub(1);
        let mut without_incoming_projection = history.clone();

        let deleted_without_incoming = trim_codex_generated_tail_items_to_fit_context_window(
            &mut without_incoming_projection,
            context_window,
            &base_instructions,
            0,
        );
        assert_eq!(
            deleted_without_incoming, 0,
            "history-only projection should not trim when currently under the limit"
        );

        let deleted_with_incoming = trim_codex_generated_tail_items_to_fit_context_window(
            &mut history,
            context_window,
            &base_instructions,
            incoming_items_tokens,
        );
        assert_eq!(
            deleted_with_incoming, 1,
            "incoming projection should trim trailing codex-generated history to fit pre-turn request"
        );
        assert_eq!(history.raw_items(), vec![user_message("USER_ONE")]);
    }
}
