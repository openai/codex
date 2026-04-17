mod log_entries;
mod prompt;
mod storage;
mod storage_tools;
mod transcript;

use std::sync::Arc;

use crate::RolloutRecorder;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::compact::CompactionAnalyticsAttempt;
use crate::compact::InitialContextInjection;
use crate::compact::compaction_status_from_result;
use crate::compact::insert_initial_context_before_last_real_user_or_summary;
use codex_analytics::CompactionImplementation;
use codex_analytics::CompactionPhase;
use codex_analytics::CompactionReason;
use codex_analytics::CompactionStrategy;
use codex_analytics::CompactionTrigger;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::items::ContextCompactionItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::CompactedItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnStartedEvent;

pub(crate) use prompt::is_near_limit_reminder;
pub(crate) use prompt::near_limit_reminder;
pub(crate) use prompt::near_limit_reminder_threshold;
pub(crate) use prompt::usage_hint;
pub(crate) use storage::ensure_sidecar_dirs;
pub(crate) use storage::resolve_reflections_shared_notes_path;
pub(crate) use storage::sidecar_path_for_rollout;
pub(crate) use storage_tools::StorageToolError;
pub(crate) use storage_tools::list_logs;
pub(crate) use storage_tools::list_notes;
pub(crate) use storage_tools::list_shared_notes;
pub(crate) use storage_tools::read_log;
pub(crate) use storage_tools::read_note;
pub(crate) use storage_tools::read_shared_note;
pub(crate) use storage_tools::search;
pub(crate) use storage_tools::search_shared_notes;
pub(crate) use storage_tools::write_note;
pub(crate) use storage_tools::write_shared_note;

pub(crate) async fn run_reflections_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) -> CodexResult<()> {
    let start_event = EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: turn_context.sub_id.clone(),
        started_at: turn_context.turn_timing_state.started_at_unix_secs().await,
        model_context_window: turn_context.model_context_window(),
        collaboration_mode_kind: turn_context.collaboration_mode.mode,
    });
    sess.send_event(&turn_context, start_event).await;

    run_reflections_compact_task_inner(
        sess,
        turn_context,
        InitialContextInjection::DoNotInject,
        CompactionTrigger::Manual,
        CompactionReason::UserRequested,
        CompactionPhase::StandaloneTurn,
    )
    .await
}

pub(crate) async fn run_inline_reflections_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    initial_context_injection: InitialContextInjection,
    reason: CompactionReason,
    phase: CompactionPhase,
) -> CodexResult<()> {
    run_reflections_compact_task_inner(
        sess,
        turn_context,
        initial_context_injection,
        CompactionTrigger::Auto,
        reason,
        phase,
    )
    .await
}

pub(crate) async fn run_reflections_compact_task_inner(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    initial_context_injection: InitialContextInjection,
    trigger: CompactionTrigger,
    reason: CompactionReason,
    phase: CompactionPhase,
) -> CodexResult<()> {
    let attempt = CompactionAnalyticsAttempt::begin_with_strategy(
        sess.as_ref(),
        turn_context.as_ref(),
        trigger,
        reason,
        CompactionImplementation::Reflections,
        phase,
        CompactionStrategy::Reflections,
    )
    .await;
    let result = run_reflections_compact_task_inner_impl(
        Arc::clone(&sess),
        Arc::clone(&turn_context),
        initial_context_injection,
        trigger,
    )
    .await;
    attempt
        .track(
            sess.as_ref(),
            compaction_status_from_result(&result),
            result.as_ref().err().map(ToString::to_string),
        )
        .await;
    if let Err(err) = &result {
        let event = EventMsg::Error(
            err.to_error_event(Some("Error running reflections compaction".to_string())),
        );
        sess.send_event(&turn_context, event).await;
    }
    result
}

async fn run_reflections_compact_task_inner_impl(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    initial_context_injection: InitialContextInjection,
    trigger: CompactionTrigger,
) -> CodexResult<()> {
    let compaction_item = TurnItem::ContextCompaction(ContextCompactionItem::new());
    sess.emit_turn_item_started(&turn_context, &compaction_item)
        .await;

    let window = write_current_window(&sess, &turn_context, trigger).await?;
    let handoff = prompt::post_compaction_handoff(
        turn_context.model_context_window(),
        &window.logs_path,
        &window.notes_path,
        turn_context.config.reflections.storage_tools_enabled,
        turn_context.reflections_shared_notes_path.is_some()
            && turn_context.config.reflections.storage_tools_enabled
            && turn_context.config.reflections.shared_notes_enabled,
    );
    let mut new_history = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: handoff.clone(),
        }],
        end_turn: None,
        phase: None,
    }];

    if matches!(
        initial_context_injection,
        InitialContextInjection::BeforeLastUserMessage
    ) {
        let initial_context = sess.build_initial_context(turn_context.as_ref()).await;
        new_history =
            insert_initial_context_before_last_real_user_or_summary(new_history, initial_context);
    }

    let history_snapshot = sess.clone_history().await;
    let ghost_snapshots: Vec<ResponseItem> = history_snapshot
        .raw_items()
        .iter()
        .filter(|item| matches!(item, ResponseItem::GhostSnapshot { .. }))
        .cloned()
        .collect();
    new_history.extend(ghost_snapshots);

    let reference_context_item = match initial_context_injection {
        InitialContextInjection::DoNotInject => None,
        InitialContextInjection::BeforeLastUserMessage => Some(turn_context.to_turn_context_item()),
    };
    let compacted_item = CompactedItem {
        message: handoff,
        replacement_history: Some(new_history.clone()),
    };
    sess.replace_compacted_history(new_history, reference_context_item, compacted_item)
        .await;
    sess.reset_reflections_near_limit_reminder();
    sess.recompute_token_usage(&turn_context).await;
    sess.emit_turn_item_completed(&turn_context, compaction_item)
        .await;
    Ok(())
}

pub(crate) async fn write_current_window(
    sess: &Session,
    turn_context: &TurnContext,
    trigger: CompactionTrigger,
) -> CodexResult<storage::WrittenWindow> {
    sess.try_ensure_rollout_materialized().await?;
    sess.flush_rollout().await?;
    let Some(rollout_path) = sess.current_rollout_path().await else {
        return Err(CodexErr::InvalidRequest(
            "Reflections requires a persisted rollout path and is unavailable for ephemeral sessions"
                .to_string(),
        ));
    };
    let sidecar_path = storage::sidecar_path_for_rollout(&rollout_path);
    let (rollout_items, _, _) = RolloutRecorder::load_rollout_items(&rollout_path).await?;
    let item_range = transcript::item_range_since_last_compaction(&rollout_items);
    let rollout_start_line = item_range.start.saturating_add(1);
    let rollout_end_line = item_range.end;
    let events = transcript::events_since_last_compaction(&rollout_items);
    let transcript = transcript::render(transcript::TranscriptInput {
        events: &events,
        trigger,
        context_window_size: turn_context.model_context_window(),
        rollout_path: &rollout_path,
    });

    storage::write_window(
        &sidecar_path,
        &rollout_path,
        trigger,
        turn_context.model_context_window(),
        rollout_start_line,
        rollout_end_line,
        transcript,
    )
    .await
    .map_err(Into::into)
}
