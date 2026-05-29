use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RemoteControlConnectionStatus;
use codex_app_server_protocol::RemoteControlDisableResponse;
use codex_app_server_protocol::RemoteControlEnableResponse;
use codex_app_server_protocol::RemoteControlPairingStartParams;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use codex_app_server_protocol::RemoteControlStatusReadResponse;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn remote_control_disable_returns_disabled_status() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_remote_control_disable_request().await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlDisableResponse = to_response(response)?;

    assert_eq!(received.status, RemoteControlConnectionStatus::Disabled);
    assert!(!received.server_name.is_empty());
    assert_eq!(received.environment_id, None);
    assert!(!received.installation_id.is_empty());
    Ok(())
}

#[tokio::test]
async fn remote_control_status_read_returns_disabled_status() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_remote_control_status_read_request().await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlStatusReadResponse = to_response(response)?;

    assert_eq!(received.status, RemoteControlConnectionStatus::Disabled);
    assert!(!received.server_name.is_empty());
    assert_eq!(received.environment_id, None);
    assert!(!received.installation_id.is_empty());
    Ok(())
}

#[tokio::test]
async fn remote_control_enable_returns_connecting_status() -> Result<()> {
    let codex_home = TempDir::new()?;
    let _backend = BlockingRemoteControlBackend::start(codex_home.path()).await?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_remote_control_enable_request().await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlEnableResponse = to_response(response)?;

    assert_eq!(received.status, RemoteControlConnectionStatus::Connecting);
    assert!(!received.server_name.is_empty());
    assert_eq!(received.environment_id, None);
    assert!(!received.installation_id.is_empty());
    Ok(())
}

#[tokio::test]
async fn remote_control_status_read_returns_connecting_status_after_enable() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut backend = BlockingRemoteControlBackend::start(codex_home.path()).await?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_remote_control_enable_request().await?;
    let _: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let enroll_request = timeout(DEFAULT_TIMEOUT, backend.wait_for_enroll_request()).await??;
    assert_eq!(
        enroll_request,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );

    let request_id = mcp.send_remote_control_status_read_request().await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlStatusReadResponse = to_response(response)?;

    assert_eq!(received.status, RemoteControlConnectionStatus::Connecting);
    assert!(!received.server_name.is_empty());
    assert_eq!(received.environment_id, None);
    assert!(!received.installation_id.is_empty());
    Ok(())
}

