use std::sync::Arc;

use crate::Prompt;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::context_manager::ContextManager;
use crate::context_manager::is_codex_generated_item;
use crate::error::Result as CodexResult;
use crate::protocol::CompactedItem;
use crate::protocol::EventMsg;
use crate::protocol::RolloutItem;
use crate::protocol::TurnStartedEvent;
use codex_protocol::items::ContextCompactionItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ResponseItem;
use tracing::info;

pub(crate) async fn run_inline_remote_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) {
    run_remote_compact_task_inner(&sess, &turn_context).await;
}

pub(crate) async fn run_remote_compact_task(sess: Arc<Session>, turn_context: Arc<TurnContext>) {
    let start_event = EventMsg::TurnStarted(TurnStartedEvent {
        model_context_window: turn_context.model_context_window(),
        collaboration_mode_kind: turn_context.collaboration_mode.mode,
    });
    sess.send_event(&turn_context, start_event).await;

    run_remote_compact_task_inner(&sess, &turn_context).await;
}

async fn run_remote_compact_task_inner(sess: &Arc<Session>, turn_context: &Arc<TurnContext>) {
    if let Err(err) = run_remote_compact_task_inner_impl(sess, turn_context).await {
        let event = EventMsg::Error(
            err.to_error_event(Some("Error running remote compact task".to_string())),
        );
        sess.send_event(turn_context, event).await;
    }
}

async fn run_remote_compact_task_inner_impl(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
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
    );
    if deleted_items > 0 {
        info!(
            turn_id = %turn_context.sub_id,
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
        input: history.for_prompt(),
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
        .await?;

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

fn trim_function_call_history_to_fit_context_window(
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
        // Keep a trailing tool call until its output is present; trimming the
        // call first can orphan a later-arriving output during mid-stream auto-compaction.
        if is_pending_tool_call_without_output(last_item, history.raw_items()) {
            break;
        }
        if !is_remote_compaction_trim_candidate(last_item) {
            break;
        }
        if !history.remove_last_item() {
            break;
        }
        deleted_items += 1;
    }

    deleted_items
}

fn is_remote_compaction_trim_candidate(item: &ResponseItem) -> bool {
    is_codex_generated_item(item)
        || matches!(
            item,
            ResponseItem::FunctionCall { .. }
                | ResponseItem::CustomToolCall { .. }
                | ResponseItem::LocalShellCall { .. }
        )
}

fn is_pending_tool_call_without_output(item: &ResponseItem, items: &[ResponseItem]) -> bool {
    match item {
        ResponseItem::FunctionCall { call_id, .. } => !items.iter().any(|candidate| {
            matches!(
                candidate,
                ResponseItem::FunctionCallOutput {
                    call_id: existing, ..
                } if existing == call_id
            )
        }),
        ResponseItem::CustomToolCall { call_id, .. } => !items.iter().any(|candidate| {
            matches!(
                candidate,
                ResponseItem::CustomToolCallOutput {
                    call_id: existing, ..
                } if existing == call_id
            )
        }),
        ResponseItem::LocalShellCall { call_id, .. } => {
            let Some(call_id) = call_id.as_ref() else {
                return true;
            };
            !items.iter().any(|candidate| {
                matches!(
                    candidate,
                    ResponseItem::FunctionCallOutput {
                        call_id: existing, ..
                    } if existing == call_id
                )
            })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context;
    use codex_protocol::models::ContentItem;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn trim_keeps_trailing_function_call_without_output() {
        let (_session, mut turn_context) = make_session_and_context().await;
        turn_context.model_info.context_window = Some(200);
        turn_context.model_info.effective_context_window_percent = 100;

        let mut history = ContextManager::new();
        history.record_items(
            &[
                ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "user question".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                },
                ResponseItem::FunctionCall {
                    id: None,
                    name: "shell_command".to_string(),
                    arguments: serde_json::json!({
                        "command": format!("echo {}", "x".repeat(2_000)),
                    })
                    .to_string(),
                    call_id: "pending-call".to_string(),
                },
            ],
            turn_context.truncation_policy,
        );

        let deleted_items = trim_function_call_history_to_fit_context_window(
            &mut history,
            &turn_context,
            &BaseInstructions {
                text: "base".to_string(),
            },
        );

        assert_eq!(deleted_items, 0);
        assert!(
            history
                .raw_items()
                .iter()
                .any(|item| matches!(item, ResponseItem::FunctionCall { call_id, .. } if call_id == "pending-call")),
            "expected trailing function_call without output to be preserved"
        );
    }
}
