use crate::outgoing_message::ThreadScopedOutgoingMessageSender;
use crate::realtime_history::RealtimeEventEffects;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadRealtimeItemCompletedNotification;
use codex_app_server_protocol::ThreadRealtimeItemStartedNotification;
use codex_app_server_protocol::ThreadRealtimeItemTranscriptDeltaNotification;
use codex_core::CodexThread;
use codex_protocol::ThreadId;
use codex_protocol::realtime::RealtimeItem;
use codex_protocol::realtime::RealtimeItemContent;
use codex_rollout::RolloutItem;
use tracing::warn;

pub(crate) async fn apply_realtime_event_effects(
    conversation: &CodexThread,
    outgoing: &ThreadScopedOutgoingMessageSender,
    thread_id: ThreadId,
    effects: RealtimeEventEffects,
) {
    let thread_id = thread_id.to_string();

    if let Some(stream) = effects.transcript_stream {
        if let Some(item) = stream.started_item {
            outgoing
                .send_server_notification(ServerNotification::ThreadRealtimeItemStarted(
                    ThreadRealtimeItemStartedNotification {
                        thread_id: thread_id.clone(),
                        item: item.into(),
                    },
                ))
                .await;
        }
        outgoing
            .send_server_notification(ServerNotification::ThreadRealtimeItemTranscriptDelta(
                ThreadRealtimeItemTranscriptDeltaNotification {
                    thread_id: thread_id.clone(),
                    item_id: stream.item_id,
                    delta: stream.delta,
                },
            ))
            .await;
    }

    if let Err(error) =
        persist_realtime_items(conversation, outgoing, &thread_id, effects.items).await
    {
        warn!(thread_id, "failed to persist realtime history: {error}");
    }
}

pub(crate) async fn persist_realtime_items(
    conversation: &CodexThread,
    outgoing: &ThreadScopedOutgoingMessageSender,
    thread_id: &str,
    items: Vec<RealtimeItem>,
) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }
    conversation
        .append_rollout_items(
            &items
                .iter()
                .cloned()
                .map(RolloutItem::RealtimeItem)
                .collect::<Vec<_>>(),
        )
        .await
        .map_err(|error| format!("failed to persist realtime history: {error}"))?;
    for item in items {
        if !matches!(&item.content, RealtimeItemContent::TranscriptSegment { .. }) {
            outgoing
                .send_server_notification(ServerNotification::ThreadRealtimeItemStarted(
                    ThreadRealtimeItemStartedNotification {
                        thread_id: thread_id.to_string(),
                        item: item.clone().into(),
                    },
                ))
                .await;
        }
        outgoing
            .send_server_notification(ServerNotification::ThreadRealtimeItemCompleted(
                ThreadRealtimeItemCompletedNotification {
                    thread_id: thread_id.to_string(),
                    item: item.into(),
                },
            ))
            .await;
    }
    Ok(())
}
