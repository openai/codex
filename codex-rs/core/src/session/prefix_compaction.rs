use std::sync::Arc;

use crate::compact::provider_supports_inline_remote_compaction;
use crate::compact_remote::run_remote_prefix_compact_task;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_features::Feature;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::items::ContextCompactionItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::CompactedItem;
use codex_protocol::protocol::TurnContextItem;
use tokio::task::JoinHandle;
use tracing::debug;
use tracing::trace;

pub(crate) type PrefixCompactTask = JoinHandle<CodexResult<PrefixCompactCandidate>>;

#[derive(Debug, Clone)]
pub(crate) struct PrefixCompactCandidate {
    base_history: Vec<ResponseItem>,
    replacement_prefix: Vec<ResponseItem>,
    captured_context: Vec<ResponseItem>,
    captured_reference_context_item: Option<TurnContextItem>,
}

pub(crate) async fn maybe_start_prefix_compact(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    total_usage_tokens: i64,
    auto_compact_limit: i64,
) {
    if !turn_context.features.enabled(Feature::PrefixCompaction) {
        return;
    }

    if !provider_supports_inline_remote_compaction(turn_context.provider.info()) {
        return;
    }

    let Some(prefix_compact_limit) = prefix_compact_token_limit(auto_compact_limit) else {
        return;
    };
    if total_usage_tokens < prefix_compact_limit || total_usage_tokens >= auto_compact_limit {
        return;
    }

    let captured_context = sess.build_initial_context(turn_context.as_ref()).await;
    let captured_reference_context_item = Some(turn_context.to_turn_context_item());

    let mut state = sess.state.lock().await;
    if state.prefix_compact_task.is_some() {
        trace!(
            turn_id = %turn_context.sub_id,
            total_usage_tokens,
            prefix_compact_limit,
            auto_compact_limit,
            "prefix compaction already running or ready"
        );
        return;
    }

    let base_history = state.history.raw_items().to_vec();
    if base_history.is_empty() {
        return;
    }

    let task = spawn_prefix_compact_task(
        Arc::clone(sess),
        Arc::clone(turn_context),
        base_history.clone(),
        captured_context,
        captured_reference_context_item,
    );
    state.prefix_compact_task = Some(task);

    debug!(
        turn_id = %turn_context.sub_id,
        model_slug = %turn_context.model_info.slug,
        total_usage_tokens,
        prefix_compact_limit,
        auto_compact_limit,
        base_history_len = base_history.len(),
        "starting background prefix compaction"
    );
}

pub(crate) async fn try_apply_ready_prefix_compact(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
) -> CodexResult<bool> {
    let Some(task) = take_finished_prefix_compact_task(sess).await else {
        return Ok(false);
    };

    let candidate = match task.await {
        Ok(Ok(candidate)) => candidate,
        Ok(Err(err)) => {
            debug!(
                turn_id = %turn_context.sub_id,
                "prefix compaction failed: {err:#}"
            );
            return Ok(false);
        }
        Err(err) => {
            debug!(
                turn_id = %turn_context.sub_id,
                "prefix compaction task failed: {err:#}"
            );
            return Ok(false);
        }
    };

    if apply_prefix_compact_candidate(sess, turn_context, candidate).await? {
        return Ok(true);
    }

    debug!(
        turn_id = %turn_context.sub_id,
        "prefix compaction candidate is stale; running foreground auto-compaction"
    );
    Ok(false)
}

pub(crate) async fn abandon_prefix_compact(sess: &Arc<Session>) {
    let mut state = sess.state.lock().await;
    abort_prefix_compact_task(&mut state.prefix_compact_task);
}

pub(crate) fn abort_prefix_compact_task(task: &mut Option<PrefixCompactTask>) {
    if let Some(task) = task.take() {
        task.abort();
    }
}

pub(super) fn prefix_compact_token_limit(auto_compact_limit: i64) -> Option<i64> {
    if auto_compact_limit == i64::MAX || auto_compact_limit <= 1 {
        return None;
    }

    let token_limit = auto_compact_limit.saturating_mul(60) / 100;
    let token_limit = token_limit.clamp(1, auto_compact_limit.saturating_sub(1));
    Some(token_limit)
}

async fn take_finished_prefix_compact_task(sess: &Arc<Session>) -> Option<PrefixCompactTask> {
    let mut state = sess.state.lock().await;
    if state
        .prefix_compact_task
        .as_ref()
        .is_some_and(tokio::task::JoinHandle::is_finished)
    {
        state.prefix_compact_task.take()
    } else {
        None
    }
}

async fn apply_prefix_compact_candidate(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    candidate: PrefixCompactCandidate,
) -> CodexResult<bool> {
    let current_history = sess.clone_history().await;
    let current_items = current_history.raw_items();
    if current_items.len() < candidate.base_history.len()
        || current_items[..candidate.base_history.len()] != candidate.base_history
    {
        debug!(
            turn_id = %turn_context.sub_id,
            current_history_len = current_items.len(),
            base_history_len = candidate.base_history.len(),
            "prefix compaction candidate no longer matches current history"
        );
        return Ok(false);
    }

    let compaction_item = TurnItem::ContextCompaction(ContextCompactionItem::new_prefix());
    sess.emit_turn_item_started(turn_context, &compaction_item)
        .await;

    let retained_suffix: Vec<ResponseItem> = current_items[candidate.base_history.len()..]
        .iter()
        .filter(|item| !matches!(item, ResponseItem::GhostSnapshot { .. }))
        .cloned()
        .collect();
    let reference_context_item = sess
        .reference_context_item()
        .await
        .or(candidate.captured_reference_context_item);
    let mut new_history = candidate.replacement_prefix;
    new_history.extend(candidate.captured_context);
    new_history.extend(retained_suffix);

    new_history.extend(
        current_items
            .iter()
            .filter(|item| matches!(item, ResponseItem::GhostSnapshot { .. }))
            .cloned(),
    );

    let compacted_item = CompactedItem {
        message: String::new(),
        replacement_history: Some(new_history.clone()),
    };
    sess.replace_compacted_history(new_history, reference_context_item, compacted_item)
        .await;
    sess.recompute_token_usage(turn_context).await;
    sess.emit_turn_item_completed(turn_context, compaction_item)
        .await;
    debug!(
        turn_id = %turn_context.sub_id,
        "applied prefix compaction candidate"
    );
    Ok(true)
}

fn spawn_prefix_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    base_history: Vec<ResponseItem>,
    captured_context: Vec<ResponseItem>,
    captured_reference_context_item: Option<TurnContextItem>,
) -> PrefixCompactTask {
    tokio::spawn(async move {
        let replacement_prefix = run_remote_prefix_compact_task(
            Arc::clone(&sess),
            Arc::clone(&turn_context),
            base_history.clone(),
        )
        .await?;
        debug!(
            turn_id = %turn_context.sub_id,
            model_slug = %turn_context.model_info.slug,
            base_history_len = base_history.len(),
            replacement_prefix_len = replacement_prefix.len(),
            captured_context_len = captured_context.len(),
            "background prefix compaction ready"
        );
        Ok(PrefixCompactCandidate {
            base_history,
            replacement_prefix,
            captured_context,
            captured_reference_context_item,
        })
    })
}
