use std::collections::HashMap;
use std::time::Duration;

use codex_api::RealtimeAudioFrame;
use codex_api::RealtimeEvent;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeWebsocketClient;
use codex_api::provider::Provider;
use codex_api::provider::RetryConfig;
use futures::SinkExt;
use futures::StreamExt;
use http::HeaderMap;
use serde_json::Value;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn realtime_ws_e2e_session_create_and_event_flow() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");

    let server = tokio::spawn(async move {
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
        assert_eq!(first_json["type"], "session.create");
        assert_eq!(
            first_json["session"]["backend_prompt"],
            Value::String("backend prompt".to_string())
        );
        assert_eq!(
            first_json["session"]["conversation_id"],
            Value::String("conv_123".to_string())
        );

        ws.send(Message::Text(
            json!({
                "type": "session.created",
                "session": {"id": "sess_mock"}
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("send session.created");

        let second = ws
            .next()
            .await
            .expect("second msg")
            .expect("second msg ok")
            .into_text()
            .expect("text");
        let second_json: Value = serde_json::from_str(&second).expect("json");
        assert_eq!(second_json["type"], "response.input_audio.delta");

        ws.send(Message::Text(
            json!({
                "type": "response.output_audio.delta",
                "delta": "AQID",
                "sample_rate": 48000,
                "num_channels": 1
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("send audio out");
    });

    let provider = Provider {
        name: "test".to_string(),
        base_url: "http://localhost".to_string(),
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
    };
    let client = RealtimeWebsocketClient::new(provider);
    let connection = client
        .connect(
            RealtimeSessionConfig {
                api_url: format!("ws://{addr}"),
                prompt: "backend prompt".to_string(),
                session_id: Some("conv_123".to_string()),
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
        RealtimeEvent::SessionCreated {
            session_id: "sess_mock".to_string()
        }
    );

    connection
        .send_audio_frame(RealtimeAudioFrame {
            data: "AQID".to_string(),
            sample_rate: 48000,
            num_channels: 1,
            samples_per_channel: Some(960),
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
        })
    );

    connection.close().await.expect("close");
    server.await.expect("server task");
}

#[tokio::test]
async fn realtime_ws_e2e_send_while_next_event_waits() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");

    let server = tokio::spawn(async move {
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
        assert_eq!(first_json["type"], "session.create");

        let second = ws
            .next()
            .await
            .expect("second msg")
            .expect("second msg ok")
            .into_text()
            .expect("text");
        let second_json: Value = serde_json::from_str(&second).expect("json");
        assert_eq!(second_json["type"], "response.input_audio.delta");

        ws.send(Message::Text(
            json!({
                "type": "session.created",
                "session": {"id": "sess_after_send"}
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("send session.created");
    });

    let provider = Provider {
        name: "test".to_string(),
        base_url: "http://localhost".to_string(),
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
    };
    let client = RealtimeWebsocketClient::new(provider);
    let connection = client
        .connect(
            RealtimeSessionConfig {
                api_url: format!("ws://{addr}"),
                prompt: "backend prompt".to_string(),
                session_id: Some("conv_123".to_string()),
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
        RealtimeEvent::SessionCreated {
            session_id: "sess_after_send".to_string()
        }
    );

    connection.close().await.expect("close");
    server.await.expect("server task");
}

#[tokio::test]
async fn realtime_ws_e2e_disconnected_emitted_once() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");

    let server = tokio::spawn(async move {
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
        assert_eq!(first_json["type"], "session.create");

        ws.send(Message::Close(None)).await.expect("send close");
    });

    let provider = Provider {
        name: "test".to_string(),
        base_url: "http://localhost".to_string(),
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
    };
    let client = RealtimeWebsocketClient::new(provider);
    let connection = client
        .connect(
            RealtimeSessionConfig {
                api_url: format!("ws://{addr}"),
                prompt: "backend prompt".to_string(),
                session_id: Some("conv_123".to_string()),
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
