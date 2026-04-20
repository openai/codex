use anyhow::Context;
use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::create_shell_command_sse_response;
use app_test_support::to_response;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadRealtimeAppendAudioParams;
use codex_app_server_protocol::ThreadRealtimeAppendAudioResponse;
use codex_app_server_protocol::ThreadRealtimeAppendTextParams;
use codex_app_server_protocol::ThreadRealtimeAppendTextResponse;
use codex_app_server_protocol::ThreadRealtimeAudioChunk;
use codex_app_server_protocol::ThreadRealtimeClosedNotification;
use codex_app_server_protocol::ThreadRealtimeErrorNotification;
use codex_app_server_protocol::ThreadRealtimeItemAddedNotification;
use codex_app_server_protocol::ThreadRealtimeListVoicesParams;
use codex_app_server_protocol::ThreadRealtimeListVoicesResponse;
use codex_app_server_protocol::ThreadRealtimeOutputAudioDeltaNotification;
use codex_app_server_protocol::ThreadRealtimeSdpNotification;
use codex_app_server_protocol::ThreadRealtimeStartParams;
use codex_app_server_protocol::ThreadRealtimeStartResponse;
use codex_app_server_protocol::ThreadRealtimeStartTransport;
use codex_app_server_protocol::ThreadRealtimeStartedNotification;
use codex_app_server_protocol::ThreadRealtimeStopParams;
use codex_app_server_protocol::ThreadRealtimeStopResponse;
use codex_app_server_protocol::ThreadRealtimeTranscriptDeltaNotification;
use codex_app_server_protocol::ThreadRealtimeTranscriptDoneNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartedNotification;
use codex_features::FEATURES;
use codex_features::Feature;
use codex_protocol::protocol::RealtimeConversationVersion;
use codex_protocol::protocol::RealtimeOutputModality;
use codex_protocol::protocol::RealtimeVoice;
use codex_protocol::protocol::RealtimeVoicesList;
use core_test_support::responses;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::responses::WebSocketRequest;
use core_test_support::responses::WebSocketTestServer;
use core_test_support::responses::start_websocket_server;
use core_test_support::responses::start_websocket_server_with_headers;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Match;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request as WiremockRequest;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::path_regex;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const DELEGATED_SHELL_TOOL_TIMEOUT_MS: u64 = 30_000;
const STARTUP_CONTEXT_HEADER: &str = "Startup context from Codex.";
const V2_STEERING_ACKNOWLEDGEMENT: &str =
    "This was sent to steer the previous background agent task.";
const V2_HANDOFF_COMPLETE_ACKNOWLEDGEMENT: &str =
    "Background agent finished. Use the preceding [BACKEND] messages as the result.";

#[derive(Debug, Clone, Copy)]
enum StartupContextConfig<'a> {
    Generated,
    Override(&'a str),
}

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

fn normalized_json_string(raw: &str) -> Result<String> {
    let value: Value = serde_json::from_str(raw).context("expected JSON fixture to parse")?;
    serde_json::to_string(&value).context("expected JSON fixture to serialize")
}

struct GatedSseResponse {
    gate_rx: Mutex<Option<mpsc::Receiver<()>>>,
    response: String,
}

impl Respond for GatedSseResponse {
    fn respond(&self, _: &WiremockRequest) -> ResponseTemplate {
        let gate_rx = self
            .gate_rx
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(gate_rx) = gate_rx {
            let _ = gate_rx.recv();
        }
        responses::sse_response(self.response.clone())
    }
}

#[derive(Debug, Clone, Copy)]
enum RealtimeTestVersion {
    V1,
    V2,
}

impl RealtimeTestVersion {
    fn config_value(self) -> &'static str {
        match self {
            RealtimeTestVersion::V1 => "v1",
            RealtimeTestVersion::V2 => "v2",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum RealtimeTestSandbox {
    ReadOnly,
    DangerFullAccess,
}

impl RealtimeTestSandbox {
    fn config_value(self) -> &'static str {
        match self {
            RealtimeTestSandbox::ReadOnly => "read-only",
            RealtimeTestSandbox::DangerFullAccess => "danger-full-access",
        }
    }
}

#[derive(Debug, PartialEq)]
struct StartedWebrtcRealtime {
    started: ThreadRealtimeStartedNotification,
    sdp: ThreadRealtimeSdpNotification,
}

// Scripted SSE responses for the normal background agent loop. Realtime can ask for a delegated
// background agent turn; that turn talks to this mock `/responses` endpoint and may request
// ordinary tools.
struct MainLoopResponsesScript {
    responses: Vec<String>,
}

// Scripted server events for the direct realtime sideband WebSocket. This mock is the realtime
// session app-server joins after call creation; it is not the background agent Responses stream.
struct RealtimeSidebandScript {
    connections: Vec<WebSocketConnectionConfig>,
}

struct RealtimeE2eHarness {
    mcp: McpProcess,
    _codex_home: TempDir,
    main_loop_responses_server: MockServer,
    realtime_server: WebSocketTestServer,
    call_capture: RealtimeCallRequestCapture,
    thread_id: String,
}

impl RealtimeE2eHarness {
    // Owns the full mocked app-server realtime route: MCP client, Responses mocks, WebRTC call
    // creation capture, sideband WebSocket server, login, config, and a started thread.
    async fn new(
        realtime_version: RealtimeTestVersion,
        main_loop: MainLoopResponsesScript,
        realtime_sideband: RealtimeSidebandScript,
    ) -> Result<Self> {
        let main_loop_responses_server =
            create_mock_responses_server_sequence_unchecked(main_loop.responses).await;
        Self::new_with_main_loop_responses_server_and_sandbox(
            realtime_version,
            main_loop_responses_server,
            realtime_sideband,
            RealtimeTestSandbox::ReadOnly,
        )
        .await
    }

    async fn new_with_sandbox(
        realtime_version: RealtimeTestVersion,
        main_loop: MainLoopResponsesScript,
        realtime_sideband: RealtimeSidebandScript,
        sandbox: RealtimeTestSandbox,
    ) -> Result<Self> {
        let main_loop_responses_server =
            create_mock_responses_server_sequence_unchecked(main_loop.responses).await;
        Self::new_with_main_loop_responses_server_and_sandbox(
            realtime_version,
            main_loop_responses_server,
            realtime_sideband,
            sandbox,
        )
        .await
    }

    async fn new_with_main_loop_responses_server(
        realtime_version: RealtimeTestVersion,
        main_loop_responses_server: MockServer,
        realtime_sideband: RealtimeSidebandScript,
    ) -> Result<Self> {
        Self::new_with_main_loop_responses_server_and_sandbox(
            realtime_version,
            main_loop_responses_server,
            realtime_sideband,
            RealtimeTestSandbox::ReadOnly,
        )
        .await
    }

    async fn new_with_main_loop_responses_server_and_sandbox(
        realtime_version: RealtimeTestVersion,
        main_loop_responses_server: MockServer,
        realtime_sideband: RealtimeSidebandScript,
        sandbox: RealtimeTestSandbox,
    ) -> Result<Self> {
        let call_capture = RealtimeCallRequestCapture::new();
        Mock::given(method("POST"))
            .and(path("/v1/realtime/calls"))
            .and(call_capture.clone())
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Location", "/v1/realtime/calls/rtc_e2e")
                    .set_body_string("v=answer\r\n"),
            )
            .mount(&main_loop_responses_server)
            .await;

