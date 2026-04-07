use anyhow::Context;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginAccountResponse;
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
use codex_app_server_protocol::ThreadRealtimeSdpNotification;
use codex_app_server_protocol::ThreadRealtimeStartParams;
use codex_app_server_protocol::ThreadRealtimeStartResponse;
use codex_app_server_protocol::ThreadRealtimeStartTransport;
use codex_app_server_protocol::ThreadRealtimeStartedNotification;
use codex_app_server_protocol::ThreadRealtimeStopParams;
use codex_app_server_protocol::ThreadRealtimeStopResponse;
use codex_app_server_protocol::ThreadRealtimeTranscriptUpdatedNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_features::FEATURES;
use codex_features::Feature;
use codex_protocol::protocol::RealtimeConversationVersion;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Match;
use wiremock::Mock;
use wiremock::Request as WiremockRequest;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const STARTUP_CONTEXT_HEADER: &str = "Startup context from Codex.";

#[derive(Debug, Clone)]
struct RealtimeCallRequestCapture {
    requests: Arc<Mutex<Vec<WiremockRequest>>>,
}

impl RealtimeCallRequestCapture {
    fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn single_request(&self) -> WiremockRequest {
        let requests = self
            .requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(requests.len(), 1, "expected one realtime call request");
        requests[0].clone()
    }
}

impl Match for RealtimeCallRequestCapture {
    fn matches(&self, request: &WiremockRequest) -> bool {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request.clone());
        true
    }
}

