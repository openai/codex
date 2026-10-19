use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::EnvironmentAddResponse;
use codex_app_server_protocol::EnvironmentConnectionNotification;
use codex_app_server_protocol::EnvironmentInfoResponse;
use codex_app_server_protocol::EnvironmentShellInfo;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnEnvironmentParams;
use futures::SinkExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use super::exec_server_test_support::read_exec_server_json;

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

    let (disconnect_tx, mut disconnect_rx) = mpsc::channel(/*buffer*/ 1);
    let exec_server = tokio::spawn(async move {
        // Recover the first disconnect, then reject the next resume. This
        // process boundary cannot share Tokio's paused clock; deadline
        // exhaustion is covered by core's connection-event tests instead.
        for attempt in 0..3 {
            let mut websocket = accept_async(listener.accept().await?.0).await?;
            let initialize = read_exec_server_json(&mut websocket).await?;
            assert_eq!(initialize["method"], "initialize");
            assert_eq!(
                initialize["params"]["resumeSessionId"],
                json!((attempt > 0).then_some("test-session"))
            );
            let response = if attempt == 2 {
                json!({"id": initialize["id"], "error": {
                    "code": -32000, "message": "session no longer exists"
                }})
            } else {
                json!({"id": initialize["id"], "result": {"sessionId": "test-session"}})
            };
            websocket
                .send(Message::Text(response.to_string().into()))
                .await?;
            if attempt == 2 {
                break;
            }
            assert_eq!(
                read_exec_server_json(&mut websocket).await?["method"],
                "initialized"
            );
            loop {
                let request = tokio::select! {
                    command = disconnect_rx.recv() => {
                        command.expect("disconnect command");
                        break;
                    }
                    request = read_exec_server_json(&mut websocket) => request?,
                };
                let response = if request["method"] == "environment/info" {
                    json!({"id": request["id"], "result": {
                        "shell": {"name": "zsh", "path": "/bin/zsh"}
                    }})
                } else {
                    // Startup discovery can probe files on the resumed transport.
                    json!({"id": request["id"], "error": {
                        "code": -32601, "message": "method not supported by test executor"
                    }})
                };
                websocket
                    .send(Message::Text(response.to_string().into()))
                    .await?;
            }
            websocket.close(None).await?;
        }
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
    assert!(
        !app_server
            .pending_notification_methods()
            .iter()
            .any(|method| method == "thread/environment/connected"),
        "connection state should not be replayed when a thread starts"
    );

    for method in [
        "thread/environment/connected",
        "thread/environment/disconnected",
    ] {
        disconnect_tx.send(()).await?;
        let mut notifications = Vec::new();
        for _ in 0..2 {
            notifications.push(
                timeout(
                    RPC_TIMEOUT,
                    app_server.read_notification::<EnvironmentConnectionNotification>(method),
                )
                .await??,
            );
        }
        notifications.sort_by(|left, right| left.thread_id.cmp(&right.thread_id));
        assert_eq!(notifications, expected);
        if method == "thread/environment/connected" {
            let request_id = app_server
                .send_raw_request(
                    "environment/info",
                    Some(json!({"environmentId": "remote-a"})),
                )
                .await?;
            let response = timeout(
                RPC_TIMEOUT,
                app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
            )
            .await??;
            assert_eq!(
                to_response::<EnvironmentInfoResponse>(response)?,
                EnvironmentInfoResponse {
                    shell: EnvironmentShellInfo {
                        name: "zsh".to_string(),
                        path: "/bin/zsh".to_string()
                    },
                    cwd: None,
                }
            );
        }
        assert!(
            timeout(
                Duration::from_millis(100),
                app_server.read_stream_until_matching_notification(
                    "unexpected lifecycle notification",
                    |notification| matches!(
                        notification.method.as_str(),
                        "thread/environment/connected" | "thread/environment/disconnected"
                    ),
                ),
            )
            .await
            .is_err(),
            "expected exactly one {method} event per selected thread"
        );
    }

    timeout(RPC_TIMEOUT, exec_server).await???;
    Ok(())
}