        let realtime_server =
            start_websocket_server_with_headers(realtime_sideband.connections).await;
        let codex_home = TempDir::new()?;
        create_config_toml_with_realtime_version(
            codex_home.path(),
            &main_loop_responses_server.uri(),
            realtime_server.uri(),
            /*realtime_enabled*/ true,
            StartupContextConfig::Override("startup context"),
            realtime_version,
            sandbox,
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

        Ok(Self {
            mcp,
            _codex_home: codex_home,
            main_loop_responses_server,
            realtime_server,
            call_capture,
            thread_id: thread_start.thread.id,
        })
    }

    async fn start_webrtc_realtime(&mut self, offer_sdp: &str) -> Result<StartedWebrtcRealtime> {
        // Starts realtime through the public JSON-RPC method, then waits for the same client-visible
        // notifications a desktop app needs: started first, SDP answer second.
        let start_request_id = self
            .mcp
            .send_thread_realtime_start_request(ThreadRealtimeStartParams {
                thread_id: self.thread_id.clone(),
                output_modality: RealtimeOutputModality::Audio,
                prompt: Some(Some("backend prompt".to_string())),
                session_id: None,
                transport: Some(ThreadRealtimeStartTransport::Webrtc {
                    sdp: offer_sdp.to_string(),
                }),
                voice: None,
            })
            .await?;
        let start_response: JSONRPCResponse = timeout(
            DEFAULT_TIMEOUT,
            self.mcp
                .read_stream_until_response_message(RequestId::Integer(start_request_id)),
        )
        .await??;
        let _: ThreadRealtimeStartResponse = to_response(start_response)?;

        let started = self
            .read_notification::<ThreadRealtimeStartedNotification>("thread/realtime/started")
            .await?;
        let sdp = self
            .read_notification::<ThreadRealtimeSdpNotification>("thread/realtime/sdp")
            .await?;

        Ok(StartedWebrtcRealtime { started, sdp })
    }

    async fn read_notification<T: DeserializeOwned>(&mut self, method: &str) -> Result<T> {
        read_notification(&mut self.mcp, method).await
    }

    /// Returns the nth JSON message app-server wrote to the fake Realtime API
    /// sideband websocket.
    async fn sideband_outbound_request(&self, request_index: usize) -> Value {
        self.realtime_server
            .wait_for_request(/*connection_index*/ 0, request_index)
            .await
            .body_json()
    }

    async fn append_audio(&mut self, thread_id: String) -> Result<()> {
        let request_id = self
            .mcp
            .send_thread_realtime_append_audio_request(ThreadRealtimeAppendAudioParams {
                thread_id,
                audio: ThreadRealtimeAudioChunk {
                    data: "BQYH".to_string(),
                    sample_rate: 24_000,
                    num_channels: 1,
                    samples_per_channel: Some(480),
                    item_id: None,
                },
            })
            .await?;
        let response: JSONRPCResponse = timeout(
            DEFAULT_TIMEOUT,
            self.mcp
                .read_stream_until_response_message(RequestId::Integer(request_id)),
        )
        .await??;
        let _: ThreadRealtimeAppendAudioResponse = to_response(response)?;
        Ok(())
    }

    async fn append_text(&mut self, thread_id: String, text: &str) -> Result<()> {
        let request_id = self
            .mcp
            .send_thread_realtime_append_text_request(ThreadRealtimeAppendTextParams {
                thread_id,
                text: text.to_string(),
            })
            .await?;
        let response: JSONRPCResponse = timeout(
            DEFAULT_TIMEOUT,
            self.mcp
                .read_stream_until_response_message(RequestId::Integer(request_id)),
        )
        .await??;
        let _: ThreadRealtimeAppendTextResponse = to_response(response)?;
        Ok(())
    }

    async fn main_loop_responses_requests(&self) -> Result<Vec<Value>> {
        responses_requests(&self.main_loop_responses_server).await
    }