#[tokio::test]
async fn remote_control_pairing_start_returns_pairing_artifacts() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut backend = PairingRemoteControlBackend::start(codex_home.path()).await?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_remote_control_enable_request().await?;
    let _: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        timeout(DEFAULT_TIMEOUT, backend.wait_for_enroll_request()).await??,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_matching_notification(
            "remoteControl/status/changed enrolled",
            |notification| {
                notification.method == "remoteControl/status/changed"
                    && notification
                        .params
                        .as_ref()
                        .and_then(|params| params.get("environmentId"))
                        .and_then(serde_json::Value::as_str)
                        == Some("environment-id")
            },
        ),
    )
    .await??;

    let request_id = mcp
        .send_remote_control_pairing_start_request(RemoteControlPairingStartParams {
            manual_code: true,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(response.result.get("serverId"), None);
    let received: RemoteControlPairingStartResponse = to_response(response)?;

    assert_eq!(
        received,
        RemoteControlPairingStartResponse {
            pairing_code: "pairing-code".to_string(),
            manual_pairing_code: Some("ABCD-EFGH".to_string()),
            environment_id: "environment-id".to_string(),
            expires_at: 33_336_362_096,
        }
    );
    Ok(())
}

struct BlockingRemoteControlBackend {
    enroll_request_rx: Option<oneshot::Receiver<Result<String>>>,
    server_task: JoinHandle<()>,
}

impl BlockingRemoteControlBackend {
    async fn start(codex_home: &std::path::Path) -> Result<Self> {
        let listener = configured_remote_control_listener(codex_home).await?;

        let (enroll_request_tx, enroll_request_rx) = oneshot::channel();
        let server_task = tokio::spawn(async move {
            match read_enroll_request(listener).await {
                Ok((request_line, _reader)) => {
                    let _ = enroll_request_tx.send(Ok(request_line));
                    std::future::pending::<()>().await;
                }
                Err(err) => {
                    let _ = enroll_request_tx.send(Err(err));
                }
            }
        });

        Ok(Self {
            enroll_request_rx: Some(enroll_request_rx),
            server_task,
        })
    }

    async fn wait_for_enroll_request(&mut self) -> Result<String> {
        let rx = self
            .enroll_request_rx
            .take()
            .context("enroll request should only be awaited once")?;
        rx.await?
    }
}

struct PairingRemoteControlBackend {
    enroll_request_rx: Option<oneshot::Receiver<Result<String>>>,
    server_task: JoinHandle<()>,
}

impl PairingRemoteControlBackend {
    async fn start(codex_home: &std::path::Path) -> Result<Self> {
        let listener = configured_remote_control_listener(codex_home).await?;
        let (enroll_request_tx, enroll_request_rx) = oneshot::channel();
        let server_task = tokio::spawn(async move {
            let mut enroll_request_tx = Some(enroll_request_tx);
            let result = async {
                let enroll_request = read_http_request(&listener).await?;
                if let Some(enroll_request_tx) = enroll_request_tx.take() {
                    let _ = enroll_request_tx.send(Ok(enroll_request.request_line.clone()));
                }
                respond_with_json(
                    enroll_request.reader.into_inner(),
                    serde_json::json!({
                        "server_id": "server-id",
                        "environment_id": "environment-id",
                        "remote_control_token": "remote-control-token",
                        "expires_at": "3026-05-22T12:34:56Z",
                    }),
                )
                .await?;

                let _websocket_request = read_http_request(&listener).await?;
                let pair_http_request = read_http_request(&listener).await?;
                respond_with_json(
                    pair_http_request.reader.into_inner(),
                    serde_json::json!({
                        "pairing_code": "pairing-code",
                        "manual_pairing_code": "ABCD-EFGH",
                        "server_id": "server-id",
                        "environment_id": "environment-id",
                        "expires_at": "3026-05-22T12:34:56Z",
                    }),
                )
                .await?;
                std::future::pending::<()>().await;
                Ok::<(), anyhow::Error>(())
            }
            .await;

            if let Err(err) = result {
                let err = err.to_string();
                if let Some(enroll_request_tx) = enroll_request_tx {
                    let _ = enroll_request_tx.send(Err(anyhow::anyhow!(err)));
                }
            }
        });

        Ok(Self {
            enroll_request_rx: Some(enroll_request_rx),
            server_task,
        })
    }

    async fn wait_for_enroll_request(&mut self) -> Result<String> {
        self.enroll_request_rx
            .take()
            .context("enroll request should only be awaited once")?
            .await?
    }
}

impl Drop for PairingRemoteControlBackend {
    fn drop(&mut self) {
        self.server_task.abort();
    }
}

impl Drop for BlockingRemoteControlBackend {
    fn drop(&mut self) {
        self.server_task.abort();
    }
}

struct HttpRequest {
    request_line: String,
    reader: BufReader<TcpStream>,
}

async fn configured_remote_control_listener(codex_home: &std::path::Path) -> Result<TcpListener> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let remote_control_url = format!("http://{}/backend-api/", listener.local_addr()?);
    write_mock_responses_config_toml_with_chatgpt_base_url(
        codex_home,
        &remote_control_url,
        &remote_control_url,
    )?;
    write_chatgpt_auth(
        codex_home,
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account_id")
            .chatgpt_account_id("account_id"),
        AuthCredentialsStoreMode::File,
    )?;
    Ok(listener)
}

async fn read_enroll_request(listener: TcpListener) -> Result<(String, BufReader<TcpStream>)> {
    let request = read_http_request(&listener).await?;
    Ok((request.request_line, request.reader))
}

async fn read_http_request(listener: &TcpListener) -> Result<HttpRequest> {
    let (stream, _) = listener.accept().await?;
    let mut reader = BufReader::new(stream);

    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        if line == "\r\n" {
            break;
        }
    }

    Ok(HttpRequest {
        request_line: request_line.trim_end().to_string(),
        reader,
    })
}

async fn respond_with_json(stream: TcpStream, body: serde_json::Value) -> Result<()> {
    let body = body.to_string();
    let mut stream = stream;
    stream
        .write_all(
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await?;
    Ok(())
}
