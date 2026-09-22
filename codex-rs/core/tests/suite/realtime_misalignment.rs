//! A failed turn must not admit handoffs still arriving from its voice session.

use anyhow::Context;
use anyhow::Result;
use codex_config::config_toml::RealtimeWsVersion;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::RealtimeOutputModality;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use futures::SinkExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use wiremock::ResponseTemplate;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn misalignment_retires_late_voice_handoff_before_it_starts_a_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let api_server = responses::start_mock_server().await;
    let first_response = responses::mount_response_once(
        &api_server,
        ResponseTemplate::new(403).set_body_json(json!({
            "error": {
                "message": "This request violated the misalignment policy.",
                "type": "invalid_request_error",
                "code": "misalignment_policy_violation"
            }
        })),
    )
    .await;

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let realtime_url = format!("ws://{}", listener.local_addr()?);
    let (late_handoff_tx, mut late_handoff_rx) = oneshot::channel();
    let (finish_tx, finish_rx) = oneshot::channel();
    let sideband = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut websocket = accept_async(stream).await?;
        websocket
            .send(Message::Text(
                json!({
                    "type": "session.updated",
                    "session": {"id": "session_misalignment", "instructions": "backend prompt"}
                })
                .to_string()
                .into(),
            ))
            .await?;
        for (handoff_id, text) in [
            ("initial_handoff", "first request"),
            ("late_handoff", "must not start a turn"),
        ] {
            if handoff_id == "late_handoff" {
                (&mut late_handoff_rx).await?;
            }
            websocket
                .send(Message::Text(
                    json!({
                        "type": "conversation.handoff.requested",
                        "handoff_id": handoff_id,
                        "item_id": handoff_id,
                        "input_transcript": text
                    })
                    .to_string()
                    .into(),
                ))
                .await?;
        }
        finish_rx.await?;
        websocket.close(None).await?;
        Ok::<_, anyhow::Error>(())
    });

    let mut builder = test_codex().with_config(move |config| {
        config.experimental_realtime_ws_base_url = Some(realtime_url);
        config.realtime.version = RealtimeWsVersion::V1;
    });
    let test = builder.build_with_auto_env(&api_server).await?;
    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            client_managed_handoffs: false,
            delegation_ack_filler: None,
            flush_transcript_tail_on_session_end: false,
            codex_responses_as_items: false,
            codex_response_item_prefix: None,
            codex_response_handoff_mode:
                codex_protocol::protocol::CodexResponseHandoffMode::Thinking,
            backend_reasoning_status: false,
            codex_response_handoff_channel_prefixes: None,
            model: None,
            output_modality: RealtimeOutputModality::Audio,
            include_startup_context: true,
            initial_items: Vec::new(),
            realtime_start_instructions: None,
            realtime_end_instructions: None,
            prompt: Some(Some("backend prompt".to_string())),
            realtime_session_id: None,
            transport: None,
            version: None,
            voice: None,
        }))
        .await?;

    wait_for_event_match(&test.codex, |event| match event {
        EventMsg::Error(error)
            if error.codex_error_info == Some(CodexErrorInfo::MisalignmentPolicyViolation) =>
        {
            Some(())
        }
        _ => None,
    })
    .await;
    assert_eq!(first_response.requests().len(), 1);
    late_handoff_tx.send(()).expect("sideband still open");
    wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::HandoffRequested(handoff),
        }) if handoff.handoff_id == "late_handoff" => Some(()),
        _ => None,
    })
    .await;
    finish_tx.send(()).expect("sideband still open");
    sideband.await??;

    // The fanout forwards a handoff only after deciding whether to route it.
    // Give any spawned turn a chance to reach the mock API before checking.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let requests = api_server
        .received_requests()
        .await
        .context("API requests")?;
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path() == "/v1/responses")
            .count(),
        1,
    );
    Ok(())
}
