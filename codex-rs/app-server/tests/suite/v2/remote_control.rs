use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RemoteControlConnectionStatus;
use codex_app_server_protocol::RemoteControlDisableResponse;
use codex_app_server_protocol::RemoteControlEnableResponse;
use codex_app_server_protocol::RemoteControlStatusReadResponse;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
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
    let mut mcp = McpProcess::new(codex_home.path()).await?;
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
    let mut mcp = McpProcess::new(codex_home.path()).await?;
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
    let mut mcp = McpProcess::new(codex_home.path()).await?;
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
    let mut mcp = McpProcess::new(codex_home.path()).await?;
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

struct BlockingRemoteControlBackend {
    enroll_request_rx: Option<oneshot::Receiver<Result<String>>>,
    server_task: JoinHandle<()>,
}

impl BlockingRemoteControlBackend {
    async fn start(codex_home: &std::path::Path) -> Result<Self> {
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

impl Drop for BlockingRemoteControlBackend {
    fn drop(&mut self) {
        self.server_task.abort();
    }
}

async fn read_enroll_request(listener: TcpListener) -> Result<(String, BufReader<TcpStream>)> {
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

    Ok((request_line.trim_end().to_string(), reader))
}
