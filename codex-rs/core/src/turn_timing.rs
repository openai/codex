use std::time::Duration;
use std::time::Instant;

use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use tokio::sync::Mutex;

use crate::ResponseEvent;
use crate::codex::TurnContext;
use crate::stream_events_utils::raw_assistant_output_text_from_item;

const TURN_TTFT_DURATION_METRIC: &str = "codex.turn.ttft.duration_ms";
const TURN_TTFM_DURATION_METRIC: &str = "codex.turn.ttfm.duration_ms";

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
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::GhostSnapshot { .. }
        | ResponseItem::Compaction { .. } => true,
        ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::Other => false,
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::items::AgentMessageItem;
    use codex_protocol::items::TurnItem;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::models::ResponseItem;
    use pretty_assertions::assert_eq;
    use std::time::Instant;

    use super::TurnTimingState;
    use super::response_item_records_turn_ttft;
    use crate::ResponseEvent;

    #[tokio::test]
    async fn turn_timing_state_records_ttft_only_once_per_turn() {
        let state = TurnTimingState::default();
        assert_eq!(
            state
                .record_ttft_for_response_event(&ResponseEvent::OutputTextDelta("hi".to_string()))
                .await,
            None
        );

        state.mark_turn_started(Instant::now()).await;
        assert_eq!(
            state
                .record_ttft_for_response_event(&ResponseEvent::Created)
                .await,
            None
        );
        assert!(
            state
                .record_ttft_for_response_event(&ResponseEvent::OutputTextDelta("hi".to_string()))
                .await
                .is_some()
        );
        assert_eq!(
            state
                .record_ttft_for_response_event(&ResponseEvent::OutputTextDelta(
                    "again".to_string()
                ))
                .await,
            None
        );
    }

    #[tokio::test]
    async fn turn_timing_state_records_ttfm_independently_of_ttft() {
        let state = TurnTimingState::default();
        state.mark_turn_started(Instant::now()).await;

        assert!(
            state
                .record_ttft_for_response_event(&ResponseEvent::OutputTextDelta("hi".to_string()))
                .await
                .is_some()
        );
        assert!(
            state
                .record_ttfm_for_turn_item(&TurnItem::AgentMessage(AgentMessageItem {
                    id: "msg-1".to_string(),
                    content: Vec::new(),
                    phase: None,
                }))
                .await
                .is_some()
        );
        assert_eq!(
            state
                .record_ttfm_for_turn_item(&TurnItem::AgentMessage(AgentMessageItem {
                    id: "msg-2".to_string(),
                    content: Vec::new(),
                    phase: None,
                }))
                .await,
            None
        );
    }

    #[test]
    fn response_item_records_turn_ttft_for_first_output_signals() {
        assert!(response_item_records_turn_ttft(
            &ResponseItem::FunctionCall {
                id: None,
                name: "shell".to_string(),
                arguments: "{}".to_string(),
                call_id: "call-1".to_string(),
            }
        ));
        assert!(response_item_records_turn_ttft(
            &ResponseItem::CustomToolCall {
                id: None,
                status: None,
                call_id: "call-2".to_string(),
                name: "custom".to_string(),
                input: "echo hi".to_string(),
            }
        ));
        assert!(response_item_records_turn_ttft(&ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: "hello".to_string(),
            }],
            end_turn: None,
            phase: None,
        }));
    }

    #[test]
    fn response_item_records_turn_ttft_ignores_empty_non_output_items() {
        assert!(!response_item_records_turn_ttft(&ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText {
                text: String::new(),
            }],
            end_turn: None,
            phase: None,
        }));
        assert!(!response_item_records_turn_ttft(
            &ResponseItem::FunctionCallOutput {
                call_id: "call-1".to_string(),
                output: FunctionCallOutputPayload::from_text("ok".to_string()),
            }
        ));
    }
}
