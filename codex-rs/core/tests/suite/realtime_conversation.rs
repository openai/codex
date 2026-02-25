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
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use tokio::sync::oneshot;

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
    let mut request_types = [
        connection[1].body_json()["type"]
            .as_str()
            .expect("request type")
            .to_string(),
        connection[2].body_json()["type"]
            .as_str()
            .expect("request type")
            .to_string(),
    ];
    request_types.sort();
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_mirrors_assistant_message_text_to_realtime_websocket() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let api_server = start_mock_server().await;
    let _response_mock = responses::mount_sse_once(
        &api_server,
        responses::sse(vec![
            responses::ev_response_created("resp_1"),
            responses::ev_assistant_message("msg_1", "assistant says hi"),
            responses::ev_completed("resp_1"),
        ]),
    )
    .await;

    let realtime_server = start_websocket_server(vec![vec![
        vec![json!({
            "type": "session.created",
            "session": { "id": "sess_1" }
        })],
        vec![],
    ]])
    .await;

    let mut builder = test_codex().with_config({
        let realtime_base_url = realtime_server.uri().to_string();
        move |config| {
            config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        }
    });
    let test = builder.build(&api_server).await?;

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
    assert_eq!(session_created, "sess_1");

    test.submit_turn("hello").await?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        let connections = realtime_server.connections();
        if connections.len() == 1 && connections[0].len() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let realtime_connections = realtime_server.connections();
    assert_eq!(realtime_connections.len(), 1);
    assert_eq!(realtime_connections[0].len(), 2);
    assert_eq!(
        realtime_connections[0][0].body_json()["type"].as_str(),
        Some("session.create")
    );
    assert_eq!(
        realtime_connections[0][1].body_json()["type"].as_str(),
        Some("conversation.item.create")
    );
    assert_eq!(
        realtime_connections[0][1].body_json()["item"]["content"][0]["text"].as_str(),
        Some("assistant says hi")
    );

    realtime_server.shutdown().await;
    Ok(())
}

fn sse_event(event: Value) -> String {
    responses::sse(vec![event])
}

