//! Records realtime transcript chunks in model and client history without inference.

use crate::session::new_submission_id;
use crate::session::session::Session;
use codex_history::RealtimeSessionInputMetadata;
use codex_history::ResponseItemEnvelope;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::HasLegacyEvent;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_rollout::RolloutItem;
use codex_thread_store::PersistContext;
use codex_user_input::ResolvedUserInput;
use std::sync::Arc;

pub(super) async fn record(session: &Session, text: String) -> std::io::Result<()> {
    record_input(session, text, /*metadata*/ None).await
}

pub(super) async fn record_chunk(
    session: &Session,
    text: String,
    metadata: RealtimeSessionInputMetadata,
) -> std::io::Result<()> {
    record_input(session, text, Some(metadata)).await
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "record history before the owning task can finish or a new task can start"
)]
async fn record_input(
    session: &Session,
    text: String,
    metadata: Option<RealtimeSessionInputMetadata>,
) -> std::io::Result<()> {
    let mut recording_context = session.new_inject_items_context().await;
    Arc::get_mut(&mut recording_context)
        .ok_or_else(|| std::io::Error::other("realtime transcript context is shared"))?
        .sub_id = new_submission_id();
    let active = session.active_turn.lock().await;
    let running_turn = active
        .as_ref()
        .and_then(|turn| turn.task.as_ref())
        .map(|task| Arc::clone(&task.turn_context));
    let recording_turn = running_turn.is_none();
    let turn = running_turn.unwrap_or(recording_context);
    let input = ResolvedUserInput::text(text);
    let now = chrono::Utc::now();
    if recording_turn {
        let started = EventMsg::TurnStarted(TurnStartedEvent {
            turn_attribution: None,
            turn_id: turn.sub_id.clone(),
            root_turn_id: Some(turn.sub_id.clone()),
            trace_id: turn.trace_id.clone(),
            started_at: Some(now.timestamp()),
            model_context_window: turn.model_context_window(),
            collaboration_mode_kind: turn.mode(),
        });
        session
            .persist_rollout_items(&[RolloutItem::EventMsg(started)])
            .await;
        session
            .record_started_turn(&turn.sub_id, /*attribution*/ None)
            .await;
    }
    let item = session.response_item_from_user_input(std::slice::from_ref(&input));
    if let Some(metadata) = metadata {
        let mut envelope = ResponseItemEnvelope::new(item);
        envelope
            .metadata
            .get_or_insert_default()
            .realtime_session_input = Some(Box::new(metadata));
        session
            .record_realtime_session_chunk(&turn, envelope)
            .await?;
    } else {
        session
            .record_conversation_items(&turn, turn.model_info(), &[item])
            .await;
    }

    // ItemCompleted is the existing source for thread/items/list. Keep the legacy
    // equivalent too; the store selects the appropriate format for this thread.
    let completed = EventMsg::ItemCompleted(ItemCompletedEvent {
        content_token_count: None,
        thread_id: session.thread_id,
        turn_id: turn.sub_id.clone(),
        item: TurnItem::UserMessage(UserMessageItem::new(&[input.to_user_input()])),
        started_at_ms: Some(now.timestamp_millis()),
        completed_at_ms: now.timestamp_millis(),
    });
    let mut events = Vec::new();
    let legacy = completed.as_legacy_events(/*show_raw_agent_reasoning*/ false);
    events.push(completed);
    events.extend(legacy);
    if recording_turn {
        events.push(EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: turn.sub_id.clone(),
            root_turn_id: Some(turn.sub_id.clone()),
            last_agent_message: None,
            error: None,
            started_at: Some(now.timestamp()),
            completed_at: Some(now.timestamp()),
            duration_ms: Some(0),
            time_to_first_token_ms: None,
        }));
    }
    // Persist the complete history record without advertising a running UI turn
    // or completing the model task that may still be streaming.
    session
        .persist_rollout_items(
            &events
                .into_iter()
                .map(RolloutItem::EventMsg)
                .collect::<Vec<_>>(),
        )
        .await;
    drop(active);
    session
        .try_ensure_rollout_materialized(PersistContext::Standard)
        .await?;
    session.flush_rollout().await
}
