use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::EnvironmentAddResponse;
use codex_app_server_protocol::EnvironmentConnectionNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnEnvironmentParams;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::timeout;

use super::exec_server_test_support::accept_exec_server_environment;

const RPC_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECTION_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::test]
async fn environment_add_applies_connect_timeout() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let exec_server_url = format!("ws://{}", listener.local_addr()?);
    let stalled_server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await?;
        let mut request = Vec::new();
        socket.read_to_end(&mut request).await?;
        anyhow::ensure!(!request.is_empty(), "expected a WebSocket handshake");
        Ok::<_, anyhow::Error>(())
    });
    let codex_home = TempDir::new()?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(RPC_TIMEOUT, app_server.initialize()).await??;

    let request_id = app_server
        .send_raw_request(
            "environment/add",
            Some(json!({
                "environmentId": "remote-a",
                "execServerUrl": exec_server_url,
                "connectTimeoutMs": 1_000,
            })),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let _: EnvironmentAddResponse = to_response(response)?;

    timeout(CONNECTION_CLOSE_TIMEOUT, stalled_server).await???;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_environment_emits_connection_lifecycle_notifications() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let exec_server_url = format!("ws://{}", listener.local_addr()?);

    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        "[features]\ndeferred_executor = true\n",
    )?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build()
        .await?;
    timeout(RPC_TIMEOUT, app_server.initialize()).await??;

    let request_id = app_server
        .send_raw_request(
            "environment/add",
            Some(json!({
                "environmentId": "remote-a",
                "execServerUrl": exec_server_url,
            })),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let _: EnvironmentAddResponse = to_response(response)?;

    let environment = TurnEnvironmentParams {
        environment_id: "remote-a".to_string(),
        cwd: codex_utils_absolute_path::AbsolutePathBuf::try_from(codex_home.path().to_path_buf())?
            .into(),
        runtime_workspace_roots: None,
    };
    let request_id = app_server
        .send_thread_start_request(ThreadStartParams {
            environments: Some(vec![environment.clone()]),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(response)?;

    let (disconnect_tx, disconnect_rx) = oneshot::channel();
    let exec_server = tokio::spawn(async move {
        let mut websocket = accept_exec_server_environment(
            listener,
            json!({"shell": {"name": "zsh", "path": "/bin/zsh"}}),
        )
        .await?;
        disconnect_rx.await?;
        websocket.close(None).await?;
        Ok::<_, anyhow::Error>(())
    });

    let connected = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_notification_message("thread/environment/connected"),
    )
    .await??;
    assert_eq!(
        serde_json::from_value::<EnvironmentConnectionNotification>(
            connected.params.expect("connected notification params"),
        )?,
        EnvironmentConnectionNotification {
            thread_id: thread.id.clone(),
            environment_id: "remote-a".to_string(),
        }
    );

    let request_id = app_server
        .send_thread_start_request(ThreadStartParams {
            environments: Some(vec![environment]),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadStartResponse {
        thread: second_thread,
        ..
    } = to_response(response)?;

    disconnect_tx
        .send(())
        .map_err(|_| anyhow::anyhow!("exec-server disconnect receiver closed"))?;
    let mut disconnected = Vec::new();
    for _ in 0..2 {
        let notification = timeout(
            RPC_TIMEOUT,
            app_server.read_stream_until_notification_message("thread/environment/disconnected"),
        )
        .await??;
        disconnected.push(serde_json::from_value::<EnvironmentConnectionNotification>(
            notification
                .params
                .expect("disconnected notification params"),
        )?);
    }
    disconnected.sort_by(|left, right| left.thread_id.cmp(&right.thread_id));
    let mut expected = vec![
        EnvironmentConnectionNotification {
            thread_id: thread.id,
            environment_id: "remote-a".to_string(),
        },
        EnvironmentConnectionNotification {
            thread_id: second_thread.id,
            environment_id: "remote-a".to_string(),
        },
    ];
    expected.sort_by(|left, right| left.thread_id.cmp(&right.thread_id));
    assert_eq!(disconnected, expected);
    assert!(
        !app_server
            .pending_notification_methods()
            .iter()
            .any(|method| method == "thread/environment/connected"),
        "connection state should not be replayed when a thread starts"
    );

    timeout(RPC_TIMEOUT, exec_server).await???;
    Ok(())
}
