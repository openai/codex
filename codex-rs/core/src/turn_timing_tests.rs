use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

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
            .record_ttft_for_response_event(&ResponseEvent::OutputTextDelta("again".to_string()))
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
                memory_citation: None,
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
                memory_citation: None,
            }))
            .await,
        None
    );
}

#[tokio::test]
async fn turn_timing_state_records_turn_started_epoch_millis() {
    let state = TurnTimingState::default();
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis();

    let started_at_unix_ms = state.mark_turn_started(Instant::now()).await;

    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis();
    assert!(u128::try_from(started_at_unix_ms).is_ok_and(|ms| before <= ms && ms <= after));
    assert_eq!(
        state.started_at_unix_secs().await,
        Some(started_at_unix_ms / 1000)
    );
}

#[test]
fn response_item_records_turn_ttft_for_first_output_signals() {
    assert!(response_item_records_turn_ttft(
        &ResponseItem::FunctionCall {
            id: None,
            name: "shell".to_string(),
            namespace: None,
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
