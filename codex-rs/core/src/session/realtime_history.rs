use super::session::Session;
use crate::realtime_history::RealtimeEventEffects;
use codex_history::RolloutItem;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::realtime::RealtimeItemContent;

fn realtime_store_read_error(error: codex_thread_store::ThreadStoreError) -> std::io::Error {
    match error {
        error @ codex_thread_store::ThreadStoreError::Retryable { .. } => {
            std::io::Error::new(std::io::ErrorKind::WouldBlock, error)
        }
        error => std::io::Error::other(error),
    }
}

impl Session {
    /// A successful close must mean the canonical close marker was sealed.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "Keep realtime event reduction, close persistence, and delivery in the same order."
    )]
    pub(crate) async fn seal_realtime_close(&self, event: Event) -> anyhow::Result<()> {
        let Some(history) = &self.realtime_history else {
            self.send_event_raw(event).await;
            return Ok(());
        };
        let mut history = history.lock().await;
        if !history.frames_enabled() {
            drop(history);
            self.send_event_raw(event).await;
            return Ok(());
        }
        self.services.mcp_runtime.observe_event(&event.msg);
        let effects = history.observe(&event.msg);
        self.send_realtime_history_effects(&event.id, effects)
            .await?;
        self.services
            .rollout_thread_trace
            .record_protocol_event(&event.msg);
        self.deliver_event_raw(event).await;
        Ok(())
    }

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
            .map_err(realtime_store_read_error)?;
        let floor = if enabled
            && matches!(
                self.history_initialization,
                codex_history::HistoryInitialization::ColdResume
            ) {
            self.services
                .thread_store
                .realtime_frame_clock_floor(self.thread_id())
                .await
                .map_err(realtime_store_read_error)?
        } else {
            None
        };
        let mut history = history.lock().await;
        if let Some(floor) = floor {
            history.observation_clock.observe(floor);
        }
        history.frame_enrollment = Some(enabled);
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
        if effects.seal_frames {
            live_thread.seal_realtime_items(items.clone()).await?;
        } else {
            let records = items
                .iter()
                .cloned()
                .map(RolloutItem::RealtimeItem)
                .collect::<Vec<_>>();
            live_thread.append_items(&records).await?;
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
