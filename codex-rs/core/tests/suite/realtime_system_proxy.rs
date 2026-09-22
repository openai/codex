//! Verifies that Voice uses the configured system proxy for its realtime sideband.

use anyhow::Result;
use codex_features::Feature;
use codex_http_client::cache_system_proxy_route_for_test;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use codex_protocol::protocol::CodexResponseHandoffMode;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::ConversationStartTransport;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeConversationVersion;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::RealtimeOutputModality;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use futures::SinkExt;
use futures::StreamExt;
use http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::task::AbortOnDropHandle;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const SUBPROCESS_ENV_VAR: &str = "CODEX_REALTIME_SYSTEM_PROXY_TEST_SUBPROCESS";
const TEST_NAME: &str =
    "suite::realtime_system_proxy::webrtc_sideband_honors_configured_system_proxy";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn webrtc_sideband_honors_configured_system_proxy() -> Result<()> {
    skip_if_no_network!(Ok(()));

    if std::env::var_os(SUBPROCESS_ENV_VAR).is_none() {
        let mut command = Command::new(std::env::current_exe()?);
        command.arg("--exact").arg(TEST_NAME);
        for &key in codex_network_proxy::PROXY_ENV_KEYS {
            command.env_remove(key);
        }
        let output = command.env(SUBPROCESS_ENV_VAR, "1").output().await?;
        assert!(
            output.status.success(),
            "subprocess test `{TEST_NAME}` failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return Ok(());
    }

    let server = start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/backend-api/codex/realtime/calls"))
        .respond_with(
            ResponseTemplate::new(StatusCode::OK)
                .insert_header("Location", "/v1/live/rtc_system_proxy")
                .set_body_string("v=answer\r\n"),
        )
        .mount(&server)
        .await;

    let proxy = TcpListener::bind("127.0.0.1:0").await?;
    let proxy_url = format!("http://{}", proxy.local_addr()?);
    let proxy_task = AbortOnDropHandle::new(tokio::spawn(async move {
        let (mut client, _) = proxy.accept().await?;
        let mut request = Vec::new();
        let mut byte = [0_u8; 1];
        while !request.ends_with(b"\r\n\r\n") {
            anyhow::ensure!(request.len() < 64 * 1024, "proxy headers exceeded 64 KiB");
            client.read_exact(&mut byte).await?;
            request.push(byte[0]);
        }
        let request_line = String::from_utf8(request)?
            .lines()
            .next()
            .ok_or_else(|| anyhow::anyhow!("proxy request was empty"))?
            .to_string();
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        let mut websocket = accept_async(client).await?;
        websocket
            .send(Message::Text(
                json!({
                    "type": "session.started",
                    "session": { "id": "rtc_system_proxy" }
                })
                .to_string()
                .into(),
            ))
            .await?;
        // Keep the sideband open until the test closes Voice.
        while let Some(Ok(message)) = websocket.next().await {
            if matches!(message, Message::Close(_)) {
                break;
            }
        }
        Ok::<_, anyhow::Error>(request_line)
    }));

    let backend_base_url = format!("{}/backend-api/codex", server.uri());
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config.model_catalog =
                Some(bundled_models_response().expect("bundled models.json should parse"));
            config.model_provider.base_url = Some(backend_base_url);
            config.experimental_realtime_ws_base_url =
                Some("ws://realtime-system-proxy.invalid:8765".to_string());
            config
                .features
                .enable(Feature::RespectSystemProxy)
                .expect("test config should allow feature update");
            config.respect_system_proxy = true;
        });
    let test = builder.build_with_auto_env(&server).await?;
    cache_system_proxy_route_for_test(
        "http://realtime-system-proxy.invalid:8765/v1/live/rtc_system_proxy",
        proxy_url,
    );

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            client_managed_handoffs: false,
            delegation_ack_filler: None,
            flush_transcript_tail_on_session_end: false,
            codex_responses_as_items: false,
            codex_response_item_prefix: None,
            codex_response_handoff_mode: CodexResponseHandoffMode::Thinking,
            backend_reasoning_status: false,
            codex_response_handoff_channel_prefixes: None,
            model: None,
            output_modality: RealtimeOutputModality::Audio,
            include_startup_context: false,
            initial_items: Vec::new(),
            realtime_start_instructions: None,
            realtime_end_instructions: None,
            prompt: None,
            realtime_session_id: None,
            transport: Some(ConversationStartTransport::Webrtc {
                sdp: "v=offer\r\n".to_string(),
            }),
            version: Some(RealtimeConversationVersion::V3),
            voice: None,
        }))
        .await?;

    let sdp = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RealtimeConversationSdp(event) => Some(Ok(event.sdp.clone())),
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::Error(message),
        }) => Some(Err(anyhow::anyhow!("{message}"))),
        EventMsg::Error(error) => Some(Err(anyhow::anyhow!("{error:?}"))),
        _ => None,
    })
    .await?;
    let session = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RealtimeConversationRealtime(event) => match &event.payload {
            payload @ RealtimeEvent::SessionUpdated { .. } => Some(Ok(payload.clone())),
            RealtimeEvent::Error(message) => Some(Err(anyhow::anyhow!("{message}"))),
            _ => None,
        },
        EventMsg::Error(error) => Some(Err(anyhow::anyhow!("{error:?}"))),
        _ => None,
    })
    .await?;

    test.codex.submit(Op::RealtimeConversationClose).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::RealtimeConversationClosed(_))
    })
    .await;
    test.codex.shutdown_and_wait().await?;
    let proxy_request =
        tokio::time::timeout(Duration::from_secs(/*secs*/ 5), proxy_task).await???;
    assert_eq!(
        (sdp, session, proxy_request),
        (
            "v=answer\r\n".to_string(),
            RealtimeEvent::SessionUpdated {
                realtime_session_id: "rtc_system_proxy".to_string(),
                instructions: None,
            },
            "CONNECT realtime-system-proxy.invalid:8765 HTTP/1.1".to_string(),
        )
    );
    Ok(())
}
