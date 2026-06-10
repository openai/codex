use super::RealtimeAssistantOutput;
use super::RealtimeHandoffState;
use super::realtime_delegation_from_handoff;
use super::realtime_request_headers;
use super::realtime_text_from_handoff_request;
use super::wrap_realtime_delegation_input;
use async_channel::bounded;
use codex_config::config_toml::RealtimeWsVersion;
use codex_protocol::protocol::RealtimeHandoffRequested;
use codex_protocol::protocol::RealtimeTranscriptEntry;
use codex_utils_output_truncation::approx_token_count;
use pretty_assertions::assert_eq;

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
        wrap_realtime_delegation_input("hello", /*transcript_delta*/ None),
        "<realtime_delegation>\n  <input>hello</input>\n</realtime_delegation>"
    );
}

#[test]
fn wraps_realtime_delegation_input_with_xml_escaping() {
    assert_eq!(
        wrap_realtime_delegation_input("use a < b && c > d", Some("saw <that>")),
        "<realtime_delegation>\n  <input>use a &lt; b &amp;&amp; c &gt; d</input>\n  <transcript_delta>saw &lt;that&gt;</transcript_delta>\n</realtime_delegation>"
    );
}

#[test]
fn wraps_realtime_delegation_input_with_xml_escaping_without_transcript() {
    assert_eq!(
        wrap_realtime_delegation_input("use a < b && c > d", /*transcript_delta*/ None),
        "<realtime_delegation>\n  <input>use a &lt; b &amp;&amp; c &gt; d</input>\n</realtime_delegation>"
    );
}

#[tokio::test]
async fn assistant_outputs_preserve_active_handoff_until_turn_completion() {
    let (tx, _rx) = bounded(1);
    let state = RealtimeHandoffState::new(tx);
    *state.active_handoff.lock().await = Some("handoff_1".to_string());

    assert_eq!(
        state.assistant_output("working".to_string()).await,
        RealtimeAssistantOutput {
            handoff_id: Some("handoff_1".to_string()),
            output_text: "working".to_string(),
        }
    );
    assert_eq!(
        state.active_handoff.lock().await.as_deref(),
        Some("handoff_1")
    );
    assert_eq!(
        state.assistant_output("finished".to_string()).await,
        RealtimeAssistantOutput {
            handoff_id: Some("handoff_1".to_string()),
            output_text: "finished".to_string(),
        }
    );
    assert_eq!(
        state.active_handoff.lock().await.as_deref(),
        Some("handoff_1")
    );
    state.finish_turn().await;
    assert_eq!(state.active_handoff.lock().await.as_deref(), None);
}

#[tokio::test]
async fn assistant_output_without_handoff_has_no_active_id() {
    let (tx, _rx) = bounded(1);
    let state = RealtimeHandoffState::new(tx);

    assert_eq!(
        state.assistant_output("working".to_string()).await,
        RealtimeAssistantOutput {
            handoff_id: None,
            output_text: "working".to_string(),
        }
    );
    assert_eq!(
        state.assistant_output("finished".to_string()).await,
        RealtimeAssistantOutput {
            handoff_id: None,
            output_text: "finished".to_string(),
        }
    );
}

#[tokio::test]
async fn assistant_output_is_capped_for_realtime_context() {
    let (tx, _rx) = bounded(1);
    let state = RealtimeHandoffState::new(tx);
    *state.active_handoff.lock().await = Some("handoff_1".to_string());
    let output = state
        .assistant_output(format!("start {} end", "middle ".repeat(2_000)))
        .await;

    assert_eq!(output.handoff_id.as_deref(), Some("handoff_1"));
    assert!(approx_token_count(&output.output_text) <= 1_000);
    assert!(output.output_text.starts_with("start "));
    assert!(output.output_text.ends_with(" end"));
    assert!(output.output_text.contains("tokens truncated"));
}

#[tokio::test]
async fn finishing_turn_consumes_handoff() {
    let (tx, _rx) = bounded(1);
    let state = RealtimeHandoffState::new(tx);
    *state.active_handoff.lock().await = Some("handoff_1".to_string());

    state.finish_turn().await;
    assert_eq!(state.active_handoff.lock().await.as_deref(), None);
}

#[test]
fn uses_quicksilver_alpha_header_for_realtime_v1() {
    let headers =
        realtime_request_headers(Some("session_1"), Some("sk-test"), RealtimeWsVersion::V1)
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
    let headers =
        realtime_request_headers(Some("session_1"), Some("sk-test"), RealtimeWsVersion::V2)
            .expect("headers")
            .expect("headers");

    assert!(headers.get("openai-alpha").is_none());
}