    async fn shutdown(self) {
        self.realtime_server.shutdown().await;
    }
}

fn main_loop_responses(responses: Vec<String>) -> MainLoopResponsesScript {
    MainLoopResponsesScript { responses }
}

fn no_main_loop_responses() -> MainLoopResponsesScript {
    main_loop_responses(Vec::new())
}

fn realtime_sideband(connections: Vec<WebSocketConnectionConfig>) -> RealtimeSidebandScript {
    RealtimeSidebandScript { connections }
}

fn realtime_sideband_connection(
    realtime_server_events: Vec<Vec<Value>>,
) -> WebSocketConnectionConfig {
    WebSocketConnectionConfig {
        requests: realtime_server_events,
        response_headers: Vec::new(),
        accept_delay: None,
        close_after_requests: true,
    }
}

fn open_realtime_sideband_connection(
    realtime_server_events: Vec<Vec<Value>>,
) -> WebSocketConnectionConfig {
    WebSocketConnectionConfig {
        close_after_requests: false,
        ..realtime_sideband_connection(realtime_server_events)
    }
}

fn session_updated(session_id: &str) -> Value {
    json!({
        "type": "session.updated",
        "session": { "id": session_id, "instructions": "backend prompt" }
    })
}

fn v2_background_agent_tool_call(call_id: &str, prompt: &str) -> Value {
    json!({
        "type": "conversation.item.done",
        "item": {
            "id": format!("item_{call_id}"),
            "type": "function_call",
            "name": "background_agent",
            "call_id": call_id,
            "arguments": json!({ "prompt": prompt }).to_string()
        }
    })
}

#[tokio::test]
async fn realtime_conversation_streams_v2_notifications() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("delegated")?,
    ])
    .await;
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
                "type": "response.output_text.done",
                "text": "working on it"
            }),
            json!({
                "type": "conversation.item.done",
                "item": {
                    "id": "item_assistant_1",
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": "working on it" }]
                }
            }),
            json!({
                "type": "conversation.item.done",
                "item": {
                    "id": "item_2",
                    "type": "function_call",
                    "name": "background_agent",
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
        StartupContextConfig::Generated,
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
            output_modality: RealtimeOutputModality::Audio,
            prompt: None,
            session_id: None,
            transport: None,
            voice: Some(RealtimeVoice::Cedar),
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
    assert_eq!(
        startup_context_request.body_json()["session"]["audio"]["output"]["voice"],
        "cedar"
    );
    assert_eq!(
        startup_context_request.body_json()["session"]["output_modalities"],
        json!(["audio"])
    );
    let startup_context_instructions =
        startup_context_request.body_json()["session"]["instructions"]
            .as_str()
            .context("expected startup context instructions")?
            .to_string();
    assert!(startup_context_instructions.starts_with("backend prompt"));
    assert!(startup_context_instructions.contains(STARTUP_CONTEXT_HEADER));

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

    let first_transcript_delta = read_notification::<ThreadRealtimeTranscriptDeltaNotification>(
        &mut mcp,
        "thread/realtime/transcript/delta",
    )
    .await?;
    assert_eq!(first_transcript_delta.thread_id, output_audio.thread_id);
    assert_eq!(first_transcript_delta.role, "user");
    assert_eq!(first_transcript_delta.delta, "delegate now");

    let second_transcript_delta = read_notification::<ThreadRealtimeTranscriptDeltaNotification>(
        &mut mcp,
        "thread/realtime/transcript/delta",
    )
    .await?;
    assert_eq!(second_transcript_delta.thread_id, output_audio.thread_id);
    assert_eq!(second_transcript_delta.role, "assistant");
    assert_eq!(second_transcript_delta.delta, "working");

    let final_transcript_done = read_notification::<ThreadRealtimeTranscriptDoneNotification>(
        &mut mcp,
        "thread/realtime/transcript/done",
    )
    .await?;
    assert_eq!(final_transcript_done.thread_id, output_audio.thread_id);
    assert_eq!(final_transcript_done.role, "assistant");
    assert_eq!(final_transcript_done.text, "working on it");

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
    assert_eq!(
        handoff_item_added.item["active_transcript"],
        json!([
            {"role": "user", "text": "delegate now"},
            {"role": "assistant", "text": "working on it"}
        ])
    );

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
    assert_eq!(connection.len(), 3);
    assert_eq!(
        connection[0].body_json()["type"].as_str(),
        Some("session.update")
    );
    assert_eq!(
        connection[0].body_json()["session"]["instructions"].as_str(),
        Some(startup_context_instructions.as_str()),
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
            "input_audio_buffer.append".to_string(),
        ]
    );

    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn realtime_text_output_modality_requests_text_output_and_final_transcript() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let realtime_server = start_websocket_server(vec![vec![vec![
        json!({
            "type": "session.updated",
            "session": { "id": "sess_text", "instructions": "backend prompt" }
        }),
        json!({
            "type": "response.output_text.delta",
            "delta": "hello "
        }),
        json!({
            "type": "response.output_text.delta",
            "delta": "world"
        }),
        json!({
            "type": "response.output_audio_transcript.done",
            "transcript": "hello world"
        }),
        json!({
            "type": "conversation.item.done",
            "item": {
                "id": "item_output_1",
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "hello world"}]
            }
        }),
    ]]])
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        realtime_server.uri(),
        /*realtime_enabled*/ true,
        StartupContextConfig::Generated,
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
            output_modality: RealtimeOutputModality::Text,
            prompt: None,
            session_id: None,
            transport: None,
            voice: None,
        })
        .await?;
    let start_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_request_id)),
    )
    .await??;
    let _: ThreadRealtimeStartResponse = to_response(start_response)?;

    let session_update = realtime_server
        .wait_for_request(/*connection_index*/ 0, /*request_index*/ 0)
        .await;
    assert_eq!(
        session_update.body_json()["session"]["output_modalities"],
        json!(["text"])
    );

    let first_delta = read_notification::<ThreadRealtimeTranscriptDeltaNotification>(
        &mut mcp,
        "thread/realtime/transcript/delta",
    )
    .await?;
    let second_delta = read_notification::<ThreadRealtimeTranscriptDeltaNotification>(
        &mut mcp,
        "thread/realtime/transcript/delta",
    )
    .await?;
    let done = read_notification::<ThreadRealtimeTranscriptDoneNotification>(
        &mut mcp,
        "thread/realtime/transcript/done",
    )
    .await?;
    assert_eq!(
        vec![first_delta, second_delta],
        vec![
            ThreadRealtimeTranscriptDeltaNotification {
                thread_id: thread_start.thread.id.clone(),
                role: "assistant".to_string(),
                delta: "hello ".to_string(),
            },
            ThreadRealtimeTranscriptDeltaNotification {
                thread_id: thread_start.thread.id.clone(),
                role: "assistant".to_string(),
                delta: "world".to_string(),
            },
        ]
    );
    assert_eq!(
        done,
        ThreadRealtimeTranscriptDoneNotification {
            thread_id: thread_start.thread.id,
            role: "assistant".to_string(),
            text: "hello world".to_string(),
        }
    );
    assert!(
        timeout(
            Duration::from_millis(200),
            mcp.read_stream_until_notification_message("thread/realtime/transcript/done"),
        )
        .await
        .is_err(),
        "should not emit duplicate transcript done from audio transcript done"
    );

    realtime_server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn realtime_list_voices_returns_supported_names() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        "http://127.0.0.1:1",
        "ws://127.0.0.1:1",
        /*realtime_enabled*/ true,
        StartupContextConfig::Generated,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    mcp.initialize().await?;

    let request_id = mcp
        .send_thread_realtime_list_voices_request(ThreadRealtimeListVoicesParams {})
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ThreadRealtimeListVoicesResponse = to_response(response)?;

    assert_eq!(
        response,
        ThreadRealtimeListVoicesResponse {
            voices: RealtimeVoicesList {
                v1: vec![
                    RealtimeVoice::Juniper,
                    RealtimeVoice::Maple,
                    RealtimeVoice::Spruce,
                    RealtimeVoice::Ember,
                    RealtimeVoice::Vale,
                    RealtimeVoice::Breeze,
                    RealtimeVoice::Arbor,
                    RealtimeVoice::Sol,
                    RealtimeVoice::Cove,
                ],
                v2: vec![
                    RealtimeVoice::Alloy,
                    RealtimeVoice::Ash,
                    RealtimeVoice::Ballad,
                    RealtimeVoice::Coral,
                    RealtimeVoice::Echo,
                    RealtimeVoice::Sage,
                    RealtimeVoice::Shimmer,
                    RealtimeVoice::Verse,
                    RealtimeVoice::Marin,
                    RealtimeVoice::Cedar,
                ],
                default_v1: RealtimeVoice::Cove,
                default_v2: RealtimeVoice::Marin,
            },
        }
    );

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
        StartupContextConfig::Generated,
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
            output_modality: RealtimeOutputModality::Audio,
            prompt: Some(Some("backend prompt".to_string())),
            session_id: None,
            transport: None,
            voice: None,
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
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Location", "/v1/realtime/calls/rtc_app_test")
                .set_body_string("v=answer\r\n"),
        )
        .mount(&responses_server)
        .await;
    let realtime_server = start_websocket_server_with_headers(vec![WebSocketConnectionConfig {
        requests: vec![vec![json!({
            "type": "session.updated",
            "session": { "id": "sess_webrtc", "instructions": "backend prompt" }
        })]],
        response_headers: Vec::new(),
        accept_delay: None,
        close_after_requests: false,
    }])
    .await;

    let codex_home = TempDir::new()?;
    create_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        realtime_server.uri(),
        /*realtime_enabled*/ true,
        StartupContextConfig::Override("startup context"),
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
            output_modality: RealtimeOutputModality::Audio,
            prompt: Some(Some("backend prompt".to_string())),
            session_id: None,
            transport: Some(ThreadRealtimeStartTransport::Webrtc {
                sdp: "v=offer\r\n".to_string(),
            }),
            voice: None,
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
    assert_eq!(started.thread_id, thread_id);
    assert_eq!(started.version, RealtimeConversationVersion::V2);

    let sdp_notification =
        read_notification::<ThreadRealtimeSdpNotification>(&mut mcp, "thread/realtime/sdp").await?;
    assert_eq!(
        sdp_notification,
        ThreadRealtimeSdpNotification {
            thread_id: thread_id.clone(),
            sdp: "v=answer\r\n".to_string()
        }
    );

    let session_update = realtime_server
        .wait_for_request(/*connection_index*/ 0, /*request_index*/ 0)
        .await;
    assert_eq!(
        session_update.body_json()["type"].as_str(),
        Some("session.update")
    );
    assert!(
        session_update.body_json()["session"]["instructions"]
            .as_str()
            .context("expected session.update instructions")?
            .contains("startup context")
    );
    assert_eq!(
        realtime_server.single_handshake().uri(),
        "/v1/realtime?call_id=rtc_app_test"
    );

    let stop_request_id = mcp
        .send_thread_realtime_stop_request(ThreadRealtimeStopParams {
            thread_id: thread_id.clone(),
        })
        .await?;
    let stop_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(stop_request_id)),
    )
    .await??;
    let _: ThreadRealtimeStopResponse = to_response(stop_response)?;

    let closed_notification =
        read_notification::<ThreadRealtimeClosedNotification>(&mut mcp, "thread/realtime/closed")
            .await?;
    assert_eq!(closed_notification.thread_id, thread_id);
    assert!(
        matches!(
            closed_notification.reason.as_deref(),
            Some("requested" | "transport_closed")
        ),
        "unexpected close reason: {closed_notification:?}"
    );

    let request = call_capture.single_request();
    assert_eq!(request.url.path(), "/v1/realtime/calls");
    assert_eq!(request.url.query(), None);
    assert_eq!(
        request
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("multipart/form-data; boundary=codex-realtime-call-boundary")
    );
    let body = String::from_utf8(request.body).context("multipart body should be utf-8")?;
    let session = r#"{"tool_choice":"auto","type":"realtime","model":"gpt-realtime-1.5","instructions":"backend prompt\n\nstartup context","output_modalities":["audio"],"audio":{"input":{"format":{"type":"audio/pcm","rate":24000},"noise_reduction":{"type":"near_field"},"transcription":{"model":"gpt-4o-mini-transcribe"},"turn_detection":{"type":"server_vad","interrupt_response":true,"create_response":true,"silence_duration_ms":500}},"output":{"format":{"type":"audio/pcm","rate":24000},"voice":"marin"}},"tools":[{"type":"function","name":"background_agent","description":"Send a user request to the background agent. Use this as the default action. Do not rephrase the user's ask or rewrite it in your own words; pass along the user's own words. If the background agent is idle, this starts a new task and returns the final result to the user. If the background agent is already working on a task, this sends the request as guidance to steer that previous task. If the user asks to do something next, later, after this, or once current work finishes, call this tool so the work is actually queued instead of merely promising to do it later.","parameters":{"type":"object","properties":{"prompt":{"type":"string","description":"The user request to delegate to the background agent."}},"required":["prompt"],"additionalProperties":false}}]}"#;
    let session = normalized_json_string(session)?;
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
async fn webrtc_v1_start_posts_offer_returns_sdp_and_joins_sideband() -> Result<()> {
    skip_if_no_network!(Ok(()));

    // Phase 1: build a v1 realtime thread with a mocked call-create response and a sideband socket
    // that immediately proves the joined connection can receive server events.
    let mut harness = RealtimeE2eHarness::new(
        RealtimeTestVersion::V1,
        no_main_loop_responses(),
        realtime_sideband(vec![open_realtime_sideband_connection(vec![vec![
            session_updated("sess_v1_webrtc"),
        ]])]),
    )
    .await?;

    // Phase 2: start through app-server and assert the app receives both the started notification
    // and the answer SDP.
    let started = harness.start_webrtc_realtime("v=offer\r\n").await?;
    assert_eq!(
        started,
        StartedWebrtcRealtime {
            started: ThreadRealtimeStartedNotification {
                thread_id: harness.thread_id.clone(),
                session_id: Some(harness.thread_id.clone()),
                version: RealtimeConversationVersion::V1,
            },
            sdp: ThreadRealtimeSdpNotification {
                thread_id: harness.thread_id.clone(),
                sdp: "v=answer\r\n".to_string(),
            },
        }
    );

    // Phase 3: verify the HTTP call-create leg, the direct sideband join, and the normal v1
    // session.update; the WebRTC transport should remain alive instead of closing after SDP.
    assert_call_create_multipart(
        harness.call_capture.single_request(),
        "v=offer\r\n",
        v1_session_create_json(),
    )?;
    assert_eq!(
        harness.realtime_server.single_handshake().uri(),
        "/v1/realtime?intent=quicksilver&call_id=rtc_e2e"
    );

    let session_update = harness.sideband_outbound_request(/*request_index*/ 0).await;
    assert_v1_session_update(&session_update)?;

    let closed = timeout(
        Duration::from_millis(100),
        harness
            .mcp
            .read_stream_until_notification_message("thread/realtime/closed"),
    )
    .await;
    assert!(closed.is_err(), "WebRTC start should not close immediately");

    harness.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn webrtc_v1_handoff_request_delegates_and_appends_result() -> Result<()> {
    skip_if_no_network!(Ok(()));

    // Phase 1: script one v1 handoff request on the sideband and one delegated Responses turn.
    let mut harness = RealtimeE2eHarness::new(
        RealtimeTestVersion::V1,
        main_loop_responses(vec![create_final_assistant_message_sse_response(
            "delegated from v1",
        )?]),
        realtime_sideband(vec![realtime_sideband_connection(vec![
            vec![
                session_updated("sess_v1_handoff"),
                json!({
                    "type": "conversation.item.input_audio_transcription.completed",
                    "transcript": "delegate from v1"
                }),
                json!({
                    "type": "response.output_audio_transcript.delta",
                    "delta": "the secret word is "
                }),
                json!({
                    "type": "response.output_audio_transcript.delta",
                    "delta": "kumquat"
                }),
                json!({
                    "type": "conversation.handoff.requested",
                    "handoff_id": "handoff_v1",
                    "item_id": "item_v1",
                    "input_transcript": "delegate from v1"
                }),
            ],
            vec![],
        ])]),
    )
    .await?;

    let started = harness.start_webrtc_realtime("v=offer\r\n").await?;
    assert_eq!(started.started.version, RealtimeConversationVersion::V1);
    assert_call_create_multipart(
        harness.call_capture.single_request(),
        "v=offer\r\n",
        v1_session_create_json(),
    )?;
    assert_v1_session_update(&harness.sideband_outbound_request(/*request_index*/ 0).await)?;

    // Phase 2: wait for the delegated background agent turn that is launched by the handoff request.
    let turn_started = harness
        .read_notification::<TurnStartedNotification>("turn/started")
        .await?;
    assert_eq!(turn_started.thread_id, harness.thread_id);
    let turn_completed = harness
        .read_notification::<TurnCompletedNotification>("turn/completed")
        .await?;
    assert_eq!(turn_completed.thread_id, harness.thread_id);

    // Phase 3: assert the delegated prompt went to Responses, then the v1 handoff append went back
    // over the existing sideband connection.
    let requests = harness.main_loop_responses_requests().await?;
    assert_eq!(requests.len(), 1);
    assert!(
        response_request_contains_text(
            &requests[0],
            "<realtime_delegation>\n  <input>delegate from v1</input>\n  <transcript_delta>user: delegate from v1\nassistant: the secret word is kumquat</transcript_delta>\n</realtime_delegation>",
        ),
        "delegated Responses request should contain realtime delegation envelope: {}",
        requests[0]
    );
    let handoff_append = harness.sideband_outbound_request(/*request_index*/ 1).await;
    assert_eq!(
        handoff_append,
        json!({
            "type": "conversation.handoff.append",
            "handoff_id": "handoff_v1",
            "output_text": "\"Agent Final Message\":\n\ndelegated from v1",
        })
    );

    harness.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn webrtc_v2_forwards_audio_and_text_between_client_and_sideband() -> Result<()> {
    skip_if_no_network!(Ok(()));

    // Phase 1: create a v2 WebRTC conversation whose sideband sends transcript + output audio
    // after the client has had a chance to append input.
    let mut harness = RealtimeE2eHarness::new(
        RealtimeTestVersion::V2,
        no_main_loop_responses(),
        realtime_sideband(vec![realtime_sideband_connection(vec![
            vec![session_updated("sess_v2_stream")],
            vec![],
            vec![
                json!({
                    "type": "conversation.item.input_audio_transcription.delta",
                    "delta": "transcribed audio"
                }),
                json!({
                    "type": "response.output_audio.delta",
                    "delta": "AQID",
                    "sample_rate": 24_000,
                    "channels": 1,
                    "samples_per_channel": 512
                }),
            ],
        ])]),
    )
    .await?;

    let started = harness.start_webrtc_realtime("v=offer\r\n").await?;
    assert_eq!(started.started.version, RealtimeConversationVersion::V2);
    assert_v2_session_update(&harness.sideband_outbound_request(/*request_index*/ 0).await)?;

    // Phase 2: drive app-server as the client would: append audio, append text, then receive
    // transcript/audio notifications that came from the sideband socket.
    let thread_id = started.started.thread_id.clone();
    harness.append_audio(thread_id.clone()).await?;
    harness.append_text(thread_id, "hello").await?;

    let transcript = harness
        .read_notification::<ThreadRealtimeTranscriptDeltaNotification>(
            "thread/realtime/transcript/delta",
        )
        .await?;
    assert_eq!(transcript.delta, "transcribed audio");
    let output_audio = harness
        .read_notification::<ThreadRealtimeOutputAudioDeltaNotification>(
            "thread/realtime/outputAudio/delta",
        )
        .await?;
    assert_eq!(output_audio.audio.data, "AQID");

    // Phase 3: prove the client inputs were translated into the v2 realtime sideband events.
    let requests = [
        harness.sideband_outbound_request(/*request_index*/ 1).await,
        harness.sideband_outbound_request(/*request_index*/ 2).await,
    ];
    assert!(
        requests
            .iter()
            .any(|request| request["type"] == "input_audio_buffer.append"
                && request["audio"] == "BQYH"),
        "sideband requests should include audio append: {requests:?}"
    );
    assert!(
        requests.iter().any(|request| {
            request["type"] == "conversation.item.create"
                && request["item"]["type"] == "message"
                && request["item"]["role"] == "user"
                && request["item"]["content"][0]["type"] == "input_text"
                && request["item"]["content"][0]["text"] == "[USER] hello"
        }),
        "sideband requests should include user text item: {requests:?}"
    );

    harness.shutdown().await;
    Ok(())
}

/// Regression coverage for Realtime V2 text input while a response is active.
///
/// Text input is append-only, so app-server should send the user message without
/// requesting a new realtime response.
#[tokio::test]
async fn webrtc_v2_text_input_is_append_only_while_response_is_active() -> Result<()> {
    skip_if_no_network!(Ok(()));

    // Phase 1: script a server-side response that becomes active after the first
    // user text turn, then finishes only after a later audio input.
    let mut harness = RealtimeE2eHarness::new(
        RealtimeTestVersion::V2,
        no_main_loop_responses(),
        realtime_sideband(vec![realtime_sideband_connection(vec![
            vec![session_updated("sess_v2_response_queue")],
            vec![
                json!({
                    "type": "response.created",
                    "response": { "id": "resp_active" }
                }),
                json!({
                    "type": "response.output_text.delta",
                    "delta": "active response started"
                }),
            ],
            vec![],
            vec![json!({
                "type": "response.done",
                "response": { "id": "resp_active" }
            })],
        ])]),
    )
    .await?;

    let started = harness.start_webrtc_realtime("v=offer\r\n").await?;
    assert_eq!(started.started.version, RealtimeConversationVersion::V2);

    // From here on, `sideband_outbound_request(n)` reads outbound messages to
    // the fake Realtime API sideband websocket. These are not client-facing
    // notifications; they are the protocol frames app-server sends upstream.
    assert_v2_session_update(&harness.sideband_outbound_request(/*request_index*/ 0).await)?;

    // Phase 2: send the first text turn. Text input is append-only, so this
    // sends only the user text item.
    let thread_id = started.started.thread_id.clone();
    harness.append_text(thread_id.clone(), "first").await?;
    assert_v2_user_text_item(
        &harness.sideband_outbound_request(/*request_index*/ 1).await,
        "first",
    );
    let transcript = harness
        .read_notification::<ThreadRealtimeTranscriptDeltaNotification>(
            "thread/realtime/transcript/delta",
        )
        .await?;
    assert_eq!(transcript.delta, "active response started");

    // Phase 3: send a second text turn while `resp_active` is still open. The
    // user message must reach realtime without requesting another response.
    harness.append_text(thread_id.clone(), "second").await?;
    assert_v2_user_text_item(
        &harness.sideband_outbound_request(/*request_index*/ 2).await,
        "second",
    );

    // Phase 4: audio still forwards normally after text input.
    harness.append_audio(thread_id).await?;

    let audio = harness.sideband_outbound_request(/*request_index*/ 3).await;
    assert_eq!(audio["type"], "input_audio_buffer.append");
    assert_eq!(audio["audio"], "BQYH");

    harness.shutdown().await;
    Ok(())
}

/// Regression coverage for append-only Realtime V2 text input when the active
/// response is cancelled instead of completed.
#[tokio::test]
async fn webrtc_v2_text_input_is_append_only_when_response_is_cancelled() -> Result<()> {
    skip_if_no_network!(Ok(()));

    // Phase 1: script a server-side response that becomes active after the first
    // text turn, then is cancelled only after a later audio input.
    let mut harness = RealtimeE2eHarness::new(
        RealtimeTestVersion::V2,
        no_main_loop_responses(),
        realtime_sideband(vec![realtime_sideband_connection(vec![
            vec![session_updated("sess_v2_response_cancel_queue")],
            vec![json!({
                "type": "response.created",
                "response": { "id": "resp_cancelled" }
            })],
            vec![],
            vec![json!({
                "type": "response.cancelled",
                "response": { "id": "resp_cancelled" }
            })],
        ])]),
    )
    .await?;

    let started = harness.start_webrtc_realtime("v=offer\r\n").await?;
    assert_eq!(started.started.version, RealtimeConversationVersion::V2);
    assert_v2_session_update(&harness.sideband_outbound_request(/*request_index*/ 0).await)?;

    // Phase 2: send the first text turn. Text input is append-only, so this
    // sends only the user text item.
    let thread_id = started.started.thread_id.clone();
    harness.append_text(thread_id.clone(), "first").await?;
    assert_v2_user_text_item(
        &harness.sideband_outbound_request(/*request_index*/ 1).await,
        "first",
    );

    // Phase 3: send a second text turn while `resp_cancelled` is still open.
    // The user message must reach realtime without requesting another response.
    harness.append_text(thread_id.clone(), "second").await?;
    assert_v2_user_text_item(
        &harness.sideband_outbound_request(/*request_index*/ 2).await,
        "second",
    );

    // Phase 4: audio still forwards normally after text input.
    harness.append_audio(thread_id).await?;

    let audio = harness.sideband_outbound_request(/*request_index*/ 3).await;
    assert_eq!(audio["type"], "input_audio_buffer.append");
    assert_eq!(audio["audio"], "BQYH");

    harness.shutdown().await;
    Ok(())
}

/// Regression coverage for the Realtime V2 background-agent final-output path.
///
/// Once the background agent finishes, app-server sends the final function-call
/// output to realtime and then requests a new `response.create` so realtime can
/// react to that final output.
#[tokio::test]
async fn webrtc_v2_background_agent_tool_call_delegates_and_returns_function_output() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    // Phase 1: script a v2 background agent function call and a delegated Responses turn that
    // returns final assistant text.
    let mut harness = RealtimeE2eHarness::new(
        RealtimeTestVersion::V2,
        main_loop_responses(vec![create_final_assistant_message_sse_response(
            "delegated from v2",
        )?]),
        realtime_sideband(vec![realtime_sideband_connection(vec![
            vec![
                session_updated("sess_v2_tool"),
                json!({
                    "type": "conversation.item.input_audio_transcription.completed",
                    "transcript": "Hi how are you"
                }),
                json!({
                    "type": "response.output_audio_transcript.done",
                    "transcript": "Doing well, what can I help you with?"
                }),
                json!({
                    "type": "conversation.item.input_audio_transcription.completed",
                    "transcript": "The secret word is strawberry"
                }),
                json!({
                    "type": "conversation.item.created",
                    "item": {
                        "type": "message",
                        "role": "user",
                        "content": [{
                            "type": "input_text",
                            "text": "<realtime_collaboration_update><voice_policy>silent_delegate</voice_policy></realtime_collaboration_update>"
                        }]
                    }
                }),
                json!({
                    "type": "response.output_audio_transcript.delta",
                    "delta": "Got it-strawberry. What's next on the menu?"
                }),
                v2_background_agent_tool_call("call_v2", "run ls"),
            ],
            vec![],
            vec![],
            vec![],
        ])]),
    )
    .await?;

    let started = harness.start_webrtc_realtime("v=offer\r\n").await?;
    assert_eq!(started.started.version, RealtimeConversationVersion::V2);

    // Phase 2: wait for the delegated turn lifecycle kicked off by the v2 function-call item.
    let turn_started = harness
        .read_notification::<TurnStartedNotification>("turn/started")
        .await?;
    assert_eq!(turn_started.thread_id, harness.thread_id);
    let turn_completed = harness
        .read_notification::<TurnCompletedNotification>("turn/completed")
        .await?;
    assert_eq!(turn_completed.thread_id, harness.thread_id);

    // Phase 3: assert the delegated prompt went to Responses and the result
    // returned as exactly one v2 function-call output event on the sideband.
    let requests = harness.main_loop_responses_requests().await?;
    assert_eq!(requests.len(), 1);
    assert!(
        response_request_contains_text(
            &requests[0],
            "<realtime_delegation>\n  <input>run ls</input>\n  <transcript_delta>user: Hi how are you\nassistant: Doing well, what can I help you with?\nuser: The secret word is strawberry\nassistant: Got it-strawberry. What's next on the menu?\nuser: run ls</transcript_delta>\n</realtime_delegation>",
        ),
        "delegated Responses request should contain realtime delegation envelope: {}",
        requests[0]
    );
    assert!(
        !response_request_contains_text(&requests[0], "<realtime_collaboration_update>"),
        "delegated Responses request should not include realtime control injects: {}",
        requests[0]
    );

    let progress = harness.sideband_outbound_request(/*request_index*/ 1).await;
    assert_v2_progress_update(&progress, "delegated from v2");

    let tool_output = harness.sideband_outbound_request(/*request_index*/ 2).await;
    assert_v2_function_call_output(&tool_output, "call_v2", V2_HANDOFF_COMPLETE_ACKNOWLEDGEMENT);
    assert_eq!(
        function_call_output_sideband_requests(&harness.realtime_server).len(),
        1
    );

    // Phase 4: after the final function-call output, realtime needs an explicit
    // `response.create` to produce the next user-visible response.
    assert_v2_response_create(&harness.sideband_outbound_request(/*request_index*/ 3).await);

    harness.shutdown().await;
    Ok(())
}

/// Regression coverage for Realtime V2 steering while a background-agent task is
/// already active.
///
/// The second background-agent tool call is treated as guidance for the active
/// task. App-server acknowledges that steering message to realtime and then
/// emits `response.create` so realtime can speak that acknowledgement.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn webrtc_v2_background_agent_steering_ack_requests_response_create() -> Result<()> {
    skip_if_no_network!(Ok(()));

    // Phase 1: gate the delegated Responses turn from the first tool call so
    // the background-agent handoff stays active while realtime sends a second
    // tool call that should steer the active task.
    let main_loop_responses_server = responses::start_mock_server().await;
    let (gate_completed_tx, gate_completed_rx) = mpsc::channel();
    let gated_response = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "first task finished"),
        responses::ev_completed("resp-1"),
    ]);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(GatedSseResponse {
            gate_rx: Mutex::new(Some(gate_completed_rx)),
            response: gated_response,
        })
        .expect(2)
        .mount(&main_loop_responses_server)
        .await;

    let mut harness = RealtimeE2eHarness::new_with_main_loop_responses_server(
        RealtimeTestVersion::V2,
        main_loop_responses_server,
        realtime_sideband(vec![realtime_sideband_connection(vec![
            vec![
                session_updated("sess_v2_steering_ack"),
                v2_background_agent_tool_call("call_active", "start a task"),
                v2_background_agent_tool_call("call_steer", "steer the active task"),
            ],
            vec![],
            vec![],
            vec![],
            vec![],
        ])]),
    )
    .await?;

    let started = harness.start_webrtc_realtime("v=offer\r\n").await?;
    assert_eq!(started.started.version, RealtimeConversationVersion::V2);
    assert_v2_session_update(&harness.sideband_outbound_request(/*request_index*/ 0).await)?;
    let turn_started = harness
        .read_notification::<TurnStartedNotification>("turn/started")
        .await?;
    assert_eq!(turn_started.thread_id, harness.thread_id);

    // Phase 2: the second tool call happens while `call_active` is still
    // running, so app-server sends a steering acknowledgement as a function-call
    // output for the second call.
    assert_v2_function_call_output(
        &harness.sideband_outbound_request(/*request_index*/ 1).await,
        "call_steer",
        V2_STEERING_ACKNOWLEDGEMENT,
    );

    // Phase 3: realtime needs a `response.create` after the steering
    // acknowledgement so it can surface that acknowledgement to the user.
    assert_v2_response_create(&harness.sideband_outbound_request(/*request_index*/ 2).await);

    // Phase 4: release the gated delegated turn. Codex should then continue
    // the same run with the steering text included in the follow-up Responses
    // request, proving realtime did not merely acknowledge and drop it.
    let _ = gate_completed_tx.send(());
    let turn_completed = harness
        .read_notification::<TurnCompletedNotification>("turn/completed")
        .await?;
    assert_eq!(turn_completed.thread_id, harness.thread_id);

    let requests = harness.main_loop_responses_requests().await?;
    assert_eq!(requests.len(), 2);
    assert!(
        response_request_contains_text(&requests[1], "steer the active task"),
        "follow-up Responses request should contain steering prompt: {}",
        requests[1]
    );

    harness.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn webrtc_v2_background_agent_progress_is_sent_before_function_output() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let mut harness = RealtimeE2eHarness::new(
        RealtimeTestVersion::V2,
        main_loop_responses(vec![create_final_assistant_message_sse_response(
            "progress before final",
        )?]),
        realtime_sideband(vec![realtime_sideband_connection(vec![
            vec![
                session_updated("sess_v2_progress_before_final"),
                v2_background_agent_tool_call("call_progress_order", "stream progress"),
            ],
            vec![],
            vec![],
        ])]),
    )
    .await?;

    let started = harness.start_webrtc_realtime("v=offer\r\n").await?;
    assert_eq!(started.started.version, RealtimeConversationVersion::V2);

    let turn_completed = harness
        .read_notification::<TurnCompletedNotification>("turn/completed")
        .await?;
    assert_eq!(turn_completed.thread_id, harness.thread_id);

    let progress = harness.sideband_outbound_request(/*request_index*/ 1).await;
    assert_v2_progress_update(&progress, "progress before final");

    let tool_output = harness.sideband_outbound_request(/*request_index*/ 2).await;
    assert_v2_function_call_output(
        &tool_output,
        "call_progress_order",
        V2_HANDOFF_COMPLETE_ACKNOWLEDGEMENT,
    );

    harness.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn webrtc_v2_tool_call_delegated_turn_can_execute_shell_tool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    // Phase 1: keep the two mocked OpenAI conversations explicit. The realtime sideband only
    // calls the `background_agent` function; the shell command is requested by the delegated
    // background agent Responses turn that app-server starts after receiving that function call.
    let main_loop = main_loop_responses(vec![
        create_shell_command_sse_response(
            realtime_tool_ok_command(),
            /*workdir*/ None,
            // Windows CI can spend several seconds starting the nested PowerShell command. This
            // test verifies delegated shell-tool plumbing, not timeout enforcement.
            Some(DELEGATED_SHELL_TOOL_TIMEOUT_MS),
            "shell_call",
        )?,
        create_final_assistant_message_sse_response("shell tool finished")?,
    ]);
    let realtime = realtime_sideband(vec![realtime_sideband_connection(vec![
        vec![
            session_updated("sess_v2_shell"),
            v2_background_agent_tool_call("call_shell", "run shell through delegated turn"),
        ],
        vec![],
        vec![],
    ])]);

    let mut harness = RealtimeE2eHarness::new_with_sandbox(
        RealtimeTestVersion::V2,
        main_loop,
        realtime,
        RealtimeTestSandbox::DangerFullAccess,
    )
    .await?;

    let _ = harness.start_webrtc_realtime("v=offer\r\n").await?;

    // Phase 2: observe the delegated background agent turn executing the requested shell command.
    let started_command = wait_for_started_command_execution(&mut harness.mcp).await?;
    let ThreadItem::CommandExecution { id, status, .. } = started_command.item else {
        unreachable!("helper returns command execution items");
    };
    assert_eq!(
        (id.as_str(), status),
        ("shell_call", CommandExecutionStatus::InProgress)
    );

    let completed_command = wait_for_completed_command_execution(&mut harness.mcp).await?;
    let ThreadItem::CommandExecution {
        id,
        status,
        aggregated_output,
        ..
    } = completed_command.item
    else {
        unreachable!("helper returns command execution items");
    };
    assert_eq!(id.as_str(), "shell_call");
    assert_eq!(status, CommandExecutionStatus::Completed);
    assert_eq!(aggregated_output.as_deref(), Some("realtime-tool-ok"));

    // Phase 3: verify the shell output reached Responses and the final delegated answer returned
    // to realtime as a single function-call-output item.
    let turn_completed = harness
        .read_notification::<TurnCompletedNotification>("turn/completed")
        .await?;
    assert_eq!(turn_completed.thread_id, harness.thread_id);

    let requests = harness.main_loop_responses_requests().await?;
    assert_eq!(requests.len(), 2);
    assert!(
        response_request_contains_text(&requests[1], "realtime-tool-ok"),
        "follow-up Responses request should contain shell output: {}",
        requests[1]
    );

    let progress = harness.sideband_outbound_request(/*request_index*/ 1).await;
    assert_v2_progress_update(&progress, "shell tool finished");

    let tool_output = harness.sideband_outbound_request(/*request_index*/ 2).await;
    assert_v2_function_call_output(
        &tool_output,
        "call_shell",
        V2_HANDOFF_COMPLETE_ACKNOWLEDGEMENT,
    );
    assert_eq!(
        function_call_output_sideband_requests(&harness.realtime_server).len(),
        1
    );

    harness.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn webrtc_v2_tool_call_does_not_block_sideband_audio() -> Result<()> {
    skip_if_no_network!(Ok(()));

    // Phase 1: gate the delegated Responses stream so the sideband can send audio while the tool
    // call is still waiting on its delegated turn.
    let main_loop_responses_server = responses::start_mock_server().await;
    let (gate_completed_tx, gate_completed_rx) = mpsc::channel();
    let gated_response = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "late delegated result"),
        responses::ev_completed("resp-1"),
    ]);
    Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .respond_with(GatedSseResponse {
            gate_rx: Mutex::new(Some(gate_completed_rx)),
            response: gated_response,
        })
        .expect(1)
        .mount(&main_loop_responses_server)
        .await;

    let mut harness = RealtimeE2eHarness::new_with_main_loop_responses_server(
        RealtimeTestVersion::V2,
        main_loop_responses_server,
        realtime_sideband(vec![realtime_sideband_connection(vec![
            vec![
                session_updated("sess_v2_nonblocking"),
                v2_background_agent_tool_call("call_audio", "delegate while audio continues"),
                json!({
                    "type": "response.output_audio.delta",
                    "delta": "CQoL",
                    "sample_rate": 24_000,
                    "channels": 1,
                    "samples_per_channel": 256
                }),
            ],
            vec![],
            vec![],
        ])]),
    )
    .await?;

    let _ = harness.start_webrtc_realtime("v=offer\r\n").await?;
    let _ = harness
        .read_notification::<TurnStartedNotification>("turn/started")
        .await?;

    // Phase 2: require app-server to fan out sideband audio before the delegated tool call is
    // allowed to finish.
    let audio = harness
        .read_notification::<ThreadRealtimeOutputAudioDeltaNotification>(
            "thread/realtime/outputAudio/delta",
        )
        .await?;
    assert_eq!(audio.audio.data, "CQoL");

    // Phase 3: release the delegated turn and assert the sideband function-call output is delivered
    // after the nonblocking audio.
    let _ = gate_completed_tx.send(());
    let turn_completed = harness
        .read_notification::<TurnCompletedNotification>("turn/completed")
        .await?;
    assert_eq!(turn_completed.thread_id, harness.thread_id);

    let progress = harness.sideband_outbound_request(/*request_index*/ 1).await;
    assert_v2_progress_update(&progress, "late delegated result");

    let tool_output = harness.sideband_outbound_request(/*request_index*/ 2).await;
    assert_v2_function_call_output(
        &tool_output,
        "call_audio",
        V2_HANDOFF_COMPLETE_ACKNOWLEDGEMENT,
    );

    harness.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn realtime_webrtc_start_surfaces_backend_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    // Phase 1: make call creation fail before any sideband connection can matter.
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
        StartupContextConfig::Override("startup context"),
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    mcp.initialize().await?;
    login_with_api_key(&mut mcp, "sk-test-key").await?;

    // Phase 2: start a normal app-server thread and request realtime over WebRTC.
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
            output_modality: RealtimeOutputModality::Audio,
            prompt: Some(Some("backend prompt".to_string())),
            session_id: None,
            transport: Some(ThreadRealtimeStartTransport::Webrtc {
                sdp: "v=offer\r\n".to_string(),
            }),
            voice: None,
        })
        .await?;
    let start_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_request_id)),
    )
    .await??;
    let _: ThreadRealtimeStartResponse = to_response(start_response)?;

    // Phase 3: the JSON-RPC start request returns, and the realtime failure is delivered as the
    // typed realtime error notification.
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
        StartupContextConfig::Generated,
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
            output_modality: RealtimeOutputModality::Audio,
            prompt: Some(Some("backend prompt".to_string())),
            session_id: None,
            transport: None,
            voice: None,
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

