use super::session::Session;
use crate::realtime_history::RealtimeEventEffects;
use codex_history::RolloutItem;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::realtime::RealtimeItemContent;

impl Session {
    /// Start and attach operations run sequentially in the submission loop.
    /// Cache enrollment after a successful lookup so a failed start can retry.
    pub(crate) async fn initialize_realtime_history(&self) -> std::io::Result<bool> {
        let Some(history) = &self.realtime_history else {
            return Ok(false);
        };
        if let Some(enabled) = history.lock().await.frame_enrollment {
            return Ok(enabled);
        }
        let enabled = self
            .services
            .thread_store
            .realtime_frames_enabled(self.thread_id())
            .await
            .map_err(std::io::Error::other)?;
        history.lock().await.frame_enrollment = Some(enabled);
        Ok(enabled)
    }

    pub(super) async fn send_realtime_history_effects(
        &self,
        submission_id: &str,
        effects: RealtimeEventEffects,
    ) -> anyhow::Result<()> {
        let event = |payload| Event {
            id: submission_id.to_string(),
            msg: EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                payload,
            }),
        };
        if let Some(stream) = effects.transcript_stream {
            if let Some(item) = stream.started_item {
                self.deliver_event_raw(event(RealtimeEvent::HistoryItemStarted(item)))
                    .await;
            }
            self.deliver_event_raw(event(RealtimeEvent::HistoryTranscriptDelta {
                item_id: stream.item_id,
                delta: stream.delta,
            }))
            .await;
        }
        if effects.items.is_empty() {
            return Ok(());
        }
        let live_thread = self.live_thread_for_persistence("append realtime history")?;
        let items = effects.items;
        let records = items
            .iter()
            .cloned()
            .map(RolloutItem::RealtimeItem)
            .collect::<Vec<_>>();
        live_thread.append_items(&records).await?;
        // A completed notification acknowledges durable history, including on hosts
        // whose append operation only admits work to an asynchronous writer.
        if effects.seal_frames {
            live_thread.flush().await?;
        }
        for item in items {
            if !matches!(
                &item.content,
                RealtimeItemContent::Frame { .. } | RealtimeItemContent::TranscriptSegment { .. }
            ) {
                self.deliver_event_raw(event(RealtimeEvent::HistoryItemStarted(item.clone())))
                    .await;
            }
            self.deliver_event_raw(event(RealtimeEvent::HistoryItemCompleted(item)))
                .await;
        }
        Ok(())
    }
}