fn message_input_texts(body: &Value, role: &str) -> Vec<String> {
    body.get("input")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter(|item| item.get("role").and_then(Value::as_str) == Some(role))
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten()
        .filter(|span| span.get("type").and_then(Value::as_str) == Some("input_text"))
        .filter_map(|span| span.get("text").and_then(Value::as_str).map(str::to_owned))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbound_realtime_text_starts_turn_for_assistant_role() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let api_server = start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &api_server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "ok"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let realtime_server = start_websocket_server(vec![vec![vec![
        json!({
            "type": "session.created",
            "session": { "id": "sess_inbound" }
        }),
        json!({
            "type": "conversation.item.added",
            "item": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "text", "text": "text from realtime"}]
            }
        }),
    ]]])
    .await;

    let mut builder = test_codex().with_config({
        let realtime_base_url = realtime_server.uri().to_string();
        move |config| {
            config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        }
    });
    let test = builder.build(&api_server).await?;

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
    assert_eq!(session_created, "sess_inbound");

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = response_mock.single_request();
    let user_texts = request.message_input_texts("user");
    assert!(user_texts.iter().any(|text| text == "text from realtime"));

    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbound_realtime_text_ignores_user_role_and_still_forwards_audio() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let api_server = start_mock_server().await;

    let realtime_server = start_websocket_server(vec![vec![vec![
        json!({
            "type": "session.created",
            "session": { "id": "sess_ignore_user_role" }
        }),
        json!({
            "type": "conversation.item.added",
            "item": {
                "type": "message",
                "role": "user",
                "content": [{"type": "text", "text": "echoed local text"}]
            }
        }),
        json!({
            "type": "response.output_audio.delta",
            "delta": "AQID",
            "sample_rate": 24000,
            "num_channels": 1
        }),
    ]]])
    .await;

    let mut builder = test_codex().with_config({
        let realtime_base_url = realtime_server.uri().to_string();
        move |config| {
            config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        }
    });
    let test = builder.build(&api_server).await?;

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionCreated { session_id },
        }) if session_id == "sess_ignore_user_role" => Some(()),
        _ => None,
    })
    .await;

    let audio_out = tokio::time::timeout(
        Duration::from_millis(500),
        wait_for_event_match(&test.codex, |msg| match msg {
            EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                payload: RealtimeEvent::AudioOut(frame),
            }) => Some(frame.clone()),
            _ => None,
        }),
    )
    .await
    .expect("timed out waiting for realtime audio after user-role conversation item");
    assert_eq!(audio_out.data, "AQID");

    let unexpected_turn_started = tokio::time::timeout(
        Duration::from_millis(200),
        wait_for_event_match(&test.codex, |msg| match msg {
            EventMsg::TurnStarted(_) => Some(()),
            _ => None,
        }),
    )
    .await;
    assert!(unexpected_turn_started.is_err());

    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delegated_turn_user_role_echo_does_not_redelegate_and_still_forwards_audio() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let (gate_completed_tx, gate_completed_rx) = oneshot::channel();
    let first_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(responses::ev_response_created("resp-1")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(responses::ev_assistant_message(
                "msg-1",
                "assistant says hi",
            )),
        },
        StreamingSseChunk {
            gate: Some(gate_completed_rx),
            body: sse_event(responses::ev_completed("resp-1")),
        },
    ];
    let (api_server, completions) = start_streaming_sse_server(vec![first_chunks]).await;

    let realtime_server = start_websocket_server(vec![vec![
        vec![
            json!({
                "type": "session.created",
                "session": { "id": "sess_echo_guard" }
            }),
            json!({
                "type": "conversation.item.added",
                "item": {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "text", "text": "delegate now"}]
                }
            }),
        ],
        vec![
            json!({
                "type": "conversation.item.added",
                "item": {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "text", "text": "assistant says hi"}]
                }
            }),
            json!({
                "type": "response.output_audio.delta",
                "delta": "AQID",
                "sample_rate": 24000,
                "num_channels": 1
            }),
        ],
    ]])
    .await;

    let mut builder = test_codex().with_model("gpt-5.1").with_config({
        let realtime_base_url = realtime_server.uri().to_string();
        move |config| {
            config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        }
    });
    let test = builder.build_with_streaming_server(&api_server).await?;

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionCreated { session_id },
        }) if session_id == "sess_echo_guard" => Some(()),
        _ => None,
    })
    .await;

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::ConversationItemAdded(item),
        }) => item
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|content| content.get("text").and_then(Value::as_str) == Some("delegate now"))
            .then_some(()),
        _ => None,
    })
    .await;

    let audio_out = tokio::time::timeout(
        Duration::from_millis(500),
        wait_for_event_match(&test.codex, |msg| match msg {
            EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                payload: RealtimeEvent::AudioOut(frame),
            }) => Some(frame.clone()),
            _ => None,
        }),
    )
    .await
    .expect("timed out waiting for realtime audio after echoed user-role message");
    assert_eq!(audio_out.data, "AQID");

    let completion = completions
        .into_iter()
        .next()
        .expect("missing delegated turn completion");
    let _ = gate_completed_tx.send(());
    completion
        .await
        .expect("delegated turn request did not complete");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = api_server.requests().await;
    assert_eq!(requests.len(), 1);

    realtime_server.shutdown().await;
    api_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbound_realtime_text_does_not_block_realtime_event_forwarding() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (gate_completed_tx, gate_completed_rx) = oneshot::channel();
    let first_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(responses::ev_response_created("resp-1")),
        },
        StreamingSseChunk {
            gate: Some(gate_completed_rx),
            body: sse_event(responses::ev_completed("resp-1")),
        },
    ];
    let (api_server, completions) = start_streaming_sse_server(vec![first_chunks]).await;

    let realtime_server = start_websocket_server(vec![vec![vec![
        json!({
            "type": "session.created",
            "session": { "id": "sess_non_blocking" }
        }),
        json!({
            "type": "conversation.item.added",
            "item": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "text", "text": "delegate now"}]
            }
        }),
        json!({
            "type": "response.output_audio.delta",
            "delta": "AQID",
            "sample_rate": 24000,
            "num_channels": 1
        }),
    ]]])
    .await;

    let mut builder = test_codex().with_model("gpt-5.1").with_config({
        let realtime_base_url = realtime_server.uri().to_string();
        move |config| {
            config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        }
    });
    let test = builder.build_with_streaming_server(&api_server).await?;

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionCreated { session_id },
        }) if session_id == "sess_non_blocking" => Some(()),
        _ => None,
    })
    .await;

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::ConversationItemAdded(item),
        }) => item
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|content| content.get("text").and_then(Value::as_str) == Some("delegate now"))
            .then_some(()),
        _ => None,
    })
    .await;

    let audio_out = tokio::time::timeout(
        Duration::from_millis(500),
        wait_for_event_match(&test.codex, |msg| match msg {
            EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                payload: RealtimeEvent::AudioOut(frame),
            }) => Some(frame.clone()),
            _ => None,
        }),
    )
    .await
    .expect("timed out waiting for realtime audio while delegated turn was still pending");
    assert_eq!(audio_out.data, "AQID");

    let completion = completions
        .into_iter()
        .next()
        .expect("missing delegated turn completion");
    let _ = gate_completed_tx.send(());
    completion
        .await
        .expect("delegated turn request did not complete");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    realtime_server.shutdown().await;
    api_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbound_realtime_text_steers_active_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (gate_completed_tx, gate_completed_rx) = oneshot::channel();
    let first_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(responses::ev_response_created("resp-1")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(responses::ev_message_item_added("msg-1", "")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(responses::ev_output_text_delta("first ")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(responses::ev_output_text_delta("turn")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(responses::ev_assistant_message("msg-1", "first turn")),
        },
        StreamingSseChunk {
            gate: Some(gate_completed_rx),
            body: sse_event(responses::ev_completed("resp-1")),
        },
    ];
    let second_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(responses::ev_response_created("resp-2")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(responses::ev_completed("resp-2")),
        },
    ];
    let (api_server, completions) =
        start_streaming_sse_server(vec![first_chunks, second_chunks]).await;

    let realtime_server = start_websocket_server(vec![vec![
        vec![json!({
            "type": "session.created",
            "session": { "id": "sess_steer" }
        })],
        vec![],
        vec![json!({
            "type": "conversation.item.added",
            "item": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "text", "text": "steer via realtime"}]
            }
        })],
    ]])
    .await;

    let mut builder = test_codex().with_model("gpt-5.1").with_config({
        let realtime_base_url = realtime_server.uri().to_string();
        move |config| {
            config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        }
    });
    let test = builder.build_with_streaming_server(&api_server).await?;

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;
    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionCreated { session_id },
        }) if session_id == "sess_steer" => Some(()),
        _ => None,
    })
    .await;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "first prompt".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::AgentMessageContentDelta(_))
    })
    .await;

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
            payload: RealtimeEvent::ConversationItemAdded(item),
        }) => item
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|content| {
                content.get("text").and_then(Value::as_str) == Some("steer via realtime")
            })
            .then_some(()),
        _ => None,
    })
    .await;

    let mut completion_iter = completions.into_iter();
    let first_completion = completion_iter.next().expect("missing first completion");
    let second_completion = completion_iter.next().expect("missing second completion");

    let _ = gate_completed_tx.send(());
    first_completion
        .await
        .expect("first request did not complete");
    second_completion
        .await
        .expect("second request did not complete");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = api_server.requests().await;
    assert_eq!(requests.len(), 2);

    let first_body: Value = serde_json::from_slice(&requests[0]).expect("parse first request");
    let second_body: Value = serde_json::from_slice(&requests[1]).expect("parse second request");
    let first_texts = message_input_texts(&first_body, "user");
    let second_texts = message_input_texts(&second_body, "user");

    assert!(first_texts.iter().any(|text| text == "first prompt"));
    assert!(!first_texts.iter().any(|text| text == "steer via realtime"));
    assert!(second_texts.iter().any(|text| text == "first prompt"));
    assert!(second_texts.iter().any(|text| text == "steer via realtime"));

    realtime_server.shutdown().await;
    api_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbound_spawn_transcript_starts_turn_and_does_not_block_realtime_audio() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (gate_completed_tx, gate_completed_rx) = oneshot::channel();
    let first_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(responses::ev_response_created("resp-1")),
        },
        StreamingSseChunk {
            gate: Some(gate_completed_rx),
            body: sse_event(responses::ev_completed("resp-1")),
        },
    ];
    let (api_server, completions) = start_streaming_sse_server(vec![first_chunks]).await;

    let delegated_text = "delegate from spawn transcript";
    let realtime_server = start_websocket_server(vec![vec![vec![
        json!({
            "type": "session.created",
            "session": { "id": "sess_spawn_transcript" }
        }),
        json!({
            "type": "conversation.item.added",
            "item": {
                "type": "spawn_transcript",
                "seq": 1,
                "full_user_transcript": delegated_text,
                "delta_user_transcript": delegated_text,
                "backend_prompt_messages": [{
                    "role": "user",
                    "channel": null,
                    "content": delegated_text,
                    "content_type": "text"
                }],
                "transcript_source": "backend_prompt_messages"
            }
        }),
        json!({
            "type": "response.output_audio.delta",
            "delta": "AQID",
            "sample_rate": 24000,
            "num_channels": 1
        }),
    ]]])
    .await;

    let mut builder = test_codex().with_model("gpt-5.1").with_config({
        let realtime_base_url = realtime_server.uri().to_string();
        move |config| {
            config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        }
    });
    let test = builder.build_with_streaming_server(&api_server).await?;

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionCreated { session_id },
        }) if session_id == "sess_spawn_transcript" => Some(()),
        _ => None,
    })
    .await;

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::ConversationItemAdded(item),
        }) => (item.get("type").and_then(Value::as_str) == Some("spawn_transcript")
            && item.get("delta_user_transcript").and_then(Value::as_str) == Some(delegated_text))
        .then_some(()),
        _ => None,
    })
    .await;

    let audio_out = tokio::time::timeout(
        Duration::from_millis(500),
        wait_for_event_match(&test.codex, |msg| match msg {
            EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                payload: RealtimeEvent::AudioOut(frame),
            }) => Some(frame.clone()),
            _ => None,
        }),
    )
    .await
    .expect("timed out waiting for realtime audio after spawn_transcript");
    assert_eq!(audio_out.data, "AQID");

    let completion = completions
        .into_iter()
        .next()
        .expect("missing delegated turn completion");
    let _ = gate_completed_tx.send(());
    completion
        .await
        .expect("delegated turn request did not complete");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = api_server.requests().await;
    assert_eq!(requests.len(), 1);
    let first_body: Value = serde_json::from_slice(&requests[0]).expect("parse first request");
    let first_texts = message_input_texts(&first_body, "user");
    assert!(first_texts.iter().any(|text| text == delegated_text));

    realtime_server.shutdown().await;
    api_server.shutdown().await;
    Ok(())
}
