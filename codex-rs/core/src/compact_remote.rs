use std::sync::Arc;

use crate::Prompt;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::error::Result as CodexResult;
use crate::hooks::run_post_compact_hooks;
use crate::hooks::run_pre_compact_hooks;
use crate::protocol::CompactedItem;
use crate::protocol::ContextCompactedEvent;
use crate::protocol::EventMsg;
use crate::protocol::RolloutItem;
use crate::protocol::TurnStartedEvent;
use codex_protocol::models::ResponseItem;
use tracing::warn;

pub(crate) async fn run_inline_remote_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) {
    let reason = "auto-remote";
    if let Err(err) = run_pre_compact_hooks(sess.as_ref(), turn_context.as_ref(), reason).await {
        warn!("pre compact hook execution failed: {err}");
    }
    run_remote_compact_task_inner(&sess, &turn_context, reason).await;
}

pub(crate) async fn run_remote_compact_task(sess: Arc<Session>, turn_context: Arc<TurnContext>) {
    let reason = "manual-remote";
    if let Err(err) = run_pre_compact_hooks(sess.as_ref(), turn_context.as_ref(), reason).await {
        warn!("pre compact hook execution failed: {err}");
    }
    let start_event = EventMsg::TurnStarted(TurnStartedEvent {
        model_context_window: turn_context.client.get_model_context_window(),
    });
    sess.send_event(&turn_context, start_event).await;

    run_remote_compact_task_inner(&sess, &turn_context, reason).await;
}

async fn run_remote_compact_task_inner(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    reason: &str,
) {
    if let Err(err) = run_remote_compact_task_inner_impl(sess, turn_context).await {
        let event = EventMsg::Error(
            err.to_error_event(Some("Error running remote compact task".to_string())),
        );
        sess.send_event(turn_context, event).await;
        return;
    }

    if let Err(err) = run_post_compact_hooks(sess.as_ref(), turn_context.as_ref(), reason).await {
        warn!("post compact hook execution failed: {err}");
    }
}

async fn run_remote_compact_task_inner_impl(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
) -> CodexResult<()> {
    let history = sess.clone_history().await;

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
        base_instructions: sess.get_base_instructions().await,
        personality: turn_context.personality,
        output_schema: None,
    };

    let mut new_history = turn_context
        .client
        .compact_conversation_history(&prompt)
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

    let event = EventMsg::ContextCompacted(ContextCompactedEvent {});
    sess.send_event(turn_context, event).await;

    Ok(())
}
