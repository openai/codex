mod common;

#[path = "../src/proto/codex.exec_server.relay.v1.rs"]
mod relay_proto;

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_exec_server::ExecServerRuntimePaths;
use codex_exec_server::InitializeParams;
use codex_exec_server::InitializeResponse;
use codex_exec_server::RemoteExecutorConfig;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use prost::Message as ProstMessage;
use relay_proto::RelayData;
use relay_proto::RelayMessageFrame;
use relay_proto::RelayReset;
use relay_proto::relay_message_frame;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const EXECUTOR_ID: &str = "exec-mux-test";
const REGISTRY_TOKEN: &str = "registry-token";
const RELAY_MESSAGE_FRAME_VERSION: u32 = 1;
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multiplexed_remote_executor_routes_independent_virtual_streams() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let rendezvous_url = format!("ws://{}", listener.local_addr()?);
    let registry = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/cloud/executor/{EXECUTOR_ID}/register")))
        .and(header("authorization", format!("Bearer {REGISTRY_TOKEN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "executor_id": EXECUTOR_ID,
            "url": rendezvous_url,
        })))
        .mount(&registry)
        .await;

    let (codex_exe, codex_linux_sandbox_exe) = common::current_test_binary_helper_paths()?;
    let runtime_paths = ExecServerRuntimePaths::new(codex_exe, codex_linux_sandbox_exe)?;
    let config = RemoteExecutorConfig::with_bearer_token(
        registry.uri(),
        EXECUTOR_ID.to_string(),
        REGISTRY_TOKEN.to_string(),
    )?;
    let remote_executor = tokio::spawn(codex_exec_server::run_remote_executor(
        config,
        runtime_paths,
    ));

    let (socket, _peer_addr) = timeout(TEST_TIMEOUT, listener.accept())
        .await
        .context("remote executor should connect to fake rendezvous")??;
    let mut websocket = timeout(TEST_TIMEOUT, accept_async(socket))
        .await
        .context("fake rendezvous should accept executor websocket")??;

    let stream_a = "stream-a";
    let stream_b = "stream-b";
    send_relay_message(
        &mut websocket,
        stream_a,
        /*seq*/ 0,
        initialize_request(/*id*/ 1, "relay-test-a")?,
    )
    .await?;
    send_relay_message(
        &mut websocket,
        stream_b,
        /*seq*/ 0,
        initialize_request(/*id*/ 1, "relay-test-b")?,
    )
    .await?;

    let initialize_responses = read_relay_messages_by_stream(&mut websocket, /*count*/ 2).await?;
    let session_a =
        assert_initialize_response(initialize_responses.get(stream_a), stream_a, /*id*/ 1)?;
    let session_b =
        assert_initialize_response(initialize_responses.get(stream_b), stream_b, /*id*/ 1)?;
    assert_ne!(session_a, session_b);

    send_relay_message(
        &mut websocket,
        stream_a,
        /*seq*/ 1,
        notification("initialized", serde_json::json!({})),
    )
    .await?;
    send_relay_message(
        &mut websocket,
        stream_b,
        /*seq*/ 1,
        notification("initialized", serde_json::json!({})),
    )
    .await?;

    send_relay_message(
        &mut websocket,
        stream_a,
        /*seq*/ 2,
        request(/*id*/ 2, "test/unknown-a", serde_json::json!({})),
    )
    .await?;
    send_relay_message(
        &mut websocket,
        stream_b,
        /*seq*/ 2,
        request(/*id*/ 2, "test/unknown-b", serde_json::json!({})),
    )
    .await?;

    let unknown_method_responses =
        read_relay_messages_by_stream(&mut websocket, /*count*/ 2).await?;
    assert_error_response(
        unknown_method_responses.get(stream_a),
        stream_a,
        /*id*/ 2,
        "test/unknown-a",
    )?;
    assert_error_response(
        unknown_method_responses.get(stream_b),
        stream_b,
        /*id*/ 2,
        "test/unknown-b",
    )?;

    send_relay_reset(&mut websocket, stream_a, "test_reset").await?;
    send_relay_message(
        &mut websocket,
        stream_b,
        /*seq*/ 3,
        request(
            /*id*/ 3,
            "test/unknown-b-after-reset",
            serde_json::json!({}),
        ),
    )
    .await?;

    let (stream_id, message) = read_relay_message(&mut websocket).await?;
    assert_eq!(stream_id, stream_b);
    assert_error_response(
        Some(&message),
        stream_b,
        /*id*/ 3,
        "test/unknown-b-after-reset",
    )?;

    websocket.close(None).await?;
    remote_executor.abort();
    let _ = remote_executor.await;
    Ok(())
}

