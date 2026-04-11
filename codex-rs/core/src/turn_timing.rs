use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_otel::TURN_TTFM_DURATION_METRIC;
use codex_otel::TURN_TTFT_DURATION_METRIC;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use tokio::sync::Mutex;

use crate::ResponseEvent;
use crate::codex::TurnContext;
use crate::stream_events_utils::raw_assistant_output_text_from_item;

pub(crate) async fn record_turn_ttft_metric(turn_context: &TurnContext, event: &ResponseEvent) {
    let Some(duration) = turn_context
        .turn_timing_state
        .record_ttft_for_response_event(event)
        .await
    else {
        return;
    };
    turn_context
        .session_telemetry
        .record_duration(TURN_TTFT_DURATION_METRIC, duration, &[]);
}

pub(crate) async fn record_turn_ttfm_metric(turn_context: &TurnContext, item: &TurnItem) {
    let Some(duration) = turn_context
        .turn_timing_state
        .record_ttfm_for_turn_item(item)
        .await
    else {
        return;
    };
    turn_context
        .session_telemetry
        .record_duration(TURN_TTFM_DURATION_METRIC, duration, &[]);
}

#[derive(Debug, Default)]
pub(crate) struct TurnTimingState {
    state: Mutex<TurnTimingStateInner>,
}

#[derive(Debug, Default)]
struct TurnTimingStateInner {
    started_at: Option<Instant>,
    started_at_unix_secs: Option<i64>,
    first_token_at: Option<Instant>,
    first_message_at: Option<Instant>,
}

impl TurnTimingState {
    pub(crate) async fn mark_turn_started(&self, started_at: Instant) {
        let mut state = self.state.lock().await;
        state.started_at = Some(started_at);
        state.started_at_unix_secs = Some(now_unix_timestamp_secs());
        state.first_token_at = None;
        state.first_message_at = None;
    }

    pub(crate) async fn started_at_unix_secs(&self) -> Option<i64> {
        self.state.lock().await.started_at_unix_secs
    }

    pub(crate) async fn completed_at_and_duration_ms(&self) -> (Option<i64>, Option<i64>) {
        let state = self.state.lock().await;
        let completed_at = Some(now_unix_timestamp_secs());
        let duration_ms = state
            .started_at
            .map(|started_at| i64::try_from(started_at.elapsed().as_millis()).unwrap_or(i64::MAX));
        (completed_at, duration_ms)
    }

    pub(crate) async fn record_ttft_for_response_event(
        &self,
        event: &ResponseEvent,
    ) -> Option<Duration> {
        if !response_event_records_turn_ttft(event) {
            return None;
        }
        let mut state = self.state.lock().await;
        state.record_turn_ttft()
    }

    pub(crate) async fn record_ttfm_for_turn_item(&self, item: &TurnItem) -> Option<Duration> {
        if !matches!(item, TurnItem::AgentMessage(_)) {
            return None;
        }
        let mut state = self.state.lock().await;
        state.record_turn_ttfm()
    }
}

fn now_unix_timestamp_secs() -> i64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
}

impl TurnTimingStateInner {
    fn record_turn_ttft(&mut self) -> Option<Duration> {
        if self.first_token_at.is_some() {
            return None;
        }
        let started_at = self.started_at?;
        let first_token_at = Instant::now();
        self.first_token_at = Some(first_token_at);
        Some(first_token_at.duration_since(started_at))
    }

    fn record_turn_ttfm(&mut self) -> Option<Duration> {
        if self.first_message_at.is_some() {
            return None;
        }
        let started_at = self.started_at?;
        let first_message_at = Instant::now();
        self.first_message_at = Some(first_message_at);
        Some(first_message_at.duration_since(started_at))
    }
}

fn response_event_records_turn_ttft(event: &ResponseEvent) -> bool {
    match event {
        ResponseEvent::OutputItemDone(item) | ResponseEvent::OutputItemAdded(item) => {
            response_item_records_turn_ttft(item)
        }
        ResponseEvent::OutputTextDelta(_)
        | ResponseEvent::ReasoningSummaryDelta { .. }
        | ResponseEvent::ReasoningContentDelta { .. } => true,
        ResponseEvent::Created
        | ResponseEvent::ServerModel(_)
        | ResponseEvent::ServerReasoningIncluded(_)
        | ResponseEvent::ToolCallInputDelta { .. }
        | ResponseEvent::Completed { .. }
        | ResponseEvent::ReasoningSummaryPartAdded { .. }
        | ResponseEvent::RateLimits(_)
        | ResponseEvent::ModelsEtag(_) => false,
    }
}

fn response_item_records_turn_ttft(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { .. } => {
            raw_assistant_output_text_from_item(item).is_some_and(|text| !text.is_empty())
        }
        ResponseItem::Reasoning {
            summary, content, ..
        } => {
            summary.iter().any(|entry| match entry {
                codex_protocol::models::ReasoningItemReasoningSummary::SummaryText { text } => {
                    !text.is_empty()
                }
            }) || content.as_ref().is_some_and(|entries| {
                entries.iter().any(|entry| match entry {
                    codex_protocol::models::ReasoningItemContent::ReasoningText { text }
                    | codex_protocol::models::ReasoningItemContent::Text { text } => {
                        !text.is_empty()
                    }
                })
            })
        }
        ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::ToolSearchCall { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::GhostSnapshot { .. }
        | ResponseItem::Compaction { .. } => true,
        ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::ToolSearchOutput { .. }
        | ResponseItem::Other => false,
    }
}

#[cfg(test)]
#[path = "turn_timing_tests.rs"]
mod tests;
