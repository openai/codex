use anyhow::Context;
use anyhow::Result;
use app_test_support::DISABLE_PLUGIN_STARTUP_TASKS_ARG;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientNotification as ProtocolClientNotification;
use codex_app_server_protocol::ClientRequest as ProtocolClientRequest;
use codex_app_server_protocol::ClientResponse as ProtocolClientResponse;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::RequestId as ProtocolRequestId;
use codex_app_server_transport::NativeServerMessage;
use codex_app_server_transport::decode_grpc_server_message;
use codex_app_server_transport::encode_grpc_client_notification;
use codex_app_server_transport::encode_grpc_client_request;
use codex_app_server_transport::grpc_proto;
use codex_app_server_transport::grpc_proto::HealthRequest;
use codex_app_server_transport::grpc_proto::SchemaRequest;
use codex_app_server_transport::grpc_proto::codex_app_server_client::CodexAppServerClient;
use pretty_assertions::assert_eq;
use std::net::SocketAddr;
use std::path::Path;
use std::process::Stdio;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::timeout;
use tokio_stream::wrappers::ReceiverStream;

use super::test_support::DEFAULT_READ_TIMEOUT;
use super::test_support::create_config_toml;

#[tokio::test]
async fn grpc_transport_initializes_app_server() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;

    let (mut process, bind_addr) = spawn_grpc_server(codex_home.path()).await?;
    let mut client = CodexAppServerClient::connect(format!("http://{bind_addr}"))
        .await
        .context("connect gRPC client")?
        .max_decoding_message_size(16 * 1024 * 1024)
        .max_encoding_message_size(16 * 1024 * 1024);
    let health = client
        .health(HealthRequest {})
        .await
        .context("call gRPC health service")?
        .into_inner();
    assert_eq!(
        health.status,
        grpc_proto::health_response::ServingStatus::Serving as i32
    );
    let schema = client
        .schema(SchemaRequest {})
        .await
        .context("fetch native gRPC schema")?
        .into_inner();
    assert!(schema.proto_source.contains("rpc Session"));
    assert!(!schema.proto_source.contains("Envelope"));

    let (client_tx, client_rx) = mpsc::channel(8);
    let mut server_stream = client
        .session(ReceiverStream::new(client_rx))
        .await
        .context("open app-server gRPC session")?
        .into_inner();

    client_tx
        .send(encode_grpc_client_request(
            ProtocolClientRequest::Initialize {
                request_id: ProtocolRequestId::Integer(1),
                params: InitializeParams {
                    client_info: ClientInfo {
                        name: "grpc-integration-test".to_string(),
                        title: None,
                        version: "0.1.0".to_string(),
                    },
                    capabilities: None,
                },
            },
            None,
        )?)
        .await
        .context("send initialize request")?;

    let initialize_response = next_response(&mut server_stream, 1).await?;
    let ProtocolClientResponse::Initialize { .. } = initialize_response else {
        anyhow::bail!("expected initialize response");
    };

    client_tx
        .send(encode_grpc_client_notification(
            ProtocolClientNotification::Initialized,
        )?)
        .await
        .context("send initialized notification")?;

    client_tx
        .send(encode_grpc_client_request(
            ProtocolClientRequest::ConfigRead {
                request_id: ProtocolRequestId::Integer(2),
                params: ConfigReadParams {
                    include_layers: false,
                    cwd: None,
                },
            },
            None,
        )?)
        .await
        .context("send config/read request")?;
    let config_response = next_response(&mut server_stream, 2).await?;
    let ProtocolClientResponse::ConfigRead {
        response: config_response,
        ..
    } = config_response
    else {
        anyhow::bail!("expected config/read response");
    };
    assert_eq!(config_response.layers, None);

    process
        .kill()
        .await
        .context("failed to stop gRPC app-server process")?;
    Ok(())
}

async fn next_response(
    server_stream: &mut tonic::Streaming<grpc_proto::ServerMessage>,
    expected_id: i64,
) -> Result<ProtocolClientResponse> {
    loop {
        let message = timeout(DEFAULT_READ_TIMEOUT, server_stream.message())
            .await
            .context("timed out waiting for native gRPC response")??
            .context("gRPC stream ended before response")?;
        let NativeServerMessage::Response(response) = decode_grpc_server_message(message)? else {
            continue;
        };
        if response.id() == &ProtocolRequestId::Integer(expected_id) {
            return Ok(response);
        }
    }
}

async fn spawn_grpc_server(codex_home: &Path) -> Result<(Child, SocketAddr)> {
    let program = codex_utils_cargo_bin::cargo_bin("codex-app-server")
        .context("should find app-server binary")?;
    let mut cmd = Command::new(program);
    cmd.arg("--listen")
        .arg("grpc://127.0.0.1:0")
        .arg(DISABLE_PLUGIN_STARTUP_TASKS_ARG)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .env("CODEX_HOME", codex_home)
        .env(
            "CODEX_APP_SERVER_MANAGED_CONFIG_PATH",
            codex_home.join("managed_config.toml"),
        )
        .env("RUST_LOG", "warn");
    let mut process = cmd
        .kill_on_drop(true)
        .spawn()
        .context("failed to spawn gRPC app-server process")?;

    let stderr = process
        .stderr
        .take()
        .context("failed to capture gRPC app-server stderr")?;
    let mut stderr_reader = BufReader::new(stderr).lines();
    let deadline = Instant::now() + DEFAULT_READ_TIMEOUT;
    let bind_addr = loop {
        let line = timeout(
            deadline.saturating_duration_since(Instant::now()),
            stderr_reader.next_line(),
        )
        .await
        .context("timed out waiting for gRPC app-server bind address")?
        .context("failed to read gRPC app-server stderr")?
        .context("gRPC app-server exited before reporting its bind address")?;
        eprintln!("[gRPC app-server stderr] {line}");
        if let Some(bind_addr) = line
            .split_whitespace()
            .find_map(|token| token.strip_prefix("grpc://"))
            .and_then(|addr| addr.parse::<SocketAddr>().ok())
        {
            break bind_addr;
        }
    };

    tokio::spawn(async move {
        while let Ok(Some(line)) = stderr_reader.next_line().await {
            eprintln!("[gRPC app-server stderr] {line}");
        }
    });

    Ok((process, bind_addr))
}
