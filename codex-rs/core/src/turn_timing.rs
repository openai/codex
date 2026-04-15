use std::time::Duration;

use codex_otel::metrics::names::TURN_TTFM_DURATION_METRIC;
use codex_otel::metrics::names::TURN_TTFT_DURATION_METRIC;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use tokio::sync::Mutex;

use crate::ResponseEvent;
use crate::codex::TurnContext;
use crate::monotonic_time::Instant;
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
    first_token_at: Option<Instant>,
    first_message_at: Option<Instant>,
}

impl TurnTimingState {
    pub(crate) async fn mark_turn_started(&self, started_at: Instant) {
        let mut state = self.state.lock().await;
        state.started_at = Some(started_at);
        state.first_token_at = None;
        state.first_message_at = None;
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
