use anyhow::Result;
use codex_config::config_toml::RealtimeWsVersion;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::RealtimeOutputModality;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::start_websocket_server;
use core_test_support::responses::start_websocket_server_with_headers;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs;

fn realtime_start_params() -> ConversationStartParams {
    ConversationStartParams {
        client_managed_handoffs: false,
        codex_responses_as_items: false,
        codex_response_item_prefix: None,
        codex_response_handoff_prefix: None,
        model: None,
        output_modality: RealtimeOutputModality::Audio,
        include_startup_context: true,
        prompt: Some(Some("backend prompt".to_string())),
        realtime_session_id: None,
        transport: None,
        version: None,
        voice: None,
    }
}

pub(super) fn persisted_realtime_transcript_tails(test: &TestCodex) -> Vec<String> {
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    let Ok(rollout) = fs::read_to_string(rollout_path) else {
        return Vec::new();
    };
    rollout
        .lines()
        .filter_map(|line| serde_json::from_str::<RolloutLine>(line).ok())
        .filter_map(|line| match line.item {
            RolloutItem::ResponseItem(ResponseItem::Message { role, content, .. })
                if role == "developer" =>
            {
                Some(content)
            }
            _ => None,
        })
        .flatten()
        .filter_map(|content| match content {
            ContentItem::InputText { text }
                if text.starts_with("<realtime_delegation>")
                    && text.contains("The user just ended their realtime session") =>
            {
                Some(text)
            }
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_close_without_transcript_does_not_persist_tail() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let api_server = start_mock_server().await;
    let realtime_server = start_websocket_server_with_headers(vec![WebSocketConnectionConfig {
        requests: vec![vec![json!({
            "type": "session.updated",
            "session": { "id": "sess_no_tail", "instructions": "backend prompt" }
        })]],
        response_headers: Vec::new(),
        accept_delay: None,
        close_after_requests: false,
    }])
    .await;
    let realtime_base_url = realtime_server.uri().to_string();
    let mut builder = test_codex().with_config(move |config| {
        config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        config.realtime.version = RealtimeWsVersion::V1;
    });
    let test = builder.build(&api_server).await?;

    test.codex
        .submit(Op::RealtimeConversationStart(realtime_start_params()))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                payload: RealtimeEvent::SessionUpdated { .. }
            })
        )
    })
    .await;

    test.codex.submit(Op::RealtimeConversationClose).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::RealtimeConversationClosed(_))
    })
    .await;

    assert_eq!(
        persisted_realtime_transcript_tails(&test),
        Vec::<String>::new()
    );
    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_conversation_close_persists_trailing_transcript_once() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let api_server = start_mock_server().await;
    let realtime_server = start_websocket_server_with_headers(vec![WebSocketConnectionConfig {
        requests: vec![vec![
            json!({
                "type": "session.updated",
                "session": { "id": "sess_tail", "instructions": "backend prompt" }
            }),
            json!({
                "type": "conversation.input_transcript.delta",
                "delta": "trailing words"
            }),
        ]],
        response_headers: Vec::new(),
        accept_delay: None,
        close_after_requests: false,
    }])
    .await;
    let realtime_base_url = realtime_server.uri().to_string();
    let mut builder = test_codex().with_config(move |config| {
        config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        config.realtime.version = RealtimeWsVersion::V1;
    });
    let test = builder.build(&api_server).await?;

    test.codex
        .submit(Op::RealtimeConversationStart(realtime_start_params()))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                payload: RealtimeEvent::InputTranscriptDelta(_)
            })
        )
    })
    .await;

    for _ in 0..2 {
        test.codex.submit(Op::RealtimeConversationClose).await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::RealtimeConversationClosed(_))
        })
        .await;
    }

    assert_eq!(
        persisted_realtime_transcript_tails(&test),
        vec![
            "<realtime_delegation>\n  <input>The user just ended their realtime session. Here is the remaining handoff/transcript tail. You probably do not have to do anything; acknowledge the handoff unless the transcript itself asks for something.</input>\n  <transcript_delta>user: trailing words</transcript_delta>\n</realtime_delegation>"
                .to_string()
        ]
    );
    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_transport_close_persists_trailing_transcript() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let api_server = start_mock_server().await;
    let realtime_server = start_websocket_server(vec![vec![vec![
        json!({
            "type": "session.updated",
            "session": { "id": "sess_transport_tail", "instructions": "backend prompt" }
        }),
        json!({
            "type": "conversation.input_transcript.delta",
            "delta": "transport tail"
        }),
    ]]])
    .await;
    let realtime_base_url = realtime_server.uri().to_string();
    let mut builder = test_codex().with_config(move |config| {
        config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        config.realtime.version = RealtimeWsVersion::V1;
    });
    let test = builder.build(&api_server).await?;

    test.codex
        .submit(Op::RealtimeConversationStart(realtime_start_params()))
        .await?;
    let closed = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RealtimeConversationClosed(closed) => Some(closed.clone()),
        _ => None,
    })
    .await;

    assert_eq!(closed.reason.as_deref(), Some("transport_closed"));
    assert_eq!(persisted_realtime_transcript_tails(&test).len(), 1);
    realtime_server.shutdown().await;
    Ok(())
}
