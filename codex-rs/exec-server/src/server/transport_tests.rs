use std::net::SocketAddr;
use std::time::Duration;

use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::duplex;
use tokio::time::timeout;

use super::DEFAULT_LISTEN_URL;
use super::ExecServerListenTransport;
use super::parse_listen_url;
use super::run_stdio_connection_with_io;
use crate::ExecServerRuntimePaths;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeParams;
use crate::protocol::InitializeResponse;

#[test]
fn parse_listen_url_accepts_default_websocket_url() {
    let transport = parse_listen_url(DEFAULT_LISTEN_URL).expect("default listen URL should parse");
    assert_eq!(
        transport,
        ExecServerListenTransport::WebSocket(
            "127.0.0.1:0"
                .parse::<SocketAddr>()
                .expect("valid socket address")
        )
    );
}

#[test]
fn parse_listen_url_accepts_stdio() {
    let transport = parse_listen_url("stdio").expect("stdio listen URL should parse");
    assert_eq!(transport, ExecServerListenTransport::Stdio);
}

#[test]
fn parse_listen_url_accepts_stdio_url() {
    let transport = parse_listen_url("stdio://").expect("stdio listen URL should parse");
    assert_eq!(transport, ExecServerListenTransport::Stdio);
}

#[tokio::test]
async fn stdio_listen_transport_serves_initialize() {
    let transport = parse_listen_url("stdio").expect("stdio listen URL should parse");
    let ExecServerListenTransport::Stdio = transport else {
        panic!("expected stdio listen transport, got {transport:?}");
    };

    let (mut client_writer, server_reader) = duplex(1 << 20);
    let (server_writer, client_reader) = duplex(1 << 20);
    let server_task = tokio::spawn(run_stdio_connection_with_io(
        server_reader,
        server_writer,
        test_runtime_paths(),
    ));
    let mut client_lines = BufReader::new(client_reader).lines();

    let initialize = JSONRPCMessage::Request(JSONRPCRequest {
        id: RequestId::Integer(1),
        method: INITIALIZE_METHOD.to_string(),
        params: Some(
            serde_json::to_value(InitializeParams {
                client_name: "exec-server-transport-test".to_string(),
                resume_session_id: None,
            })
            .expect("initialize params should serialize"),
        ),
        trace: None,
    });
    write_jsonrpc_line(&mut client_writer, &initialize).await;

    let response = timeout(Duration::from_secs(1), client_lines.next_line())
        .await
        .expect("initialize response should arrive")
        .expect("initialize response read should succeed")
        .expect("initialize response should be present");
    let response: JSONRPCMessage =
        serde_json::from_str(&response).expect("initialize response should parse");
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        panic!("expected initialize response, got {response:?}");
    };
    assert_eq!(id, RequestId::Integer(1));
    let initialize_response: InitializeResponse =
        serde_json::from_value(result).expect("initialize response should decode");
    assert!(
        !initialize_response.session_id.is_empty(),
        "initialize should return a session id"
    );

    let initialized = JSONRPCMessage::Notification(JSONRPCNotification {
        method: INITIALIZED_METHOD.to_string(),
        params: Some(serde_json::to_value(()).expect("initialized params should serialize")),
    });
    write_jsonrpc_line(&mut client_writer, &initialized).await;

    drop(client_writer);
    drop(client_lines);
    timeout(Duration::from_secs(1), server_task)
        .await
        .expect("stdio transport should finish after client disconnect")
        .expect("stdio transport task should join")
        .expect("stdio transport should not fail");
}

#[test]
fn parse_listen_url_accepts_websocket_url() {
    let transport =
        parse_listen_url("ws://127.0.0.1:1234").expect("websocket listen URL should parse");
    assert_eq!(
        transport,
        ExecServerListenTransport::WebSocket(
            "127.0.0.1:1234"
                .parse::<SocketAddr>()
                .expect("valid socket address")
        )
    );
}

#[test]
fn parse_listen_url_rejects_invalid_websocket_url() {
    let err = parse_listen_url("ws://localhost:1234")
        .expect_err("hostname bind address should be rejected");
    assert_eq!(
        err.to_string(),
        "invalid websocket --listen URL `ws://localhost:1234`; expected `ws://IP:PORT`"
    );
}

#[test]
fn parse_listen_url_rejects_unsupported_url() {
    let err =
        parse_listen_url("http://127.0.0.1:1234").expect_err("unsupported scheme should fail");
    assert_eq!(
        err.to_string(),
        "unsupported --listen URL `http://127.0.0.1:1234`; expected `ws://IP:PORT` or `stdio`"
    );
}

async fn write_jsonrpc_line(writer: &mut tokio::io::DuplexStream, message: &JSONRPCMessage) {
    let encoded = serde_json::to_vec(message).expect("JSON-RPC message should serialize");
    writer
        .write_all(&encoded)
        .await
        .expect("JSON-RPC message should write");
    writer
        .write_all(b"\n")
        .await
        .expect("JSON-RPC newline should write");
}

fn test_runtime_paths() -> ExecServerRuntimePaths {
    ExecServerRuntimePaths::new(
        std::env::current_exe().expect("current exe"),
        /*codex_linux_sandbox_exe*/ None,
    )
    .expect("runtime paths")
}
