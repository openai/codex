use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::EnvironmentAddResponse;
use codex_app_server_protocol::EnvironmentInfoResponse;
use codex_app_server_protocol::EnvironmentShellInfo;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::time::timeout;

use super::exec_server_test_support::accept_exec_server_environment;

const RPC_TIMEOUT: Duration = Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;
const INTERNAL_ERROR_CODE: i64 = -32603;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn environment_info_returns_remote_environment_info() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let exec_server_url = format!("ws://{}", listener.local_addr()?);
    let exec_server = tokio::spawn(async move {
        accept_exec_server_environment(
            listener,
            json!({
                "shell": {"name": "zsh", "path": "/bin/zsh"},
                "cwd": "file:///workspace",
            }),
        )
        .await?;
        Ok::<_, anyhow::Error>(())
    });

    let codex_home = TempDir::new()?;
    let mut app_server = TestAppServer::new(codex_home.path()).await?;
    timeout(RPC_TIMEOUT, app_server.initialize()).await??;
    add_environment(
        &mut app_server,
        &exec_server_url,
        /*connect_timeout_ms*/ None,
    )
    .await?;

    let request_id = app_server
        .send_raw_request(
            "environment/info",
            Some(json!({"environmentId": "remote-a"})),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        to_response::<EnvironmentInfoResponse>(response)?,
        EnvironmentInfoResponse {
            shell: EnvironmentShellInfo {
                name: "zsh".to_string(),
                path: "/bin/zsh".to_string(),
            },
            cwd: Some(PathUri::parse("file:///workspace")?),
        }
    );
    timeout(RPC_TIMEOUT, exec_server).await???;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn environment_info_accepts_missing_cwd() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let exec_server_url = format!("ws://{}", listener.local_addr()?);
    let exec_server = tokio::spawn(async move {
        accept_exec_server_environment(
            listener,
            json!({"shell": {"name": "zsh", "path": "/bin/zsh"}}),
        )
        .await?;
        Ok::<_, anyhow::Error>(())
    });

    let codex_home = TempDir::new()?;
    let mut app_server = TestAppServer::new(codex_home.path()).await?;
    timeout(RPC_TIMEOUT, app_server.initialize()).await??;
    add_environment(
        &mut app_server,
        &exec_server_url,
        /*connect_timeout_ms*/ None,
    )
    .await?;

    let request_id = app_server
        .send_raw_request(
            "environment/info",
            Some(json!({"environmentId": "remote-a"})),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        to_response::<EnvironmentInfoResponse>(response)?,
        EnvironmentInfoResponse {
            shell: EnvironmentShellInfo {
                name: "zsh".to_string(),
                path: "/bin/zsh".to_string(),
            },
            cwd: None,
        }
    );
    timeout(RPC_TIMEOUT, exec_server).await???;
    Ok(())
}

#[tokio::test]
async fn environment_info_rejects_unknown_environment() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut app_server = TestAppServer::new(codex_home.path()).await?;
    timeout(RPC_TIMEOUT, app_server.initialize()).await??;

    let request_id = app_server
        .send_raw_request(
            "environment/info",
            Some(json!({"environmentId": "missing"})),
        )
        .await?;
    let error = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        error,
        JSONRPCError {
            id: RequestId::Integer(request_id),
            error: JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "unknown environment id `missing`".to_string(),
                data: None,
            },
        }
    );
    Ok(())
}

#[tokio::test]
async fn environment_info_reports_connection_failure() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let exec_server_url = format!("ws://{}", listener.local_addr()?);
    let codex_home = TempDir::new()?;
    let mut app_server = TestAppServer::new(codex_home.path()).await?;
    timeout(RPC_TIMEOUT, app_server.initialize()).await??;
    add_environment(&mut app_server, &exec_server_url, Some(50)).await?;

    let request_id = app_server
        .send_raw_request(
            "environment/info",
            Some(json!({"environmentId": "remote-a"})),
        )
        .await?;
    let error = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.code, INTERNAL_ERROR_CODE);
    assert!(
        error
            .error
            .message
            .contains("failed to get info for environment `remote-a`")
    );
    Ok(())
}

async fn add_environment(
    app_server: &mut TestAppServer,
    exec_server_url: &str,
    connect_timeout_ms: Option<u64>,
) -> Result<()> {
    let request_id = app_server
        .send_raw_request(
            "environment/add",
            Some(json!({
                "environmentId": "remote-a",
                "execServerUrl": exec_server_url,
                "connectTimeoutMs": connect_timeout_ms,
            })),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let _: EnvironmentAddResponse = to_response(response)?;
    Ok(())
}
