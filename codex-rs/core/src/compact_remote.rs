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
use codex_protocol::models::ResponseItem;

pub(crate) async fn run_remote_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) -> Option<String> {
    let start_event = EventMsg::TaskStarted(TaskStartedEvent {
        model_context_window: turn_context.client.get_model_context_window(),
    });
    sess.send_event(&turn_context, start_event).await;

    match run_remote_compact_task_inner(&sess, &turn_context).await {
        Ok(()) => {
            let event = EventMsg::AgentMessage(AgentMessageEvent {
                message: "Compact task completed".to_string(),
            });
            sess.send_event(&turn_context, event).await;
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
) -> CodexResult<()> {
    let mut history = sess.clone_history().await;
    let prompt = Prompt {
        input: history.get_history_for_prompt(),
        tools: vec![],
        parallel_tool_calls: false,
        base_instructions_override: turn_context.base_instructions.clone(),
        output_schema: None,
    };

    let mut new_history = turn_context
        .client
        .compact_conversation_history(&prompt)
        .await?;
    // Required to keep `/undo` available after compaction
    let ghost_snapshots: Vec<ResponseItem> = history
        .get_history()
        .iter()
        .filter(|item| matches!(item, ResponseItem::GhostSnapshot { .. }))
        .cloned()
        .collect();

    if !ghost_snapshots.is_empty() {
        new_history.extend(ghost_snapshots);
    }
    sess.replace_history(new_history.clone()).await;

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
        replacement_history: Some(new_history),
    };
    sess.persist_rollout_items(&[RolloutItem::Compacted(compacted_item)])
        .await;
    Ok(())
}