#[tokio::test]
async fn realtime_conversation_streams_v2_notifications() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let realtime_server = start_websocket_server(vec![vec![
        vec![json!({
            "type": "session.updated",
            "session": { "id": "sess_backend", "instructions": "backend prompt" }
        })],
        vec![],
        vec![
            json!({
                "type": "response.output_audio.delta",
                "delta": "AQID",
                "sample_rate": 24_000,
                "channels": 1,
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
                "type": "conversation.item.input_audio_transcription.delta",
                "delta": "delegate now"
            }),
            json!({
                "type": "response.output_text.delta",
                "delta": "working"
            }),
            json!({
                "type": "conversation.item.done",
                "item": {
                    "id": "item_2",
                    "type": "function_call",
                    "name": "codex",
                    "call_id": "handoff_1",
                    "arguments": "{\"input_transcript\":\"delegate now\"}"
                }
            }),
            json!({
                "type": "error",
                "message": "upstream boom"
            }),
        ],
        vec![],
    ]])
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        realtime_server.uri(),
        /*realtime_enabled*/ true,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    mcp.initialize().await?;
    login_with_api_key(&mut mcp, "sk-test-key").await?;

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
            transport: None,
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
    assert_eq!(started.version, RealtimeConversationVersion::V2);

    let startup_context_request = realtime_server
        .wait_for_request(/*connection_index*/ 0, /*request_index*/ 0)
        .await;
    assert_eq!(
        startup_context_request.body_json()["type"].as_str(),
        Some("session.update")
    );
    assert!(
        startup_context_request.body_json()["session"]["instructions"]
            .as_str()
            .context("expected startup context instructions")?
            .contains(STARTUP_CONTEXT_HEADER)
    );

    let audio_append_request_id = mcp
        .send_thread_realtime_append_audio_request(ThreadRealtimeAppendAudioParams {
            thread_id: started.thread_id.clone(),
            audio: ThreadRealtimeAudioChunk {
                data: "BQYH".to_string(),
                sample_rate: 24_000,
                num_channels: 1,
                samples_per_channel: Some(480),
                item_id: None,
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

    let first_transcript_update = read_notification::<ThreadRealtimeTranscriptUpdatedNotification>(
        &mut mcp,
        "thread/realtime/transcriptUpdated",
    )
    .await?;
    assert_eq!(first_transcript_update.thread_id, output_audio.thread_id);
    assert_eq!(first_transcript_update.role, "user");
    assert_eq!(first_transcript_update.text, "delegate now");

    let second_transcript_update =
        read_notification::<ThreadRealtimeTranscriptUpdatedNotification>(
            &mut mcp,
            "thread/realtime/transcriptUpdated",
        )
        .await?;
    assert_eq!(second_transcript_update.thread_id, output_audio.thread_id);
    assert_eq!(second_transcript_update.role, "assistant");
    assert_eq!(second_transcript_update.text, "working");

    let handoff_item_added = read_notification::<ThreadRealtimeItemAddedNotification>(
        &mut mcp,
        "thread/realtime/itemAdded",
    )
    .await?;
    assert_eq!(handoff_item_added.thread_id, output_audio.thread_id);
    assert_eq!(handoff_item_added.item["type"], json!("handoff_request"));
    assert_eq!(handoff_item_added.item["handoff_id"], json!("handoff_1"));
    assert_eq!(handoff_item_added.item["item_id"], json!("item_2"));
    assert_eq!(
        handoff_item_added.item["input_transcript"],
        json!("delegate now")
    );
    assert_eq!(handoff_item_added.item["active_transcript"], json!([]));

    let realtime_error =
        read_notification::<ThreadRealtimeErrorNotification>(&mut mcp, "thread/realtime/error")
            .await?;
    assert_eq!(realtime_error.thread_id, output_audio.thread_id);
    assert_eq!(realtime_error.message, "upstream boom");

    let closed =
        read_notification::<ThreadRealtimeClosedNotification>(&mut mcp, "thread/realtime/closed")
            .await?;
    assert_eq!(closed.thread_id, output_audio.thread_id);
    assert_eq!(closed.reason.as_deref(), Some("error"));

    let connections = realtime_server.connections();
    assert_eq!(connections.len(), 1);
    let connection = &connections[0];
    assert_eq!(connection.len(), 4);
    assert_eq!(
        connection[0].body_json()["type"].as_str(),
        Some("session.update")
    );
    assert!(
        connection[0].body_json()["session"]["instructions"]
            .as_str()
            .context("expected startup context instructions")?
            .contains(STARTUP_CONTEXT_HEADER)
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
        connection[3].body_json()["type"]
            .as_str()
            .context("expected websocket request type")?
            .to_string(),
    ];
    request_types.sort();
    assert_eq!(
        request_types,
        [
            "conversation.item.create".to_string(),
            "input_audio_buffer.append".to_string(),
            "response.create".to_string(),
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
            "type": "session.updated",
            "session": { "id": "sess_backend", "instructions": "backend prompt" }
        })],
        vec![],
    ]])
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        realtime_server.uri(),
        /*realtime_enabled*/ true,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    mcp.initialize().await?;
    login_with_api_key(&mut mcp, "sk-test-key").await?;

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
            transport: None,
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
async fn realtime_webrtc_start_emits_sdp_notification() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let call_capture = RealtimeCallRequestCapture::new();
    Mock::given(method("POST"))
        .and(path("/v1/realtime/calls"))
        .and(call_capture.clone())
        .respond_with(ResponseTemplate::new(200).set_body_string("v=answer\r\n"))
        .mount(&responses_server)
        .await;
    let realtime_server = start_websocket_server(vec![vec![]]).await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        realtime_server.uri(),
        /*realtime_enabled*/ true,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    mcp.initialize().await?;
    login_with_api_key(&mut mcp, "sk-test-key").await?;

    let thread_start_request_id = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let thread_start_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_request_id)),
    )
    .await??;
    let thread_start: ThreadStartResponse = to_response(thread_start_response)?;

    let thread_id = thread_start.thread.id;
    let start_request_id = mcp
        .send_thread_realtime_start_request(ThreadRealtimeStartParams {
            thread_id: thread_id.clone(),
            prompt: "backend prompt".to_string(),
            session_id: None,
            transport: Some(ThreadRealtimeStartTransport::Webrtc {
                sdp: "v=offer\r\n".to_string(),
            }),
        })
        .await?;
    let start_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_request_id)),
    )
    .await??;
    let _: ThreadRealtimeStartResponse = to_response(start_response)?;

    let sdp_notification =
        read_notification::<ThreadRealtimeSdpNotification>(&mut mcp, "thread/realtime/sdp").await?;
    assert_eq!(
        sdp_notification,
        ThreadRealtimeSdpNotification {
            thread_id,
            sdp: "v=answer\r\n".to_string()
        }
    );

    let request = call_capture.single_request();
    assert_eq!(request.url.path(), "/v1/realtime/calls");
    assert_eq!(request.url.query(), None);
    let body = String::from_utf8(request.body).context("multipart body should be utf-8")?;
    let session = json!({
        "type": "realtime",
        "instructions": "backend prompt\n\nstartup context",
        "output_modalities": ["audio"],
        "audio": {
            "input": {
                "format": {
                    "type": "audio/pcm",
                    "rate": 24_000,
                },
                "noise_reduction": {
                    "type": "near_field",
                },
                "turn_detection": {
                    "type": "server_vad",
                    "interrupt_response": true,
                    "create_response": true,
                },
            },
            "output": {
                "format": {
                    "type": "audio/pcm",
                    "rate": 24_000,
                },
                "voice": "marin",
            },
        },
        "tools": [{
            "type": "function",
            "name": "codex",
            "description": "Delegate a request to Codex and return the final result to the user. Use this as the default action. If the user asks to do something next, later, after this, or once current work finishes, call this tool so the work is actually queued instead of merely promising to do it later.",
            "parameters": {
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "The user request to delegate to Codex.",
                    },
                },
                "required": ["prompt"],
                "additionalProperties": false,
            },
        }],
        "tool_choice": "auto",
    })
    .to_string();
    assert_eq!(
        body,
        format!(
            "--codex-realtime-call-boundary\r\n\
             Content-Disposition: form-data; name=\"sdp\"\r\n\
             Content-Type: application/sdp\r\n\
             \r\n\
             v=offer\r\n\
             \r\n\
             --codex-realtime-call-boundary\r\n\
             Content-Disposition: form-data; name=\"session\"\r\n\
             Content-Type: application/json\r\n\
             \r\n\
             {session}\r\n\
             --codex-realtime-call-boundary--\r\n"
        )
    );

    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn realtime_webrtc_start_surfaces_backend_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    Mock::given(method("POST"))
        .and(path("/v1/realtime/calls"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&responses_server)
        .await;
    let realtime_server = start_websocket_server(vec![vec![]]).await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        realtime_server.uri(),
        /*realtime_enabled*/ true,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    mcp.initialize().await?;
    login_with_api_key(&mut mcp, "sk-test-key").await?;

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
            thread_id: thread_start.thread.id,
            prompt: "backend prompt".to_string(),
            session_id: None,
            transport: Some(ThreadRealtimeStartTransport::Webrtc {
                sdp: "v=offer\r\n".to_string(),
            }),
        })
        .await?;
    let start_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_request_id)),
    )
    .await??;
    let _: ThreadRealtimeStartResponse = to_response(start_response)?;

    let error =
        read_notification::<ThreadRealtimeErrorNotification>(&mut mcp, "thread/realtime/error")
            .await?;
    assert!(error.message.contains("currently experiencing high demand"));

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
        /*realtime_enabled*/ false,
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
            transport: None,
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

async fn login_with_api_key(mcp: &mut McpProcess, api_key: &str) -> Result<()> {
    let request_id = mcp.send_login_account_api_key_request(api_key).await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let login: LoginAccountResponse = to_response(response)?;
    assert_eq!(login, LoginAccountResponse::ApiKey {});

    Ok(())
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
experimental_realtime_ws_backend_prompt = "backend prompt"
experimental_realtime_ws_startup_context = "startup context"

[realtime]
version = "v2"
type = "conversational"

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