async fn wait_for_started_command_execution(
    mcp: &mut McpProcess,
) -> Result<ItemStartedNotification> {
    loop {
        let started = read_notification::<ItemStartedNotification>(mcp, "item/started").await?;
        if let ThreadItem::CommandExecution { .. } = &started.item {
            return Ok(started);
        }
    }
}

async fn wait_for_completed_command_execution(
    mcp: &mut McpProcess,
) -> Result<ItemCompletedNotification> {
    loop {
        let completed =
            read_notification::<ItemCompletedNotification>(mcp, "item/completed").await?;
        if let ThreadItem::CommandExecution { .. } = &completed.item {
            return Ok(completed);
        }
    }
}

async fn responses_requests(server: &MockServer) -> Result<Vec<Value>> {
    server
        .received_requests()
        .await
        .context("failed to fetch received requests")?
        .into_iter()
        .filter(|request| request.url.path().ends_with("/responses"))
        .map(|request| {
            request
                .body_json::<Value>()
                .context("Responses request body should be JSON")
        })
        .collect()
}

fn response_request_contains_text(request: &Value, text: &str) -> bool {
    match request {
        Value::String(value) => value.contains(text),
        Value::Array(values) => values
            .iter()
            .any(|value| response_request_contains_text(value, text)),
        Value::Object(map) => map
            .values()
            .any(|value| response_request_contains_text(value, text)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn realtime_tool_ok_command() -> Vec<String> {
    #[cfg(windows)]
    {
        vec![
            "powershell.exe".to_string(),
            "-NoProfile".to_string(),
            "-Command".to_string(),
            "[Console]::Write('realtime-tool-ok')".to_string(),
        ]
    }

    #[cfg(not(windows))]
    {
        vec!["printf".to_string(), "realtime-tool-ok".to_string()]
    }
}

fn function_call_output_sideband_requests(server: &WebSocketTestServer) -> Vec<Value> {
    server
        .single_connection()
        .iter()
        .map(WebSocketRequest::body_json)
        .filter(|request| {
            request["type"] == "conversation.item.create"
                && request["item"]["type"] == "function_call_output"
        })
        .collect()
}

fn assert_v2_function_call_output(request: &Value, call_id: &str, expected_output: &str) {
    assert_eq!(
        request,
        &json!({
            "type": "conversation.item.create",
            "item": {
                "type": "function_call_output",
                "call_id": call_id,
                "output": expected_output,
            }
        })
    );
}

fn assert_v2_progress_update(request: &Value, expected_text: &str) {
    assert_eq!(
        request,
        &json!({
            "type": "conversation.item.create",
            "item": {
                "type": "message",
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": format!("[BACKEND] {expected_text}")
                }]
            }
        })
    );
}

fn assert_v2_user_text_item(request: &Value, expected_text: &str) {
    assert_eq!(
        request,
        &json!({
            "type": "conversation.item.create",
            "item": {
                "type": "message",
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": format!("[USER] {expected_text}")
                }]
            }
        })
    );
}

