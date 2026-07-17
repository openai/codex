use super::AGENT_FINAL_MESSAGE_PREFIX;
use super::HANDOFF_STREAM_TRUNCATION_MARKER;
use super::RealtimeHandoffState;
use super::RealtimeSessionKind;
use super::RealtimeStreamedItem;
use super::realtime_delegation_from_handoff;
use super::realtime_request_headers;
use super::realtime_text_from_handoff_request;
use super::wrap_realtime_delegation_input;
use crate::context::RealtimeDelegationSource;
use async_channel::bounded;
use codex_api::RealtimeEventParser;
use codex_protocol::models::MessagePhase;
use codex_protocol::protocol::CodexResponseHandoffMode;
use codex_protocol::protocol::RealtimeHandoffRequested;
use codex_protocol::protocol::RealtimeTranscriptEntry;
use pretty_assertions::assert_eq;
use std::time::Instant;

#[test]
fn prefers_handoff_input_transcript_over_active_transcript() {
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
        Some("ignored".to_string())
    );
}

#[test]
fn extracts_text_from_handoff_request_active_transcript_if_input_missing() {
    let handoff = RealtimeHandoffRequested {
        handoff_id: "handoff_1".to_string(),
        item_id: "item_1".to_string(),
        input_transcript: String::new(),
        active_transcript: vec![RealtimeTranscriptEntry {
            role: "user".to_string(),
            text: "hello".to_string(),
        }],
    };
    assert_eq!(
        realtime_text_from_handoff_request(&handoff),
        Some("user: hello".to_string())
    );
}

