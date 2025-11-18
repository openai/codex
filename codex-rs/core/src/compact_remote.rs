use std::sync::Arc;

use crate::Prompt;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::error::Result as CodexResult;
use crate::protocol::AgentMessageEvent;
use crate::protocol::CompactedItem;
use crate::protocol::ErrorEvent;
use crate::protocol::EventMsg;
use crate::protocol::RolloutItem;
use crate::protocol::TaskStartedEvent;
use crate::protocol::WarningEvent;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;

pub(crate) async fn run_remote_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    input: Vec<UserInput>,
) -> Option<String> {
    let start_event = EventMsg::TaskStarted(TaskStartedEvent {
        model_context_window: turn_context.client.get_model_context_window(),
    });
    sess.send_event(&turn_context, start_event).await;

    match run_remote_compact_task_inner(&sess, &turn_context, input).await {
        Ok(()) => {
            let event = EventMsg::AgentMessage(AgentMessageEvent {
                message: "Compact task completed".to_string(),
            });
            sess.send_event(&turn_context, event).await;

            let warning = EventMsg::Warning(WarningEvent {
                message: "Heads up: Long conversations and multiple compactions can cause the model to be less accurate. Start a new conversation when possible to keep conversations small and targeted.".to_string(),
            });
            sess.send_event(&turn_context, warning).await;
        }
        Err(err) => {
            let event = EventMsg::Error(ErrorEvent {
                message: err.to_string(),
            });
            sess.send_event(&turn_context, event).await;
        }
    }

    None
}

async fn run_remote_compact_task_inner(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    input: Vec<UserInput>,
) -> CodexResult<()> {
    let mut history = sess.clone_history().await;
    if !input.is_empty() {
        let initial_input_for_turn: ResponseInputItem = ResponseInputItem::from(input);
        history.record_items(&[initial_input_for_turn.into()]);
    }

    let prompt = Prompt {
        input: history.get_history_for_prompt(),
        tools: vec![],
        parallel_tool_calls: false,
        base_instructions_override: turn_context.base_instructions.clone(),
        output_schema: None,
    };

    let compacted_items = turn_context
        .client
        .compact_conversation_history(&prompt)
        .await?;
    let ghost_snapshots: Vec<ResponseItem> = history
        .get_history()
        .iter()
        .filter(|item| matches!(item, ResponseItem::GhostSnapshot { .. }))
        .cloned()
        .collect();
    let mut new_history = sess.build_initial_context(turn_context.as_ref());
    new_history.extend(compacted_items.clone());
    if !ghost_snapshots.is_empty() {
        new_history.extend(ghost_snapshots);
    }
    sess.replace_history(new_history).await;

    if let Some(estimated_tokens) = sess
        .clone_history()
        .await
        .estimate_token_count(turn_context.as_ref())
    {
        sess.override_last_token_usage_estimate(turn_context.as_ref(), estimated_tokens)
            .await;
    }

    let compacted_item = CompactedItem {
        message: String::new(),
        replacement_history: Some(compacted_items),
    };
    sess.persist_rollout_items(&[RolloutItem::Compacted(compacted_item)])
        .await;
    Ok(())
}