fn assert_v2_response_create(request: &Value) {
    assert_eq!(
        request,
        &json!({
            "type": "response.create"
        })
    );
}

fn assert_v1_session_update(request: &Value) -> Result<()> {
    assert_eq!(request["type"].as_str(), Some("session.update"));
    assert_eq!(request["session"]["type"].as_str(), Some("quicksilver"));
    assert!(
        request["session"]["instructions"]
            .as_str()
            .context("v1 session.update instructions")?
            .contains("startup context")
    );
    assert_eq!(
        request["session"]["audio"]["output"]["voice"].as_str(),
        Some("cove")
    );
    assert_eq!(request["session"]["tools"], Value::Null);
    Ok(())
}

fn assert_v2_session_update(request: &Value) -> Result<()> {
    assert_eq!(request["type"].as_str(), Some("session.update"));
    assert_eq!(request["session"]["type"].as_str(), Some("realtime"));
    assert!(
        request["session"]["instructions"]
            .as_str()
            .context("v2 session.update instructions")?
            .contains("startup context")
    );
    assert_eq!(
        request["session"]["tools"][0]["name"].as_str(),
        Some("background_agent")
    );
    assert_eq!(
        request["session"]["audio"]["input"]["transcription"]["model"].as_str(),
        Some("gpt-4o-mini-transcribe")
    );
    Ok(())
}