#[test]
fn wraps_handoff_with_transcript_delta() {
    let handoff = RealtimeHandoffRequested {
        handoff_id: "handoff_1".to_string(),
        item_id: "item_1".to_string(),
        input_transcript: "delegate this".to_string(),
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
        realtime_delegation_from_handoff(&handoff),
        Some(
            "<realtime_delegation>\n  <input>delegate this</input>\n  <transcript_delta>user: hello\nassistant: hi there</transcript_delta>\n</realtime_delegation>"
                .to_string()
        )
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

#[test]
fn wraps_realtime_delegation_input() {
    assert_eq!(
        wrap_realtime_delegation_input(
            "hello",
            /*transcript_delta*/ None,
            RealtimeDelegationSource::Handoff,
        ),
        "<realtime_delegation>\n  <input>hello</input>\n</realtime_delegation>"
    );
}

#[test]
fn wraps_realtime_delegation_input_with_xml_escaping() {
    assert_eq!(
        wrap_realtime_delegation_input(
            "use a < b && c > d",
            Some("saw <that>"),
            RealtimeDelegationSource::Handoff,
        ),
        "<realtime_delegation>\n  <input>use a &lt; b &amp;&amp; c &gt; d</input>\n  <transcript_delta>saw &lt;that&gt;</transcript_delta>\n</realtime_delegation>"
    );
}

#[test]
fn wraps_realtime_delegation_input_with_xml_escaping_without_transcript() {
    assert_eq!(
        wrap_realtime_delegation_input(
            "use a < b && c > d",
            /*transcript_delta*/ None,
            RealtimeDelegationSource::Handoff,
        ),
        "<realtime_delegation>\n  <input>use a &lt; b &amp;&amp; c &gt; d</input>\n</realtime_delegation>"
    );
}

#[tokio::test]
async fn clears_active_handoff_explicitly() {
    let (tx, _rx) = bounded(1);
    let state = RealtimeHandoffState::new(
        tx,
        /*client_managed_handoffs*/ false,
        /*codex_responses_as_items*/ false,
        /*codex_response_item_prefix*/ None,
        CodexResponseHandoffMode::Thinking,
        RealtimeSessionKind::V1,
        /*event_parser*/ RealtimeEventParser::V1,
    );

    state.stream.lock().await.active_handoff = Some("handoff_1".to_string());
    assert_eq!(
        state.stream.lock().await.active_handoff.clone(),
        Some("handoff_1".to_string())
    );

    state.stream.lock().await.active_handoff = None;
    assert_eq!(state.stream.lock().await.active_handoff.clone(), None);
}

#[test]
fn streamed_handoff_preserves_a_bounded_final_tail() {
    let mut item = RealtimeStreamedItem {
        handoff_id: "handoff_1".to_string(),
        phase: Some(MessagePhase::FinalAnswer),
        bem_channel_parser: None,
        prefix_final_message: true,
        sent_bytes: 0,
        buffered_text: String::new(),
        tail_text: String::new(),
        truncated: false,
        last_flush_at: Instant::now(),
        flush_scheduled: false,
    };
    item.push_text(&format!("HEAD{}TAIL", "x".repeat(/*n*/ 5_000)));

    let first = item
        .drain_stream_chunk()
        .expect("oversized output should retain a streamable head");
    let final_chunk = item
        .drain_final_chunk()
        .expect("oversized output should retain a final tail");
    let output = format!("{first}{final_chunk}");

    assert!(output.len() <= 4_000);
    assert!(output.starts_with(&format!("{AGENT_FINAL_MESSAGE_PREFIX}HEAD")));
    assert!(output.contains(HANDOFF_STREAM_TRUNCATION_MARKER));
    assert!(output.ends_with("TAIL"));
}

#[test]
fn streamed_v3_handoff_omits_the_final_message_prefix() {
    let mut item = RealtimeStreamedItem {
        handoff_id: "handoff_1".to_string(),
        phase: Some(MessagePhase::FinalAnswer),
        bem_channel_parser: None,
        prefix_final_message: false,
        sent_bytes: 0,
        buffered_text: String::new(),
        tail_text: String::new(),
        truncated: false,
        last_flush_at: Instant::now(),
        flush_scheduled: false,
    };
    item.push_text("done");

    assert_eq!(item.drain_final_chunk(), Some("done".to_string()));
}

#[test]
fn uses_quicksilver_alpha_header_for_realtime_v1() {
    let headers = realtime_request_headers(
        Some("session_1"),
        Some("sk-test"),
        RealtimeEventParser::V1,
        "codex_work_desktop",
    )
    .expect("headers")
    .expect("headers");

    assert_eq!(
        headers
            .get("openai-alpha")
            .and_then(|value| value.to_str().ok()),
        Some("quicksilver=v1")
    );
}

#[test]
fn omits_quicksilver_alpha_header_for_realtime_v2() {
    let headers = realtime_request_headers(
        Some("session_1"),
        Some("sk-test"),
        RealtimeEventParser::RealtimeV2,
        "codex_work_desktop",
    )
    .expect("headers")
    .expect("headers");

    assert!(headers.get("openai-alpha").is_none());
}

#[test]
fn uses_frameless_alpha_header_for_realtime_v3() {
    let headers = realtime_request_headers(
        Some("session_1"),
        Some("sk-test"),
        RealtimeEventParser::FramelessBidi,
        "codex_work_desktop",
    )
    .expect("headers")
    .expect("headers");

    assert_eq!(
        headers
            .get("openai-alpha")
            .and_then(|value| value.to_str().ok()),
        Some("quicksilver=v2")
    );
}

#[test]
fn realtime_headers_include_only_non_default_originator() {
    let default_originator = codex_login::default_client::originator();
    for (originator, expected_header) in [
        ("codex_work_desktop", Some("codex_work_desktop")),
        (default_originator.value.as_str(), None),
    ] {
        let headers = realtime_request_headers(
            Some("session_1"),
            Some("sk-test"),
            RealtimeEventParser::RealtimeV2,
            originator,
        )
        .expect("headers")
        .expect("headers");

        assert_eq!(
            headers
                .get("originator")
                .and_then(|value| value.to_str().ok()),
            expected_header
        );
    }
}
