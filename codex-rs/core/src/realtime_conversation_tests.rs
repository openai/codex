use super::HandoffOutput;
use super::RealtimeHandoffState;
use super::realtime_text_from_handoff_request;
use async_channel::bounded;
use codex_protocol::protocol::RealtimeHandoffRequested;
use codex_protocol::protocol::RealtimeTranscriptEntry;
use pretty_assertions::assert_eq;

#[test]
fn extracts_text_from_handoff_request_active_transcript() {
    let handoff = RealtimeHandoffRequested {
        handoff_id: "handoff_1".to_string(),
        item_id: "item_1".to_string(),
        input_transcript: "ignored".to_string(),
        active_transcript: vec![
            RealtimeTranscriptEntry {
                role: "user".to_string(),
                text: "hello".to_string(),
            },
            RealtimeTranscriptEntry {
                role: "assistant".to_string(),
                text: "hi there".to_string(),
            },
        ],
    };
    assert_eq!(
        realtime_text_from_handoff_request(&handoff),
        Some("user: hello\nassistant: hi there".to_string())
    );
}

#[test]
fn extracts_text_from_handoff_request_input_transcript_if_messages_missing() {
    let handoff = RealtimeHandoffRequested {
        handoff_id: "handoff_1".to_string(),
        item_id: "item_1".to_string(),
        input_transcript: "ignored".to_string(),
        active_transcript: vec![],
    };
    assert_eq!(
        realtime_text_from_handoff_request(&handoff),
        Some("ignored".to_string())
    );
}

#[test]
fn ignores_empty_handoff_request_input_transcript() {
    let handoff = RealtimeHandoffRequested {
        handoff_id: "handoff_1".to_string(),
        item_id: "item_1".to_string(),
        input_transcript: String::new(),
        active_transcript: vec![],
    };
    assert_eq!(realtime_text_from_handoff_request(&handoff), None);
}

#[tokio::test]
async fn clears_active_handoff_explicitly() {
    let (tx, _rx) = bounded(1);
    let state = RealtimeHandoffState::new(tx, false);

    *state.active_handoff.lock().await = Some("handoff_1".to_string());
    assert_eq!(
        state.active_handoff.lock().await.clone(),
        Some("handoff_1".to_string())
    );

    *state.active_handoff.lock().await = None;
    assert_eq!(state.active_handoff.lock().await.clone(), None);
}

#[tokio::test]
async fn sends_multiple_handoff_outputs_until_cleared() {
    let (tx, rx) = bounded(4);
    let state = RealtimeHandoffState::new(tx, false);

    state
        .send_output("ignored".to_string())
        .await
        .expect("send");
    assert!(rx.is_empty());

    *state.active_handoff.lock().await = Some("handoff_1".to_string());
    state.send_output("result".to_string()).await.expect("send");
    state
        .send_output("result 2".to_string())
        .await
        .expect("send");

    let output_1 = rx.recv().await.expect("recv");
    assert_eq!(
        output_1,
        HandoffOutput::ImmediateAppend {
            handoff_id: "handoff_1".to_string(),
            output_text: "result".to_string(),
        }
    );

    let output_2 = rx.recv().await.expect("recv");
    assert_eq!(
        output_2,
        HandoffOutput::ImmediateAppend {
            handoff_id: "handoff_1".to_string(),
            output_text: "result 2".to_string(),
        }
    );

    *state.active_handoff.lock().await = None;
    state
        .send_output("ignored after clear".to_string())
        .await
        .expect("send");
    assert!(rx.is_empty());
}
