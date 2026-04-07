use anyhow::Context;
use anyhow::Result;
use chrono::Utc;
use codex_login::CodexAuth;
use codex_login::OPENAI_API_KEY_ENV_VAR;
use codex_protocol::ThreadId;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ConversationAudioParams;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::ConversationTextParams;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RealtimeAudioFrame;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeConversationVersion;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::process::Command;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;

const STARTUP_CONTEXT_HEADER: &str = "Startup context from Codex.";
const MEMORY_PROMPT_PHRASE: &str =
    "You have access to a memory folder with guidance from prior runs.";
const REALTIME_CONVERSATION_TEST_SUBPROCESS_ENV_VAR: &str =
    "CODEX_REALTIME_CONVERSATION_TEST_SUBPROCESS";
fn websocket_request_text(
    request: &core_test_support::responses::WebSocketRequest,
) -> Option<String> {
    request.body_json()["item"]["content"][0]["text"]
        .as_str()
        .map(str::to_owned)
}

fn websocket_request_instructions(
    request: &core_test_support::responses::WebSocketRequest,
) -> Option<String> {
    request.body_json()["session"]["instructions"]
        .as_str()
        .map(str::to_owned)
}

async fn wait_for_matching_websocket_request<F>(
    server: &core_test_support::responses::WebSocketTestServer,
    description: &str,
    predicate: F,
) -> core_test_support::responses::WebSocketRequest
where
    F: Fn(&core_test_support::responses::WebSocketRequest) -> bool,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(request) = server
            .connections()
            .iter()
            .flat_map(|connection| connection.iter())
            .find(|request| predicate(request))
            .cloned()
        {
            return request;
        }

        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {description}"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn run_realtime_conversation_test_in_subprocess(
    test_name: &str,
    openai_api_key: Option<&str>,
) -> Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("--exact")
        .arg(test_name)
        .env(REALTIME_CONVERSATION_TEST_SUBPROCESS_ENV_VAR, "1");
    match openai_api_key {
        Some(openai_api_key) => {
            command.env(OPENAI_API_KEY_ENV_VAR, openai_api_key);
        }
        None => {
            command.env_remove(OPENAI_API_KEY_ENV_VAR);
        }
    }
    let output = command.output()?;
    assert!(
        output.status.success(),
        "subprocess test `{test_name}` failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(())
}
async fn seed_recent_thread(
    test: &TestCodex,
    title: &str,
    first_user_message: &str,
    slug: &str,
) -> Result<()> {
    let db = test.codex.state_db().context("state db enabled")?;
    let thread_id = ThreadId::new();
    let updated_at = Utc::now();
    let mut metadata_builder = codex_state::ThreadMetadataBuilder::new(
        thread_id,
        test.codex_home_path()
            .join(format!("rollout-{thread_id}.jsonl")),
        updated_at,
        SessionSource::Cli,
    );
    metadata_builder.cwd = test.workspace_path(format!("workspace-{slug}"));
    metadata_builder.model_provider = Some("test-provider".to_string());
    metadata_builder.git_branch = Some(format!("branch-{slug}"));
    let mut metadata = metadata_builder.build("test-provider");
    metadata.title = title.to_string();
    metadata.first_user_message = Some(first_user_message.to_string());
    db.upsert_thread(&metadata).await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_start_audio_text_close_round_trip() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![
        vec![],
        vec![
            vec![json!({
                "type": "session.updated",
                "session": { "id": "sess_1", "instructions": "backend prompt" }
            })],
            vec![],
            vec![
                json!({
                    "type": "conversation.output_audio.delta",
                    "delta": "AQID",
                    "sample_rate": 24000,
                    "channels": 1
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
    assert!(
        server
            .wait_for_handshakes(/*expected*/ 1, Duration::from_secs(2))
            .await
    );

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
    assert_eq!(started.version, RealtimeConversationVersion::V1);

    let session_updated = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
        }) => Some(session_id.clone()),
        _ => None,
    })
    .await;
    assert_eq!(session_updated, "sess_1");

    test.codex
        .submit(Op::RealtimeConversationAudio(ConversationAudioParams {
            frame: RealtimeAudioFrame {
                data: "AQID".to_string(),
                sample_rate: 24000,
                num_channels: 1,
                samples_per_channel: Some(480),
                item_id: None,
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
        Some("session.update")
    );
    let initial_instructions = websocket_request_instructions(&connection[0])
        .expect("initial session update instructions");
    assert!(initial_instructions.starts_with("backend prompt"));
    assert_eq!(
        server.handshakes()[1]
            .header("x-session-id")
            .expect("session.update x-session-id header"),
        started
            .session_id
            .as_deref()
            .expect("started session id should be present")
    );
    assert_eq!(
        server.handshakes()[1].header("authorization").as_deref(),
        Some("Bearer dummy")
    );
    assert_eq!(
        server.handshakes()[1].uri(),
        "/v1/realtime?intent=quicksilver&model=realtime-test-model"
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
            "input_audio_buffer.append".to_string(),
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
async fn conversation_start_uses_openai_env_key_fallback_with_chatgpt_auth() -> Result<()> {
    if std::env::var_os(REALTIME_CONVERSATION_TEST_SUBPROCESS_ENV_VAR).is_none() {
        return run_realtime_conversation_test_in_subprocess(
            "suite::realtime_conversation::conversation_start_uses_openai_env_key_fallback_with_chatgpt_auth",
            Some("env-realtime-key"),
        );
    }

    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![
        vec![],
        vec![vec![json!({
            "type": "session.updated",
            "session": { "id": "sess_env", "instructions": "backend prompt" }
        })]],
    ])
    .await;

    let mut builder = test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let test = builder.build_with_websocket_server(&server).await?;
    assert!(
        server
            .wait_for_handshakes(/*expected*/ 1, Duration::from_secs(2))
            .await
    );

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

    let session_updated = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
        }) => Some(session_id.clone()),
        _ => None,
    })
    .await;
    assert_eq!(session_updated, "sess_env");

    assert_eq!(
        server.handshakes()[1].header("authorization").as_deref(),
        Some("Bearer env-realtime-key")
    );

    test.codex.submit(Op::RealtimeConversationClose).await?;
    let _closed = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationClosed(closed) => Some(closed.clone()),
        _ => None,
    })
    .await;

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_transport_close_emits_closed_event() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let session_updated = vec![json!({
        "type": "session.updated",
        "session": { "id": "sess_1", "instructions": "backend prompt" }
    })];
    let server = start_websocket_server(vec![vec![], vec![session_updated]]).await;

    let mut builder = test_codex();
    let test = builder.build_with_websocket_server(&server).await?;
    assert!(
        server
            .wait_for_handshakes(/*expected*/ 1, Duration::from_secs(2))
            .await
    );

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

    let session_updated = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
        }) => Some(session_id.clone()),
        _ => None,
    })
    .await;
    assert_eq!(session_updated, "sess_1");

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
                item_id: None,
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
async fn conversation_start_preflight_failure_emits_realtime_error_only() -> Result<()> {
    if std::env::var_os(REALTIME_CONVERSATION_TEST_SUBPROCESS_ENV_VAR).is_none() {
        return run_realtime_conversation_test_in_subprocess(
            "suite::realtime_conversation::conversation_start_preflight_failure_emits_realtime_error_only",
            /*openai_api_key*/ None,
        );
    }

    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![]).await;
    let mut builder = test_codex().with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let test = builder.build_with_websocket_server(&server).await?;

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;

    let err = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::Error(message),
        }) => Some(message.clone()),
        _ => None,
    })
    .await;
    assert_eq!(err, "realtime conversation requires API key auth");

    let closed = timeout(Duration::from_millis(200), async {
        wait_for_event_match(&test.codex, |msg| match msg {
            EventMsg::RealtimeConversationClosed(closed) => Some(closed.clone()),
            _ => None,
        })
        .await
    })
    .await;
    assert!(closed.is_err(), "preflight failure should not emit closed");

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_start_connect_failure_emits_realtime_error_only() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_websocket_server(vec![]).await;
    let mut builder = test_codex().with_config(|config| {
        config.experimental_realtime_ws_base_url = Some("http://127.0.0.1:1".to_string());
    });
    let test = builder.build_with_websocket_server(&server).await?;

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;

    let err = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::Error(message),
        }) => Some(message.clone()),
        _ => None,
    })
    .await;
    assert!(!err.is_empty());

    let closed = timeout(Duration::from_millis(200), async {
        wait_for_event_match(&test.codex, |msg| match msg {
            EventMsg::RealtimeConversationClosed(closed) => Some(closed.clone()),
            _ => None,
        })
        .await
    })
    .await;
    assert!(closed.is_err(), "connect failure should not emit closed");

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
            "type": "session.updated",
            "session": { "id": "sess_old", "instructions": "old" }
        })]],
        vec![
            vec![json!({
                "type": "session.updated",
                "session": { "id": "sess_new", "instructions": "new" }
            })],
            vec![json!({
                "type": "conversation.output_audio.delta",
                "delta": "AQID",
                "sample_rate": 24000,
                "channels": 1
            })],
        ],
    ])
    .await;
    let mut builder = test_codex();
    let test = builder.build_with_websocket_server(&server).await?;
    assert!(
        server
            .wait_for_handshakes(/*expected*/ 1, Duration::from_secs(2))
            .await
    );

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "old".to_string(),
            session_id: Some("conv_old".to_string()),
        }))
        .await?;
    wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
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
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
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
                item_id: None,
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
    let old_instructions =
        websocket_request_instructions(&connections[1][0]).expect("old session instructions");
    assert!(old_instructions.starts_with("old"));
    assert_eq!(
        server.handshakes()[1].header("x-session-id").as_deref(),
        Some("conv_old")
    );
    assert_eq!(connections[2].len(), 2);
    let new_instructions =
        websocket_request_instructions(&connections[2][0]).expect("new session instructions");
    assert!(new_instructions.starts_with("new"));
    assert_eq!(
        server.handshakes()[2].header("x-session-id").as_deref(),
        Some("conv_new")
    );
    assert_eq!(
        connections[2][1].body_json()["type"].as_str(),
        Some("input_audio_buffer.append")
    );

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_uses_experimental_realtime_ws_base_url_override() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let startup_server = start_websocket_server(vec![vec![]]).await;
    let realtime_server = start_websocket_server(vec![vec![vec![json!({
        "type": "session.updated",
        "session": { "id": "sess_override", "instructions": "backend prompt" }
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
            .wait_for_handshakes(/*expected*/ 1, Duration::from_secs(2))
            .await
    );

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;

    let session_updated = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
        }) => Some(session_id.clone()),
        _ => None,
    })
    .await;
    assert_eq!(session_updated, "sess_override");

    let startup_connections = startup_server.connections();
    assert_eq!(startup_connections.len(), 1);

    let realtime_connections = realtime_server.connections();
    assert_eq!(realtime_connections.len(), 1);
    assert_eq!(
        realtime_connections[0][0].body_json()["type"].as_str(),
        Some("session.update")
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
            "type": "session.updated",
            "session": { "id": "sess_override", "instructions": "prompt from config" }
        })]],
    ])
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.experimental_realtime_ws_backend_prompt = Some("prompt from config".to_string());
    });
    let test = builder.build_with_websocket_server(&server).await?;
    assert!(
        server
            .wait_for_handshakes(/*expected*/ 1, Duration::from_secs(2))
            .await
    );

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "prompt from op".to_string(),
            session_id: None,
        }))
        .await?;

    let session_updated = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
        }) => Some(session_id.clone()),
        _ => None,
    })
    .await;
    assert_eq!(session_updated, "sess_override");

    let connections = server.connections();
    assert_eq!(connections.len(), 2);
    let overridden_instructions = websocket_request_instructions(&connections[1][0])
        .expect("overridden session instructions");
    assert!(overridden_instructions.starts_with("prompt from config"));

    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_uses_experimental_realtime_ws_startup_context_override() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let startup_server = start_websocket_server(vec![vec![]]).await;
    let realtime_server = start_websocket_server(vec![vec![vec![json!({
        "type": "session.updated",
        "session": { "id": "sess_custom_context", "instructions": "prompt from config" }
    })]]])
    .await;

    let mut builder = test_codex().with_config({
        let realtime_base_url = realtime_server.uri().to_string();
        move |config| {
            config.experimental_realtime_ws_base_url = Some(realtime_base_url);
            config.experimental_realtime_ws_backend_prompt = Some("prompt from config".to_string());
            config.experimental_realtime_ws_startup_context =
                Some("custom startup context".to_string());
        }
    });
    let test = builder.build_with_websocket_server(&startup_server).await?;
    seed_recent_thread(
        &test,
        "Recent work: cleaned up startup flows and reviewed websocket routing.",
        "Investigate realtime startup context",
        "custom-context",
    )
    .await?;
    fs::create_dir_all(test.workspace_path("docs"))?;
    fs::write(test.workspace_path("README.md"), "workspace marker")?;
    assert!(
        startup_server
            .wait_for_handshakes(/*expected*/ 1, Duration::from_secs(2))
            .await
    );

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "prompt from op".to_string(),
            session_id: None,
        }))
        .await?;

    let startup_context_request = wait_for_matching_websocket_request(
        &realtime_server,
        "startup context request with instructions",
        |request| websocket_request_instructions(request).is_some(),
    )
    .await;
    let instructions = websocket_request_instructions(&startup_context_request)
        .expect("custom startup context request should contain instructions");

    assert_eq!(instructions, "prompt from config\n\ncustom startup context");
    assert!(!instructions.contains(STARTUP_CONTEXT_HEADER));
    assert!(!instructions.contains("## Machine / Workspace Map"));

    startup_server.shutdown().await;
    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_disables_realtime_startup_context_with_empty_override() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let startup_server = start_websocket_server(vec![vec![]]).await;
    let realtime_server = start_websocket_server(vec![vec![vec![json!({
        "type": "session.updated",
        "session": { "id": "sess_no_context", "instructions": "prompt from config" }
    })]]])
    .await;

    let mut builder = test_codex().with_config({
        let realtime_base_url = realtime_server.uri().to_string();
        move |config| {
            config.experimental_realtime_ws_base_url = Some(realtime_base_url);
            config.experimental_realtime_ws_backend_prompt = Some("prompt from config".to_string());
            config.experimental_realtime_ws_startup_context = Some(String::new());
        }
    });
    let test = builder.build_with_websocket_server(&startup_server).await?;
    seed_recent_thread(
        &test,
        "Recent work: cleaned up startup flows and reviewed websocket routing.",
        "Investigate realtime startup context",
        "no-context",
    )
    .await?;
    fs::create_dir_all(test.workspace_path("docs"))?;
    fs::write(test.workspace_path("README.md"), "workspace marker")?;
    assert!(
        startup_server
            .wait_for_handshakes(/*expected*/ 1, Duration::from_secs(2))
            .await
    );

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "prompt from op".to_string(),
            session_id: None,
        }))
        .await?;

    let startup_context_request = wait_for_matching_websocket_request(
        &realtime_server,
        "startup context disable request with instructions",
        |request| websocket_request_instructions(request).is_some(),
    )
    .await;
    let instructions = websocket_request_instructions(&startup_context_request)
        .expect("startup context disable request should contain instructions");

    assert_eq!(instructions, "prompt from config");
    assert!(!instructions.contains(STARTUP_CONTEXT_HEADER));
    assert!(!instructions.contains("## Machine / Workspace Map"));

    startup_server.shutdown().await;
    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_start_injects_startup_context_from_thread_history() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let startup_server = start_websocket_server(vec![vec![]]).await;
    let realtime_server = start_websocket_server(vec![vec![vec![json!({
        "type": "session.updated",
        "session": { "id": "sess_context", "instructions": "backend prompt" }
    })]]])
    .await;

    let mut builder = test_codex().with_config({
        let realtime_base_url = realtime_server.uri().to_string();
        move |config| {
            config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        }
    });
    let test = builder.build_with_websocket_server(&startup_server).await?;
    seed_recent_thread(
        &test,
        "Recent work: cleaned up startup flows and reviewed websocket routing.",
        "Investigate realtime startup context",
        "latest",
    )
    .await?;
    fs::create_dir_all(test.workspace_path("docs"))?;
    fs::write(test.workspace_path("README.md"), "workspace marker")?;

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;

    let startup_context_request = wait_for_matching_websocket_request(
        &realtime_server,
        "startup context request with instructions",
        |request| websocket_request_instructions(request).is_some(),
    )
    .await;
    let startup_context = websocket_request_instructions(&startup_context_request)
        .expect("startup context request should contain instructions");

    assert!(startup_context.contains(STARTUP_CONTEXT_HEADER));
    assert!(!startup_context.contains("## User"));
    assert!(startup_context.contains("### "));
    assert!(startup_context.contains("Recent sessions: 1"));
    assert!(startup_context.contains("Latest branch: branch-latest"));
    assert!(startup_context.contains("User asks:"));
    assert!(startup_context.contains("Investigate realtime startup context"));
    assert!(startup_context.contains("## Machine / Workspace Map"));
    assert!(startup_context.contains("README.md"));
    assert!(!startup_context.contains(MEMORY_PROMPT_PHRASE));

    startup_server.shutdown().await;
    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_startup_context_falls_back_to_workspace_map() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let startup_server = start_websocket_server(vec![vec![]]).await;
    let realtime_server = start_websocket_server(vec![vec![vec![json!({
        "type": "session.updated",
        "session": { "id": "sess_workspace", "instructions": "backend prompt" }
    })]]])
    .await;

    let mut builder = test_codex().with_config({
        let realtime_base_url = realtime_server.uri().to_string();
        move |config| {
            config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        }
    });
    let test = builder.build_with_websocket_server(&startup_server).await?;
    fs::create_dir_all(test.workspace_path("codex-rs/core"))?;
    fs::write(test.workspace_path("notes.txt"), "workspace marker")?;

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;

    let startup_context_request = wait_for_matching_websocket_request(
        &realtime_server,
        "workspace-map startup context request with instructions",
        |request| websocket_request_instructions(request).is_some(),
    )
    .await;
    let startup_context = websocket_request_instructions(&startup_context_request)
        .expect("startup context request should contain instructions");

    assert!(startup_context.contains(STARTUP_CONTEXT_HEADER));
    assert!(startup_context.contains("## Machine / Workspace Map"));
    assert!(startup_context.contains("notes.txt"));
    assert!(startup_context.contains("codex-rs/"));

    startup_server.shutdown().await;
    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_startup_context_is_truncated_and_sent_once_per_start() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let startup_server = start_websocket_server(vec![vec![]]).await;
    let realtime_server = start_websocket_server(vec![vec![
        vec![json!({
            "type": "session.updated",
            "session": { "id": "sess_truncated", "instructions": "backend prompt" }
        })],
        vec![],
    ]])
    .await;

    let oversized_summary = "recent work ".repeat(3_500);
    let mut builder = test_codex().with_config({
        let realtime_base_url = realtime_server.uri().to_string();
        move |config| {
            config.experimental_realtime_ws_base_url = Some(realtime_base_url);
        }
    });
    let test = builder.build_with_websocket_server(&startup_server).await?;
    seed_recent_thread(&test, &oversized_summary, "summary", "oversized").await?;
    fs::write(test.workspace_path("marker.txt"), "marker")?;

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;

    let startup_context_request = wait_for_matching_websocket_request(
        &realtime_server,
        "truncated startup context request with instructions",
        |request| websocket_request_instructions(request).is_some(),
    )
    .await;
    let startup_context = websocket_request_instructions(&startup_context_request)
        .expect("startup context request should contain instructions");
    assert!(startup_context.contains(STARTUP_CONTEXT_HEADER));
    assert!(startup_context.len() <= 20_500);

    test.codex
        .submit(Op::RealtimeConversationText(ConversationTextParams {
            text: "hello".to_string(),
        }))
        .await?;

    let explicit_text_request = wait_for_matching_websocket_request(
        &realtime_server,
        "explicit realtime text request",
        |request| websocket_request_text(request).as_deref() == Some("hello"),
    )
    .await;
    assert_eq!(
        websocket_request_text(&explicit_text_request),
        Some("hello".to_string())
    );

    startup_server.shutdown().await;
    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_mirrors_assistant_message_text_to_realtime_handoff() -> Result<()> {
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
        vec![
            json!({
                "type": "session.updated",
                "session": { "id": "sess_1", "instructions": "backend prompt" }
            }),
            json!({
                "type": "conversation.input_transcript.delta",
                "delta": "delegate hello"
            }),
            json!({
                "type": "conversation.handoff.requested",
                "handoff_id": "handoff_1",
                "item_id": "item_1",
                "input_transcript": "delegate hello"
            }),
        ],
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

    let session_updated = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
        }) => Some(session_id.clone()),
        _ => None,
    })
    .await;
    assert_eq!(session_updated, "sess_1");

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::HandoffRequested(handoff),
        }) if handoff.handoff_id == "handoff_1" => Some(()),
        _ => None,
    })
    .await;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

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
        Some("session.update")
    );
    assert_eq!(
        realtime_connections[0][1].body_json()["type"].as_str(),
        Some("conversation.handoff.append")
    );
    assert_eq!(
        realtime_connections[0][1].body_json()["handoff_id"].as_str(),
        Some("handoff_1")
    );
    assert_eq!(
        realtime_connections[0][1].body_json()["output_text"].as_str(),
        Some("\"Agent Final Message\":\n\nassistant says hi")
    );

    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conversation_handoff_persists_across_item_done_until_turn_complete() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let (gate_second_message_tx, gate_second_message_rx) = oneshot::channel();
    let first_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(responses::ev_response_created("resp-1")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(responses::ev_assistant_message(
                "msg-1",
                "assistant message 1",
            )),
        },
        StreamingSseChunk {
            gate: Some(gate_second_message_rx),
            body: sse_event(responses::ev_assistant_message(
                "msg-2",
                "assistant message 2",
            )),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(responses::ev_completed("resp-1")),
        },
    ];
    let (api_server, completions) = start_streaming_sse_server(vec![first_chunks]).await;

    let realtime_server = start_websocket_server(vec![vec![
        vec![
            json!({
                "type": "session.updated",
                "session": { "id": "sess_item_done", "instructions": "backend prompt" }
            }),
            json!({
                "type": "conversation.input_transcript.delta",
                "delta": "delegate now"
            }),
            json!({
                "type": "conversation.handoff.requested",
                "handoff_id": "handoff_item_done",
                "item_id": "item_item_done",
                "input_transcript": "delegate now"
            }),
        ],
        vec![json!({
            "type": "conversation.item.done",
            "item": { "id": "item_item_done" }
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
    let test = builder.build_with_streaming_server(&api_server).await?;

    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            prompt: "backend prompt".to_string(),
            session_id: None,
        }))
        .await?;

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
        }) if session_id == "sess_item_done" => Some(()),
        _ => None,
    })
    .await;

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::HandoffRequested(handoff),
        }) if handoff.handoff_id == "handoff_item_done" => Some(()),
        _ => None,
    })
    .await;

    let first_append = realtime_server
        .wait_for_request(/*connection_index*/ 0, /*request_index*/ 1)
        .await;
    assert_eq!(
        first_append.body_json()["type"].as_str(),
        Some("conversation.handoff.append")
    );
    assert_eq!(
        first_append.body_json()["handoff_id"].as_str(),
        Some("handoff_item_done")
    );
    assert_eq!(
        first_append.body_json()["output_text"].as_str(),
        Some("\"Agent Final Message\":\n\nassistant message 1")
    );

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::ConversationItemDone { item_id },
        }) if item_id == "item_item_done" => Some(()),
        _ => None,
    })
    .await;

    let _ = gate_second_message_tx.send(());

    let second_append = realtime_server
        .wait_for_request(/*connection_index*/ 0, /*request_index*/ 2)
        .await;
    assert_eq!(
        second_append.body_json()["type"].as_str(),
        Some("conversation.handoff.append")
    );
    assert_eq!(
        second_append.body_json()["handoff_id"].as_str(),
        Some("handoff_item_done")
    );
    assert_eq!(
        second_append.body_json()["output_text"].as_str(),
        Some("\"Agent Final Message\":\n\nassistant message 2")
    );

    let completion = completions
        .into_iter()
        .next()
        .expect("missing delegated turn completion");
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
async fn inbound_handoff_request_starts_turn() -> Result<()> {
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
            "type": "session.updated",
            "session": { "id": "sess_inbound", "instructions": "backend prompt" }
        }),
        json!({
            "type": "conversation.input_transcript.delta",
            "delta": "text from realtime"
        }),
        json!({
            "type": "conversation.handoff.requested",
            "handoff_id": "handoff_inbound",
            "item_id": "item_inbound",
            "input_transcript": "text from realtime"
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

    let session_updated = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
        }) => Some(session_id.clone()),
        _ => None,
    })
    .await;
    assert_eq!(session_updated, "sess_inbound");

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::HandoffRequested(handoff),
        }) if handoff.handoff_id == "handoff_inbound"
            && handoff.input_transcript == "text from realtime" =>
        {
            Some(())
        }
        _ => None,
    })
    .await;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = response_mock.single_request();
    let user_texts = request.message_input_texts("user");
    assert!(
        user_texts
            .iter()
            .any(|text| text == "user: text from realtime")
    );

    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbound_handoff_request_uses_active_transcript() -> Result<()> {
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
            "type": "session.updated",
            "session": { "id": "sess_inbound_multi", "instructions": "backend prompt" }
        }),
        json!({
            "type": "conversation.output_transcript.delta",
            "delta": "assistant context"
        }),
        json!({
            "type": "conversation.input_transcript.delta",
            "delta": "delegated query"
        }),
        json!({
            "type": "conversation.output_transcript.delta",
            "delta": "assist confirm"
        }),
        json!({
            "type": "conversation.handoff.requested",
            "handoff_id": "handoff_inbound_multi",
            "item_id": "item_inbound_multi",
            "input_transcript": "ignored"
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
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
        }) => Some(session_id.clone()),
        _ => None,
    })
    .await;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = response_mock.single_request();
    let user_texts = request.message_input_texts("user");
    assert!(user_texts.iter().any(|text| text
        == "assistant: assistant context\nuser: delegated query\nassistant: assist confirm"));

    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbound_handoff_request_clears_active_transcript_after_each_handoff() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let api_server = start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &api_server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_assistant_message("msg-1", "first ok"),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_assistant_message("msg-2", "second ok"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let realtime_server = start_websocket_server(vec![vec![
        vec![
            json!({
                "type": "session.updated",
                "session": { "id": "sess_inbound_clear", "instructions": "backend prompt" }
            }),
            json!({
                "type": "conversation.input_transcript.delta",
                "delta": "first question"
            }),
            json!({
                "type": "conversation.handoff.requested",
                "handoff_id": "handoff_inbound_clear_1",
                "item_id": "item_inbound_clear_1",
                "input_transcript": "first question"
            }),
        ],
        vec![],
        vec![
            json!({
                "type": "conversation.input_transcript.delta",
                "delta": "second question"
            }),
            json!({
                "type": "conversation.handoff.requested",
                "handoff_id": "handoff_inbound_clear_2",
                "item_id": "item_inbound_clear_2",
                "input_transcript": "second question"
            }),
        ],
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

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
        }) => Some(session_id.clone()),
        _ => None,
    })
    .await;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    test.codex
        .submit(Op::RealtimeConversationAudio(ConversationAudioParams {
            frame: RealtimeAudioFrame {
                data: "AQID".to_string(),
                sample_rate: 24000,
                num_channels: 1,
                samples_per_channel: Some(480),
                item_id: None,
            },
        }))
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);

    let first_user_texts = requests[0].message_input_texts("user");
    assert!(
        first_user_texts
            .iter()
            .any(|text| text == "user: first question")
    );

    let second_user_texts = requests[1].message_input_texts("user");
    assert!(
        second_user_texts
            .iter()
            .any(|text| text == "user: second question")
    );
    assert!(
        !second_user_texts
            .iter()
            .any(|text| text == "user: first question\nuser: second question")
    );

    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbound_conversation_item_does_not_start_turn_and_still_forwards_audio() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let api_server = start_mock_server().await;

    let realtime_server = start_websocket_server(vec![vec![vec![
        json!({
            "type": "session.updated",
            "session": { "id": "sess_ignore_item", "instructions": "backend prompt" }
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
            "type": "conversation.output_audio.delta",
            "delta": "AQID",
            "sample_rate": 24000,
            "channels": 1
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
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
        }) if session_id == "sess_ignore_item" => Some(()),
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
    .expect("timed out waiting for realtime audio after conversation item");
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
    let start = std::time::Instant::now();

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
                "type": "session.updated",
                "session": { "id": "sess_echo_guard", "instructions": "backend prompt" }
            }),
            json!({
                "type": "conversation.input_transcript.delta",
                "delta": "delegate now"
            }),
            json!({
                "type": "conversation.handoff.requested",
                "handoff_id": "handoff_echo_guard",
                "item_id": "item_echo_guard",
                "input_transcript": "delegate now"
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
                "type": "conversation.output_audio.delta",
                "delta": "AQID",
                "sample_rate": 24000,
                "channels": 1
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
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
        }) if session_id == "sess_echo_guard" => Some(()),
        _ => None,
    })
    .await;

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::HandoffRequested(handoff),
        }) if handoff.input_transcript == "delegate now" => Some(()),
        _ => None,
    })
    .await;
    eprintln!(
        "[realtime test +{}ms] saw trigger text={:?}",
        start.elapsed().as_millis(),
        "delegate now"
    );

    let mirrored_request = realtime_server
        .wait_for_request(/*connection_index*/ 0, /*request_index*/ 1)
        .await;
    let mirrored_request_body = mirrored_request.body_json();
    eprintln!(
        "[realtime test +{}ms] saw mirrored request type={:?} handoff_id={:?} text={:?}",
        start.elapsed().as_millis(),
        mirrored_request_body["type"].as_str(),
        mirrored_request_body["handoff_id"].as_str(),
        mirrored_request_body["output_text"].as_str(),
    );
    assert_eq!(
        mirrored_request_body["type"].as_str(),
        Some("conversation.handoff.append")
    );
    assert_eq!(
        mirrored_request_body["handoff_id"].as_str(),
        Some("handoff_echo_guard")
    );
    assert_eq!(
        mirrored_request_body["output_text"].as_str(),
        Some("\"Agent Final Message\":\n\nassistant says hi")
    );

    let audio_out = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::AudioOut(frame),
        }) => Some(frame.clone()),
        _ => None,
    })
    .await;
    eprintln!(
        "[realtime test +{}ms] saw audio out data={} sample_rate={} num_channels={}",
        start.elapsed().as_millis(),
        audio_out.data,
        audio_out.sample_rate,
        audio_out.num_channels
    );
    assert_eq!(audio_out.data, "AQID");

    let completion = completions
        .into_iter()
        .next()
        .expect("missing delegated turn completion");
    let _ = gate_completed_tx.send(());
    completion
        .await
        .expect("delegated turn request did not complete");
    eprintln!(
        "[realtime test +{}ms] delegated completion resolved",
        start.elapsed().as_millis()
    );
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
async fn inbound_handoff_request_does_not_block_realtime_event_forwarding() -> Result<()> {
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
            "type": "session.updated",
            "session": { "id": "sess_non_blocking", "instructions": "backend prompt" }
        }),
        json!({
            "type": "conversation.input_transcript.delta",
            "delta": "delegate now"
        }),
        json!({
            "type": "conversation.handoff.requested",
            "handoff_id": "handoff_non_blocking",
            "item_id": "item_non_blocking",
            "input_transcript": "delegate now"
        }),
        json!({
            "type": "conversation.output_audio.delta",
            "delta": "AQID",
            "sample_rate": 24000,
            "channels": 1
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
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
        }) if session_id == "sess_non_blocking" => Some(()),
        _ => None,
    })
    .await;

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::HandoffRequested(handoff),
        }) if handoff.input_transcript == "delegate now" => Some(()),
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
async fn inbound_handoff_request_steers_active_turn() -> Result<()> {
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
            "type": "session.updated",
            "session": { "id": "sess_steer", "instructions": "backend prompt" }
        })],
        vec![
            json!({
                "type": "conversation.input_transcript.delta",
                "delta": "steer via realtime"
            }),
            json!({
                "type": "conversation.handoff.requested",
                "handoff_id": "handoff_steer",
                "item_id": "item_steer",
                "input_transcript": "steer via realtime"
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
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
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
            submission_type: None,
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
                item_id: None,
            },
        }))
        .await?;

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::HandoffRequested(handoff),
        }) if handoff.input_transcript == "steer via realtime" => Some(()),
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
    assert!(
        !first_texts
            .iter()
            .any(|text| text == "user: steer via realtime")
    );
    assert!(second_texts.iter().any(|text| text == "first prompt"));
    assert!(
        second_texts
            .iter()
            .any(|text| text == "user: steer via realtime")
    );

    realtime_server.shutdown().await;
    api_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbound_handoff_request_starts_turn_and_does_not_block_realtime_audio() -> Result<()> {
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

    let delegated_text = "delegate from handoff request";
    let realtime_server = start_websocket_server(vec![vec![vec![
        json!({
            "type": "session.updated",
            "session": { "id": "sess_handoff_request", "instructions": "backend prompt" }
        }),
        json!({
            "type": "conversation.input_transcript.delta",
            "delta": delegated_text
        }),
        json!({
            "type": "conversation.handoff.requested",
            "handoff_id": "handoff_audio",
            "item_id": "item_audio",
            "input_transcript": delegated_text
        }),
        json!({
            "type": "conversation.output_audio.delta",
            "delta": "AQID",
            "sample_rate": 24000,
            "channels": 1
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
            payload: RealtimeEvent::SessionUpdated { session_id, .. },
        }) if session_id == "sess_handoff_request" => Some(()),
        _ => None,
    })
    .await;

    let _ = wait_for_event_match(&test.codex, |msg| match msg {
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::HandoffRequested(handoff),
        }) => (handoff.handoff_id == "handoff_audio" && handoff.input_transcript == delegated_text)
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
    .expect("timed out waiting for realtime audio after handoff request");
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
    let expected_text = format!("user: {delegated_text}");
    assert!(first_texts.iter().any(|text| text == &expected_text));

    realtime_server.shutdown().await;
    api_server.shutdown().await;
    Ok(())
}
