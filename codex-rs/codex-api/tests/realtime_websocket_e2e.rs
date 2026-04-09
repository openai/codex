use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use codex_api::Provider;
use codex_api::RealtimeAudioFrame;
use codex_api::RealtimeEvent;
use codex_api::RealtimeEventParser;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeSessionMode;
use codex_api::RealtimeWebsocketClient;
use codex_api::RetryConfig;
use codex_protocol::protocol::RealtimeHandoffRequested;
use codex_protocol::protocol::RealtimeVoice;
use futures::SinkExt;
use futures::StreamExt;
use http::HeaderMap;
use serde_json::Value;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

type RealtimeWsStream = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

async fn spawn_realtime_ws_server<Handler, Fut>(
    handler: Handler,
) -> (String, tokio::task::JoinHandle<()>)
where
    Handler: FnOnce(RealtimeWsStream) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(err) => panic!("failed to bind test websocket listener: {err}"),
    };
    let addr = match listener.local_addr() {
        Ok(addr) => addr.to_string(),
        Err(err) => panic!("failed to read local websocket listener address: {err}"),
    };

    let server = tokio::spawn(async move {
        let (stream, _) = match listener.accept().await {
            Ok(stream) => stream,
            Err(err) => panic!("failed to accept test websocket connection: {err}"),
        };
        let ws = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(err) => panic!("failed to complete websocket handshake: {err}"),
        };
        handler(ws).await;
    });

    (addr, server)
}

fn test_provider(base_url: String) -> Provider {
    Provider {
        name: "test".to_string(),
        base_url,
        query_params: Some(HashMap::new()),
        headers: HeaderMap::new(),
        retry: RetryConfig {
            max_attempts: 1,
            base_delay: Duration::from_millis(1),
            retry_429: false,
            retry_5xx: false,
            retry_transport: false,
        },
        stream_idle_timeout: Duration::from_secs(5),
    }
}