fn assert_call_create_multipart(
    request: WiremockRequest,
    offer_sdp: &str,
    session: &str,
) -> Result<()> {
    assert_eq!(request.url.path(), "/v1/realtime/calls");
    assert_eq!(request.url.query(), None);
    assert_eq!(
        request
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("multipart/form-data; boundary=codex-realtime-call-boundary")
    );
    let body = String::from_utf8(request.body).context("multipart body should be utf-8")?;
    let session = normalized_json_string(session)?;
    assert_eq!(
        body,
        format!(
            "--codex-realtime-call-boundary\r\n\
             Content-Disposition: form-data; name=\"sdp\"\r\n\
             Content-Type: application/sdp\r\n\
             \r\n\
             {offer_sdp}\r\n\
             --codex-realtime-call-boundary\r\n\
             Content-Disposition: form-data; name=\"session\"\r\n\
             Content-Type: application/json\r\n\
             \r\n\
             {session}\r\n\
             --codex-realtime-call-boundary--\r\n"
        )
    );
    Ok(())
}

fn v1_session_create_json() -> &'static str {
    r#"{"audio":{"input":{"format":{"type":"audio/pcm","rate":24000}},"output":{"voice":"cove"}},"type":"quicksilver","model":"gpt-realtime-1.5","instructions":"backend prompt\n\nstartup context"}"#
}