async fn send_relay_message<S>(
    websocket: &mut WebSocketStream<S>,
    stream_id: &str,
    seq: u32,
    message: JSONRPCMessage,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let payload = serde_json::to_vec(&message)?;
    let frame = RelayMessageFrame {
        version: RELAY_MESSAGE_FRAME_VERSION,
        stream_id: stream_id.to_string(),
        ack: 0,
        ack_bits: 0,
        body: Some(relay_message_frame::Body::Data(RelayData {
            seq,
            segment_index: 0,
            segment_count: 1,
            payload,
        })),
    };
    send_relay_frame(websocket, frame).await
}

async fn send_relay_reset<S>(
    websocket: &mut WebSocketStream<S>,
    stream_id: &str,
    reason: &str,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    send_relay_frame(
        websocket,
        RelayMessageFrame {
            version: RELAY_MESSAGE_FRAME_VERSION,
            stream_id: stream_id.to_string(),
            ack: 0,
            ack_bits: 0,
            body: Some(relay_message_frame::Body::Reset(RelayReset {
                reason: reason.to_string(),
            })),
        },
    )
    .await
}

async fn send_relay_frame<S>(
    websocket: &mut WebSocketStream<S>,
    frame: RelayMessageFrame,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    websocket
        .send(Message::Binary(frame.encode_to_vec().into()))
        .await?;
    Ok(())
}

async fn read_relay_messages_by_stream<S>(
    websocket: &mut WebSocketStream<S>,
    count: usize,
) -> Result<HashMap<String, JSONRPCMessage>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let mut messages = HashMap::new();
    for _ in 0..count {
        let (stream_id, message) = read_relay_message(websocket).await?;
        if messages.insert(stream_id.clone(), message).is_some() {
            bail!("received duplicate response for stream {stream_id}");
        }
    }
    Ok(messages)
}

async fn read_relay_message<S>(
    websocket: &mut WebSocketStream<S>,
) -> Result<(String, JSONRPCMessage)>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    loop {
        let frame = timeout(TEST_TIMEOUT, websocket.next())
            .await
            .context("timed out waiting for relay frame")?
            .ok_or_else(|| anyhow!("executor websocket closed"))??;
        match frame {
            Message::Binary(bytes) => {
                let frame = RelayMessageFrame::decode(bytes.as_ref())?;
                let stream_id = frame.stream_id;
                let Some(relay_message_frame::Body::Data(data)) = frame.body else {
                    continue;
                };
                let message = serde_json::from_slice(&data.payload)?;
                return Ok((stream_id, message));
            }
            Message::Ping(_) | Message::Pong(_) => {}
            Message::Close(_) => bail!("executor websocket closed"),
            Message::Text(_) => bail!("executor sent text frame on relay websocket"),
            Message::Frame(_) => {}
        }
    }
}

fn initialize_request(id: i64, client_name: &str) -> Result<JSONRPCMessage> {
    Ok(request(
        id,
        "initialize",
        serde_json::to_value(InitializeParams {
            client_name: client_name.to_string(),
            resume_session_id: None,
        })?,
    ))
}

fn request(id: i64, method: &str, params: serde_json::Value) -> JSONRPCMessage {
    JSONRPCMessage::Request(JSONRPCRequest {
        id: RequestId::Integer(id),
        method: method.to_string(),
        params: Some(params),
        trace: None,
    })
}

fn notification(method: &str, params: serde_json::Value) -> JSONRPCMessage {
    JSONRPCMessage::Notification(JSONRPCNotification {
        method: method.to_string(),
        params: Some(params),
    })
}

fn assert_initialize_response(
    message: Option<&JSONRPCMessage>,
    stream_id: &str,
    id: i64,
) -> Result<Uuid> {
    let message = message.ok_or_else(|| anyhow!("missing initialize response for {stream_id}"))?;
    let JSONRPCMessage::Response(JSONRPCResponse {
        id: response_id,
        result,
    }) = message
    else {
        bail!("expected initialize response for {stream_id}, got {message:?}");
    };
    assert_eq!(response_id, &RequestId::Integer(id));
    let response: InitializeResponse = serde_json::from_value(result.clone())?;
    Ok(Uuid::parse_str(&response.session_id)?)
}

fn assert_error_response(
    message: Option<&JSONRPCMessage>,
    stream_id: &str,
    id: i64,
    expected_method: &str,
) -> Result<()> {
    let message = message.ok_or_else(|| anyhow!("missing error response for {stream_id}"))?;
    let JSONRPCMessage::Error(JSONRPCError {
        id: response_id,
        error,
    }) = message
    else {
        bail!("expected error response for {stream_id}, got {message:?}");
    };
    assert_eq!(response_id, &RequestId::Integer(id));
    assert!(
        error.message.contains(expected_method),
        "expected error for {stream_id} to mention {expected_method}, got {}",
        error.message
    );
    Ok(())
}