#[tokio::test]
async fn realtime_ws_e2e_session_create_and_event_flow() {
    let (addr, server) = spawn_realtime_ws_server(|mut ws: RealtimeWsStream| async move {
        let first = ws
            .next()
            .await
            .expect("first msg")
            .expect("first msg ok")
            .into_text()
            .expect("text");
        let first_json: Value = serde_json::from_str(&first).expect("json");
        assert_eq!(first_json["type"], "session.update");
        assert_eq!(
            first_json["session"]["type"],
            Value::String("quicksilver".to_string())
        );
        assert_eq!(
            first_json["session"]["instructions"],
            Value::String("backend prompt".to_string())
        );
        assert_eq!(
            first_json["session"]["audio"]["input"]["format"]["type"],
            Value::String("audio/pcm".to_string())
        );
        assert_eq!(
            first_json["session"]["audio"]["input"]["format"]["rate"],
            Value::from(24_000)
        );

        ws.send(Message::Text(
            json!({
                "type": "session.updated",
                "session": {"id": "sess_mock", "instructions": "backend prompt"}
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("send session.updated");

        let second = ws
            .next()
            .await
            .expect("second msg")
            .expect("second msg ok")
            .into_text()
            .expect("text");
        let second_json: Value = serde_json::from_str(&second).expect("json");
        assert_eq!(second_json["type"], "input_audio_buffer.append");

        ws.send(Message::Text(
            json!({
                "type": "conversation.output_audio.delta",
                "delta": "AQID",
                "sample_rate": 48000,
                "channels": 1
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("send audio out");
    })
    .await;

    let client = RealtimeWebsocketClient::new(test_provider(format!("http://{addr}")));
    let connection = client
        .connect(
            RealtimeSessionConfig {
                instructions: "backend prompt".to_string(),
                model: Some("realtime-test-model".to_string()),
                session_id: Some("conv_123".to_string()),
                event_parser: RealtimeEventParser::V1,
                session_mode: RealtimeSessionMode::Conversational,
                voice: RealtimeVoice::Cove,
            },
            HeaderMap::new(),
            HeaderMap::new(),
        )
        .await
        .expect("connect");

    let created = connection
        .next_event()
        .await
        .expect("next event")
        .expect("event");
    assert_eq!(
        created,
        RealtimeEvent::SessionUpdated {
            session_id: "sess_mock".to_string(),
            instructions: Some("backend prompt".to_string()),
        }
    );

    connection
        .send_audio_frame(RealtimeAudioFrame {
            data: "AQID".to_string(),
            sample_rate: 48000,
            num_channels: 1,
            samples_per_channel: Some(960),
            item_id: None,
        })
        .await
        .expect("send audio");

    let audio_event = connection
        .next_event()
        .await
        .expect("next event")
        .expect("event");
    assert_eq!(
        audio_event,
        RealtimeEvent::AudioOut(RealtimeAudioFrame {
            data: "AQID".to_string(),
            sample_rate: 48000,
            num_channels: 1,
            samples_per_channel: None,
            item_id: None,
        })
    );

    connection.close().await.expect("close");
    server.await.expect("server task");
}

#[tokio::test]
async fn realtime_ws_connect_webrtc_sideband_retries_join_until_server_is_available() {
    let reserving_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = reserving_listener.local_addr().expect("local addr");
    drop(reserving_listener);

    let server = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let listener = TcpListener::bind(addr).await.expect("bind delayed server");
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = accept_async(stream).await.expect("accept ws");

        let first = ws
            .next()
            .await
            .expect("first msg")
            .expect("first msg ok")
            .into_text()
            .expect("text");
        let first_json: Value = serde_json::from_str(&first).expect("json");
        assert_eq!(first_json["type"], "session.update");
        assert_eq!(
            first_json["session"]["instructions"],
            Value::String("backend prompt".to_string())
        );

        ws.send(Message::Text(
            json!({
                "type": "session.updated",
                "session": {"id": "sess_joined", "instructions": "backend prompt"}
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("send session.updated");
    });

    let mut provider = test_provider(format!("http://{addr}"));
    provider.retry.max_attempts = 1;
    provider.retry.base_delay = Duration::from_millis(100);

    let client = RealtimeWebsocketClient::new(provider);
    let connection = client
        .connect_webrtc_sideband(
            RealtimeSessionConfig {
                instructions: "backend prompt".to_string(),
                model: Some("realtime-test-model".to_string()),
                session_id: Some("conv_123".to_string()),
                event_parser: RealtimeEventParser::RealtimeV2,
                session_mode: RealtimeSessionMode::Conversational,
                voice: RealtimeVoice::Marin,
            },
            "rtc_test",
            HeaderMap::new(),
            HeaderMap::new(),
        )
        .await
        .expect("connect on retry");

    let event = connection
        .next_event()
        .await
        .expect("next event")
        .expect("event");
    assert_eq!(
        event,
        RealtimeEvent::SessionUpdated {
            session_id: "sess_joined".to_string(),
            instructions: Some("backend prompt".to_string()),
        }
    );

    connection.close().await.expect("close");
    server.await.expect("server task");
}

#[tokio::test]
async fn realtime_ws_e2e_send_while_next_event_waits() {
    let (addr, server) = spawn_realtime_ws_server(|mut ws: RealtimeWsStream| async move {
        let first = ws
            .next()
            .await
            .expect("first msg")
            .expect("first msg ok")
            .into_text()
            .expect("text");
        let first_json: Value = serde_json::from_str(&first).expect("json");
        assert_eq!(first_json["type"], "session.update");

        let second = ws
            .next()
            .await
            .expect("second msg")
            .expect("second msg ok")
            .into_text()
            .expect("text");
        let second_json: Value = serde_json::from_str(&second).expect("json");
        assert_eq!(second_json["type"], "input_audio_buffer.append");

        ws.send(Message::Text(
            json!({
                "type": "session.updated",
                "session": {"id": "sess_after_send", "instructions": "backend prompt"}
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("send session.updated");
    })
    .await;

    let client = RealtimeWebsocketClient::new(test_provider(format!("http://{addr}")));
    let connection = client
        .connect(
            RealtimeSessionConfig {
                instructions: "backend prompt".to_string(),
                model: Some("realtime-test-model".to_string()),
                session_id: Some("conv_123".to_string()),
                event_parser: RealtimeEventParser::V1,
                session_mode: RealtimeSessionMode::Conversational,
                voice: RealtimeVoice::Cove,
            },
            HeaderMap::new(),
            HeaderMap::new(),
        )
        .await
        .expect("connect");

    let (send_result, next_result) = tokio::join!(
        async {
            tokio::time::timeout(
                Duration::from_millis(200),
                connection.send_audio_frame(RealtimeAudioFrame {
                    data: "AQID".to_string(),
                    sample_rate: 48000,
                    num_channels: 1,
                    samples_per_channel: Some(960),
                    item_id: None,
                }),
            )
            .await
        },
        connection.next_event()
    );

    send_result
        .expect("send should not block on next_event")
        .expect("send audio");
    let next_event = next_result.expect("next event").expect("event");
    assert_eq!(
        next_event,
        RealtimeEvent::SessionUpdated {
            session_id: "sess_after_send".to_string(),
            instructions: Some("backend prompt".to_string()),
        }
    );

    connection.close().await.expect("close");
    server.await.expect("server task");
}

#[tokio::test]
async fn realtime_ws_e2e_disconnected_emitted_once() {
    let (addr, server) = spawn_realtime_ws_server(|mut ws: RealtimeWsStream| async move {
        let first = ws
            .next()
            .await
            .expect("first msg")
            .expect("first msg ok")
            .into_text()
            .expect("text");
        let first_json: Value = serde_json::from_str(&first).expect("json");
        assert_eq!(first_json["type"], "session.update");

        ws.send(Message::Close(None)).await.expect("send close");
    })
    .await;

    let client = RealtimeWebsocketClient::new(test_provider(format!("http://{addr}")));
    let connection = client
        .connect(
            RealtimeSessionConfig {
                instructions: "backend prompt".to_string(),
                model: Some("realtime-test-model".to_string()),
                session_id: Some("conv_123".to_string()),
                event_parser: RealtimeEventParser::V1,
                session_mode: RealtimeSessionMode::Conversational,
                voice: RealtimeVoice::Cove,
            },
            HeaderMap::new(),
            HeaderMap::new(),
        )
        .await
        .expect("connect");

    let first = connection.next_event().await.expect("next event");
    assert_eq!(first, None);

    let second = connection.next_event().await.expect("next event");
    assert_eq!(second, None);

    server.await.expect("server task");
}

#[tokio::test]
async fn realtime_ws_e2e_ignores_unknown_text_events() {
    let (addr, server) = spawn_realtime_ws_server(|mut ws: RealtimeWsStream| async move {
        let first = ws
            .next()
            .await
            .expect("first msg")
            .expect("first msg ok")
            .into_text()
            .expect("text");
        let first_json: Value = serde_json::from_str(&first).expect("json");
        assert_eq!(first_json["type"], "session.update");

        ws.send(Message::Text(
            json!({
                "type": "response.created",
                "response": {"id": "resp_unknown"}
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("send unknown event");

        ws.send(Message::Text(
            json!({
                "type": "session.updated",
                "session": {"id": "sess_after_unknown", "instructions": "backend prompt"}
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("send session.updated");
    })
    .await;

    let client = RealtimeWebsocketClient::new(test_provider(format!("http://{addr}")));
    let connection = client
        .connect(
            RealtimeSessionConfig {
                instructions: "backend prompt".to_string(),
                model: Some("realtime-test-model".to_string()),
                session_id: Some("conv_123".to_string()),
                event_parser: RealtimeEventParser::V1,
                session_mode: RealtimeSessionMode::Conversational,
                voice: RealtimeVoice::Cove,
            },
            HeaderMap::new(),
            HeaderMap::new(),
        )
        .await
        .expect("connect");

    let event = connection
        .next_event()
        .await
        .expect("next event")
        .expect("event");
    assert_eq!(
        event,
        RealtimeEvent::SessionUpdated {
            session_id: "sess_after_unknown".to_string(),
            instructions: Some("backend prompt".to_string()),
        }
    );

    connection.close().await.expect("close");
    server.await.expect("server task");
}

#[tokio::test]
async fn realtime_ws_e2e_realtime_v2_parser_emits_handoff_requested() {
    let (addr, server) = spawn_realtime_ws_server(|mut ws: RealtimeWsStream| async move {
        let first = ws
            .next()
            .await
            .expect("first msg")
            .expect("first msg ok")
            .into_text()
            .expect("text");
        let first_json: Value = serde_json::from_str(&first).expect("json");
        assert_eq!(first_json["type"], "session.update");

        ws.send(Message::Text(
            json!({
                "type": "conversation.item.done",
                "item": {
                    "id": "item_123",
                    "type": "function_call",
                    "name": "codex",
                    "call_id": "call_123",
                    "arguments": "{\"prompt\":\"delegate now\"}"
                }
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("send function call");
    })
    .await;

    let client = RealtimeWebsocketClient::new(test_provider(format!("http://{addr}")));
    let connection = client
        .connect(
            RealtimeSessionConfig {
                instructions: "backend prompt".to_string(),
                model: Some("realtime-test-model".to_string()),
                session_id: Some("conv_123".to_string()),
                event_parser: RealtimeEventParser::RealtimeV2,
                session_mode: RealtimeSessionMode::Conversational,
                voice: RealtimeVoice::Marin,
            },
            HeaderMap::new(),
            HeaderMap::new(),
        )
        .await
        .expect("connect");

    let event = connection
        .next_event()
        .await
        .expect("next event")
        .expect("event");
    assert_eq!(
        event,
        RealtimeEvent::HandoffRequested(RealtimeHandoffRequested {
            handoff_id: "call_123".to_string(),
            item_id: "item_123".to_string(),
            input_transcript: "delegate now".to_string(),
            active_transcript: Vec::new(),
        })
    );

    connection.close().await.expect("close");
    server.await.expect("server task");
}
