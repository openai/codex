use anyhow::Context;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadRealtimeAppendAudioParams;
use codex_app_server_protocol::ThreadRealtimeAppendAudioResponse;
use codex_app_server_protocol::ThreadRealtimeAppendTextParams;
use codex_app_server_protocol::ThreadRealtimeAppendTextResponse;
use codex_app_server_protocol::ThreadRealtimeAudioChunk;
use codex_app_server_protocol::ThreadRealtimeClosedNotification;
use codex_app_server_protocol::ThreadRealtimeErrorNotification;
use codex_app_server_protocol::ThreadRealtimeItemAddedNotification;
use codex_app_server_protocol::ThreadRealtimeOutputAudioDeltaNotification;
use codex_app_server_protocol::ThreadRealtimeStartParams;
use codex_app_server_protocol::ThreadRealtimeStartResponse;
use codex_app_server_protocol::ThreadRealtimeStartedNotification;
use codex_app_server_protocol::ThreadRealtimeStopParams;
use codex_app_server_protocol::ThreadRealtimeStopResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_core::features::FEATURES;
use codex_core::features::Feature;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn realtime_conversation_streams_v2_notifications() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let realtime_server = start_websocket_server(vec![vec![
        vec![json!({
            "type": "session.created",
            "session": { "id": "sess_backend" }
        })],
        vec![json!({
            "type": "session.updated",
            "session": { "backend_prompt": "backend prompt" }
        })],
        vec![
            json!({
                "type": "response.output_audio.delta",
                "delta": "AQID",
                "sample_rate": 24_000,
                "num_channels": 1,
                "samples_per_channel": 512
            }),
            json!({
                "type": "conversation.item.added",
                "item": {
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "text", "text": "hi" }]
                }
            }),
            json!({
                "type": "error",
                "message": "upstream boom"
            }),
        ],
    ]])
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        realtime_server.uri(),
        true,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    mcp.initialize().await?;

    let thread_start_request_id = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let thread_start_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_request_id)),
    )
    .await??;
    let thread_start: ThreadStartResponse = to_response(thread_start_response)?;

    let start_request_id = mcp
        .send_thread_realtime_start_request(ThreadRealtimeStartParams {
            thread_id: thread_start.thread.id.clone(),
            prompt: "backend prompt".to_string(),
            session_id: None,
        })
        .await?;
    let start_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_request_id)),
    )
    .await??;
    let _: ThreadRealtimeStartResponse = to_response(start_response)?;

    let started =
        read_notification::<ThreadRealtimeStartedNotification>(&mut mcp, "thread/realtime/started")
            .await?;
    assert_eq!(started.thread_id, thread_start.thread.id);
    assert!(started.session_id.is_some());

    let audio_append_request_id = mcp
        .send_thread_realtime_append_audio_request(ThreadRealtimeAppendAudioParams {
            thread_id: started.thread_id.clone(),
            audio: ThreadRealtimeAudioChunk {
                data: "BQYH".to_string(),
                sample_rate: 24_000,
                num_channels: 1,
                samples_per_channel: Some(480),
            },
        })
        .await?;
    let audio_append_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(audio_append_request_id)),
    )
    .await??;
    let _: ThreadRealtimeAppendAudioResponse = to_response(audio_append_response)?;

    let text_append_request_id = mcp
        .send_thread_realtime_append_text_request(ThreadRealtimeAppendTextParams {
            thread_id: started.thread_id.clone(),
            text: "hello".to_string(),
        })
        .await?;
    let text_append_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(text_append_request_id)),
    )
    .await??;
    let _: ThreadRealtimeAppendTextResponse = to_response(text_append_response)?;

    let output_audio = read_notification::<ThreadRealtimeOutputAudioDeltaNotification>(
        &mut mcp,
        "thread/realtime/outputAudio/delta",
    )
    .await?;
    assert_eq!(output_audio.audio.data, "AQID");
    assert_eq!(output_audio.audio.sample_rate, 24_000);
    assert_eq!(output_audio.audio.num_channels, 1);
    assert_eq!(output_audio.audio.samples_per_channel, Some(512));

    let item_added = read_notification::<ThreadRealtimeItemAddedNotification>(
        &mut mcp,
        "thread/realtime/itemAdded",
    )
    .await?;
    assert_eq!(item_added.thread_id, output_audio.thread_id);
    assert_eq!(item_added.item["type"], json!("message"));

    let realtime_error =
        read_notification::<ThreadRealtimeErrorNotification>(&mut mcp, "thread/realtime/error")
            .await?;
    assert_eq!(realtime_error.thread_id, output_audio.thread_id);
    assert_eq!(realtime_error.message, "upstream boom");

    let closed =
        read_notification::<ThreadRealtimeClosedNotification>(&mut mcp, "thread/realtime/closed")
            .await?;
    assert_eq!(closed.thread_id, output_audio.thread_id);
    assert_eq!(closed.reason.as_deref(), Some("transport_closed"));

    let connections = realtime_server.connections();
    assert_eq!(connections.len(), 1);
    let connection = &connections[0];
    assert_eq!(connection.len(), 3);
    assert_eq!(
        connection[0].body_json()["type"].as_str(),
        Some("session.create")
    );
    let mut request_types = [
        connection[1].body_json()["type"]
            .as_str()
            .context("expected websocket request type")?
            .to_string(),
        connection[2].body_json()["type"]
            .as_str()
            .context("expected websocket request type")?
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

    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn realtime_conversation_stop_emits_closed_notification() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let realtime_server = start_websocket_server(vec![vec![
        vec![json!({
            "type": "session.created",
            "session": { "id": "sess_backend" }
        })],
        vec![],
    ]])
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        realtime_server.uri(),
        true,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    mcp.initialize().await?;

    let thread_start_request_id = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let thread_start_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_request_id)),
    )
    .await??;
    let thread_start: ThreadStartResponse = to_response(thread_start_response)?;

    let start_request_id = mcp
        .send_thread_realtime_start_request(ThreadRealtimeStartParams {
            thread_id: thread_start.thread.id.clone(),
            prompt: "backend prompt".to_string(),
            session_id: None,
        })
        .await?;
    let start_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_request_id)),
    )
    .await??;
    let _: ThreadRealtimeStartResponse = to_response(start_response)?;

    let started =
        read_notification::<ThreadRealtimeStartedNotification>(&mut mcp, "thread/realtime/started")
            .await?;

    let stop_request_id = mcp
        .send_thread_realtime_stop_request(ThreadRealtimeStopParams {
            thread_id: started.thread_id.clone(),
        })
        .await?;
    let stop_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(stop_request_id)),
    )
    .await??;
    let _: ThreadRealtimeStopResponse = to_response(stop_response)?;

    let closed =
        read_notification::<ThreadRealtimeClosedNotification>(&mut mcp, "thread/realtime/closed")
            .await?;
    assert_eq!(closed.thread_id, started.thread_id);
    assert!(matches!(
        closed.reason.as_deref(),
        Some("requested" | "transport_closed")
    ));

    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn realtime_conversation_requires_feature_flag() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let realtime_server = start_websocket_server(vec![vec![]]).await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        realtime_server.uri(),
        false,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    mcp.initialize().await?;

    let thread_start_request_id = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let thread_start_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_request_id)),
    )
    .await??;
    let thread_start: ThreadStartResponse = to_response(thread_start_response)?;

    let start_request_id = mcp
        .send_thread_realtime_start_request(ThreadRealtimeStartParams {
            thread_id: thread_start.thread.id.clone(),
            prompt: "backend prompt".to_string(),
            session_id: None,
        })
        .await?;
    let error = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(start_request_id)),
    )
    .await??;
    assert_invalid_request(
        error,
        format!(
            "thread {} does not support realtime conversation",
            thread_start.thread.id
        ),
    );

    realtime_server.shutdown().await;
    Ok(())
}

async fn read_notification<T: DeserializeOwned>(mcp: &mut McpProcess, method: &str) -> Result<T> {
    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message(method),
    )
    .await??;
    let params = notification
        .params
        .context("expected notification params to be present")?;
    Ok(serde_json::from_value(params)?)
}

fn create_config_toml(
    codex_home: &Path,
    responses_server_uri: &str,
    realtime_server_uri: &str,
    realtime_enabled: bool,
) -> std::io::Result<()> {
    let realtime_feature_key = FEATURES
        .iter()
        .find(|spec| spec.id == Feature::RealtimeConversation)
        .map(|spec| spec.key)
        .unwrap_or("realtime_conversation");

    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
model_provider = "mock_provider"
experimental_realtime_ws_base_url = "{realtime_server_uri}"

[features]
{realtime_feature_key} = {realtime_enabled}

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{responses_server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}

fn assert_invalid_request(error: JSONRPCError, message: String) {
    assert_eq!(error.error.code, -32600);
    assert_eq!(error.error.message, message);
    assert_eq!(error.error.data, None);
}