fn create_config_toml(
    codex_home: &Path,
    responses_server_uri: &str,
    realtime_server_uri: &str,
    realtime_enabled: bool,
    startup_context: StartupContextConfig<'_>,
) -> std::io::Result<()> {
    create_config_toml_with_realtime_version(
        codex_home,
        responses_server_uri,
        realtime_server_uri,
        realtime_enabled,
        startup_context,
        RealtimeTestVersion::V2,
        RealtimeTestSandbox::ReadOnly,
    )
}

fn create_config_toml_with_realtime_version(
    codex_home: &Path,
    responses_server_uri: &str,
    realtime_server_uri: &str,
    realtime_enabled: bool,
    startup_context: StartupContextConfig<'_>,
    realtime_version: RealtimeTestVersion,
    sandbox: RealtimeTestSandbox,
) -> std::io::Result<()> {
    let realtime_feature_key = FEATURES
        .iter()
        .find(|spec| spec.id == Feature::RealtimeConversation)
        .map(|spec| spec.key)
        .unwrap_or("realtime_conversation");
    let realtime_version = realtime_version.config_value();
    let sandbox = sandbox.config_value();
    let startup_context = match startup_context {
        StartupContextConfig::Generated => String::new(),
        StartupContextConfig::Override(context) => {
            format!("experimental_realtime_ws_startup_context = {context:?}\n")
        }
    };

    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "{sandbox}"
model_provider = "mock_provider"
experimental_realtime_ws_base_url = "{realtime_server_uri}"
experimental_realtime_ws_backend_prompt = "backend prompt"
{startup_context}

[realtime]
version = "{realtime_version}"
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
