use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RemoteControlClient;
use codex_app_server_protocol::RemoteControlClientsListOrder;
use codex_app_server_protocol::RemoteControlClientsListParams;
use codex_app_server_protocol::RemoteControlClientsListResponse;
use codex_app_server_protocol::RemoteControlClientsRevokeParams;
use codex_app_server_protocol::RemoteControlClientsRevokeResponse;
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
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
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

#[tokio::test]
async fn remote_control_client_management_works_while_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut backend = ClientManagementRemoteControlBackend::start(codex_home.path()).await?;
    let mut mcp = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_remote_control_clients_list_request(RemoteControlClientsListParams {
            environment_id: "environment-id".to_string(),
            cursor: Some("cursor-id".to_string()),
            limit: Some(10),
            order: Some(RemoteControlClientsListOrder::Desc),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlClientsListResponse = to_response(response)?;
    assert_eq!(
        received,
        RemoteControlClientsListResponse {
            data: vec![RemoteControlClient {
                client_id: "client-id".to_string(),
                display_name: Some("Anton Phone".to_string()),
                device_type: Some("phone".to_string()),
                platform: Some("ios".to_string()),
                os_version: Some("19.0".to_string()),
                device_model: Some("iPhone".to_string()),
                app_version: Some("1.2.3".to_string()),
                last_seen_at: Some(1_772_694_000),
            }],
            next_cursor: Some("next-cursor".to_string()),
        }
    );

    let request_id = mcp
        .send_remote_control_clients_revoke_request(RemoteControlClientsRevokeParams {
            environment_id: "environment-id".to_string(),
            client_id: "client-id".to_string(),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let received: RemoteControlClientsRevokeResponse = to_response(response)?;
    assert_eq!(received, RemoteControlClientsRevokeResponse {});
    assert_eq!(
        timeout(DEFAULT_TIMEOUT, backend.wait_for_requests()).await??,
        vec![
            "GET /backend-api/wham/remote/control/environments/environment-id/clients?cursor=cursor-id&limit=10&order=desc HTTP/1.1".to_string(),
            "DELETE /backend-api/wham/remote/control/environments/environment-id/clients/client-id HTTP/1.1".to_string(),
        ]
    );
    Ok(())
}

struct BlockingRemoteControlBackend {
    enroll_request_rx: Option<oneshot::Receiver<Result<String>>>,
    server_task: JoinHandle<()>,
}

struct ClientManagementRemoteControlBackend {
    requests_rx: Option<oneshot::Receiver<Result<Vec<String>>>>,
    server_task: JoinHandle<()>,
}

impl ClientManagementRemoteControlBackend {
    async fn start(codex_home: &std::path::Path) -> Result<Self> {
        let listener = configured_remote_control_listener(codex_home).await?;
        let (requests_tx, requests_rx) = oneshot::channel();
        let server_task = tokio::spawn(async move {
            let result = async {
                let list_request = read_http_request(&listener).await?;
                let list_request_line = list_request.request_line;
                respond_with_json(
                    list_request.reader.into_inner(),
                    serde_json::json!({
                        "items": [{
                            "client_id": "client-id",
                            "account_user_id": "user-id",
                            "enrollment_status": "enrolled_device_key",
                            "display_name": "Anton Phone",
                            "device_type": "phone",
                            "platform": "ios",
                            "os_version": "19.0",
                            "device_model": "iPhone",
                            "app_version": "1.2.3",
                            "last_seen_at": "2026-03-05T07:00:00Z",
                            "last_seen_city": "San Francisco",
                        }],
                        "cursor": "next-cursor",
                    }),
                )
                .await?;

                let revoke_request = read_http_request(&listener).await?;
                let revoke_request_line = revoke_request.request_line;
                respond_with_status(revoke_request.reader.into_inner(), "204 No Content", "")
                    .await?;

                Ok(vec![list_request_line, revoke_request_line])
            }
            .await;
            let _ = requests_tx.send(result);
        });
        Ok(Self {
            requests_rx: Some(requests_rx),
            server_task,
        })
    }

    async fn wait_for_requests(&mut self) -> Result<Vec<String>> {
        self.requests_rx
            .take()
            .context("requests should only be awaited once")?
            .await?
    }
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

impl Drop for ClientManagementRemoteControlBackend {
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

async fn respond_with_status(mut stream: TcpStream, status: &str, body: &str) -> Result<()> {
    stream
        .write_all(
            format!(
                "HTTP/1.1 {status}\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await?;
    Ok(())
}
