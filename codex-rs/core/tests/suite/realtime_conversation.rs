use anyhow::Result;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ConversationAudioParams;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::ConversationTextParams;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RealtimeAudioFrame;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeEvent;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_start_audio_text_close_round_trip() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![
        vec![],
        vec![
            vec![json!({
                "type": "session.created",
                "session": { "id": "sess_1" }
            })],
            vec![],
            vec![
                json!({
                    "type": "response.output_audio.delta",
                    "delta": "AQID",
                    "sample_rate": 24000,
                    "num_channels": 1
                }),
                json!({
                    "type": "conversation.item.added",
                    "item": {
                        "type": "message",
                        "role": "assistant",
                        "content": [{"type": "text", "text": "hi"}]
                    }
                }),
            ],
        ],
    ])
    .await;

    let mut builder = test_codex();
    let test = builder.build_with_websocket_server(&server).await?;
    assert!(server.wait_for_handshakes(1, Duration::from_secs(2)).await);

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;

    let started = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationStarted(started) => Some(Ok(started.clone())),
        EventMsg::Error(err) => Some(Err(err.clone())),
        _ => None,
    })
    .await
    .unwrap_or_else(|err: ErrorEvent| panic!("conversation start failed: {err:?}"));
    assert!(started.session_id.is_some());

    let session_created = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionCreated { session_id },
        }) => Some(session_id.clone()),
        _ => None,
    })
    .await;
    assert_eq!(session_created, "sess_1");

    test.codex
        .submit(Op::RealtimeConversationAudio(ConversationAudioParams {
            frame: RealtimeAudioFrame {
                data: "AQID".to_string(),
                sample_rate: 24000,
                num_channels: 1,
                samples_per_channel: Some(480),
            },
        }))
        .await?;
    test.codex
        .submit(Op::RealtimeConversationText(ConversationTextParams {
            text: "hello".to_string(),
        }))
        .await?;

    let audio_out = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::AudioOut(frame),
        }) => Some(frame.clone()),
        _ => None,
    })
    .await;
    assert_eq!(audio_out.data, "AQID");

    let connections = server.connections();
    assert_eq!(connections.len(), 2);
    let connection = &connections[1];
    assert_eq!(connection.len(), 3);
    assert_eq!(
        connection[0].body_json()["type"].as_str(),
        Some("session.create")
    );
    assert_eq!(
        connection[0].body_json()["session"]["conversation_id"]
            .as_str()
            .expect("session.create conversation_id"),
        started
            .session_id
            .as_deref()
            .expect("started session id should be present")
    );
    let request_types = [
        connection[1].body_json()["type"]
            .as_str()
            .expect("request type")
            .to_string(),
        connection[2].body_json()["type"]
            .as_str()
            .expect("request type")
            .to_string(),
    ];
    assert_eq!(
        request_types,
        [
            "conversation.item.create".to_string(),
            "response.input_audio.delta".to_string(),
        ]
    );

    test.codex.submit(Op::RealtimeConversationClose).await?;
    let closed = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationClosed(closed) => Some(closed.clone()),
        _ => None,
    })
    .await;
    assert!(matches!(
        closed.reason.as_deref(),
        Some("requested" | "transport_closed")
    ));

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_transport_close_emits_closed_event() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let session_created = vec![json!({
        "type": "session.created",
        "session": { "id": "sess_1" }
    })];
    let server = start_websocket_server(vec![vec![], vec![session_created]]).await;

    let mut builder = test_codex();
    let test = builder.build_with_websocket_server(&server).await?;
    assert!(server.wait_for_handshakes(1, Duration::from_secs(2)).await);

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;

    let started = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationStarted(started) => Some(Ok(started.clone())),
        EventMsg::Error(err) => Some(Err(err.clone())),
        _ => None,
    })
    .await
    .unwrap_or_else(|err: ErrorEvent| panic!("conversation start failed: {err:?}"));
    assert!(started.session_id.is_some());

    let session_created = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionCreated { session_id },
        }) => Some(session_id.clone()),
        _ => None,
    })
    .await;
    assert_eq!(session_created, "sess_1");

    let closed = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationClosed(closed) => Some(closed.clone()),
        _ => None,
    })
    .await;
    assert_eq!(closed.reason.as_deref(), Some("transport_closed"));

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_audio_before_start_emits_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![]).await;
    let mut builder = test_codex();
    let test = builder.build_with_websocket_server(&server).await?;

    test.codex
        .submit(Op::RealtimeConversationAudio(ConversationAudioParams {
            frame: RealtimeAudioFrame {
                data: "AQID".to_string(),
                sample_rate: 24000,
                num_channels: 1,
                samples_per_channel: Some(480),
            },
        }))
        .await?;

    let err = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::Error(err) => Some(err.clone()),
        _ => None,
    })
    .await;
    assert_eq!(err.codex_error_info, Some(CodexErrorInfo::BadRequest));
    assert_eq!(err.message, "conversation is not running");

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_text_before_start_emits_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![]).await;
    let mut builder = test_codex();
    let test = builder.build_with_websocket_server(&server).await?;

    test.codex
        .submit(Op::RealtimeConversationText(ConversationTextParams {
            text: "hello".to_string(),
        }))
        .await?;

    let err = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::Error(err) => Some(err.clone()),
        _ => None,
    })
    .await;
    assert_eq!(err.codex_error_info, Some(CodexErrorInfo::BadRequest));
    assert_eq!(err.message, "conversation is not running");

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_second_start_replaces_runtime() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![
        vec![],
        vec![vec![json!({
            "type": "session.created",
            "session": { "id": "sess_old" }
        })]],
        vec![
            vec![json!({
                "type": "session.created",
                "session": { "id": "sess_new" }
            })],
            vec![json!({
                "type": "response.output_audio.delta",
                "delta": "AQID",
                "sample_rate": 24000,
                "num_channels": 1
            })],
        ],
    ])
    .await;
    let mut builder = test_codex();
    let test = builder.build_with_websocket_server(&server).await?;
    assert!(server.wait_for_handshakes(1, Duration::from_secs(2)).await);

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "old".to_string(),
            session_id: Some("conv_old".to_string()),
        }))
        .await?;
    wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionCreated { session_id },
        }) if session_id == "sess_old" => Some(Ok(())),
        EventMsg::Error(err) => Some(Err(err.clone())),
        _ => None,
    })
    .await
    .unwrap_or_else(|err: ErrorEvent| panic!("first conversation start failed: {err:?}"));

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "new".to_string(),
            session_id: Some("conv_new".to_string()),
        }))
        .await?;
    wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionCreated { session_id },
        }) if session_id == "sess_new" => Some(Ok(())),
        EventMsg::Error(err) => Some(Err(err.clone())),
        _ => None,
    })
    .await
    .unwrap_or_else(|err: ErrorEvent| panic!("second conversation start failed: {err:?}"));

    test.codex
        .submit(Op::RealtimeConversationAudio(ConversationAudioParams {
            frame: RealtimeAudioFrame {
                data: "AQID".to_string(),
                sample_rate: 24000,
                num_channels: 1,
                samples_per_channel: Some(480),
            },
        }))
        .await?;
    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::AudioOut(frame),
        }) if frame.data == "AQID" => Some(()),
        _ => None,
    })
    .await;

    let connections = server.connections();
    assert_eq!(connections.len(), 3);
    assert_eq!(connections[1].len(), 1);
    assert_eq!(
        connections[1][0].body_json()["session"]["conversation_id"].as_str(),
        Some("conv_old")
    );
    assert_eq!(connections[2].len(), 2);
    assert_eq!(
        connections[2][0].body_json()["session"]["conversation_id"].as_str(),
        Some("conv_new")
    );
    assert_eq!(
        connections[2][1].body_json()["type"].as_str(),
        Some("response.input_audio.delta")
    );

    server.shutdown().await;
    Ok(())
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_uses_experimental_realtime_ws_base_url_override() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let startup_server = start_websocket_server(vec![vec![]]).await;
    let realtime_server = start_websocket_server(vec![vec![vec![json!({
        "type": "session.created",
        "session": { "id": "sess_override" }
    })]]])
    .await;

    let mut builder = test_codex().with_config({
        let realtime_base_url = realtime_server.uri().to_string();
        move |config| {
            config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        }
    });
    let test = builder.build_with_websocket_server(&startup_server).await?;
    assert!(
        startup_server
            .wait_for_handshakes(1, Duration::from_secs(2))
            .await
    );

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;

    let session_created = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionCreated { session_id },
        }) => Some(session_id.clone()),
        _ => None,
    })
    .await;
    assert_eq!(session_created, "sess_override");

    let startup_connections = startup_server.connections();
    assert_eq!(startup_connections.len(), 1);

    let realtime_connections = realtime_server.connections();
    assert_eq!(realtime_connections.len(), 1);
    assert_eq!(
        realtime_connections[0][0].body_json()["type"].as_str(),
        Some("session.create")
    );

    startup_server.shutdown().await;
    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_uses_experimental_realtime_ws_backend_prompt_override() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![
        vec![],
        vec![vec![json!({
            "type": "session.created",
            "session": { "id": "sess_override" }
        })]],
    ])
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.experimental_realtime_ws_backend_prompt = Some("prompt from config".to_string());
    });
    let test = builder.build_with_websocket_server(&server).await?;
    assert!(server.wait_for_handshakes(1, Duration::from_secs(2)).await);

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "prompt from op".to_string(),
            session_id: None,
        }))
        .await?;

    let session_created = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionCreated { session_id },
        }) => Some(session_id.clone()),
        _ => None,
    })
    .await;
    assert_eq!(session_created, "sess_override");

    let connections = server.connections();
    assert_eq!(connections.len(), 2);
    assert_eq!(
        connections[1][0].body_json()["session"]["backend_prompt"].as_str(),
        Some("prompt from config")
    );

    server.shutdown().await;
    Ok(())
}
