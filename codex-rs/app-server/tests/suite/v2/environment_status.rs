use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use codex_app_server_protocol::EnvironmentAddParams;
use codex_app_server_protocol::EnvironmentAddResponse;
use codex_app_server_protocol::EnvironmentStatusKind;
use codex_app_server_protocol::EnvironmentStatusParams;
use codex_app_server_protocol::EnvironmentStatusResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use futures::SinkExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use super::exec_server_test_support::accept_initialized_exec_server;
use super::exec_server_test_support::read_exec_server_json;

const RPC_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn environment_status_reports_connection_states_with_auto_env() -> Result<()> {
    let ready_listener = TcpListener::bind("127.0.0.1:0").await?;
    let ready_exec_server_url = format!("ws://{}", ready_listener.local_addr()?);
    let (ready_connected_tx, ready_connected_rx) = oneshot::channel();
    let (ready_release_tx, mut ready_release_rx) = oneshot::channel();
    let ready_exec_server = tokio::spawn(async move {
        let mut websocket = accept_initialized_exec_server(ready_listener).await?;
        ready_connected_tx
            .send(())
            .map_err(|()| anyhow::anyhow!("status test stopped before exec-server was ready"))?;

        loop {
            tokio::select! {
                _ = &mut ready_release_rx => return Ok::<_, anyhow::Error>(()),
                request = read_exec_server_json(&mut websocket) => {
                    let request = request?;
                    assert_eq!(request["method"], "environment/status");
                    websocket
                        .send(Message::Text(
                            json!({
                                "id": request["id"],
                                "result": {"status": "ready"},
                            })
                            .to_string()
                            .into(),
                        ))
                        .await?;
                }
            }
        }
    });

    let pending_listener = TcpListener::bind("127.0.0.1:0").await?;
    let pending_exec_server_url = format!("ws://{}", pending_listener.local_addr()?);
    let (pending_connected_tx, pending_connected_rx) = oneshot::channel();
    let (pending_release_tx, pending_release_rx) = oneshot::channel();
    let pending_exec_server = tokio::spawn(async move {
        let (stream, _) = pending_listener.accept().await?;
        let _websocket = accept_async(stream).await?;
        pending_connected_tx
            .send(())
            .map_err(|()| anyhow::anyhow!("status test stopped before pending connection"))?;
        let _ = pending_release_rx.await;
        Ok::<_, anyhow::Error>(())
    });

    let disconnected_listener = TcpListener::bind("127.0.0.1:0").await?;
    let disconnected_exec_server_url = format!("ws://{}", disconnected_listener.local_addr()?);
    let (disconnected_tx, disconnected_rx) = oneshot::channel();
    let disconnected_exec_server = tokio::spawn(async move {
        let (stream, _) = disconnected_listener.accept().await?;
        let websocket = accept_async(stream).await?;
        drop(websocket);
        disconnected_tx
            .send(())
            .map_err(|()| anyhow::anyhow!("status test stopped before disconnect"))?;
        Ok::<_, anyhow::Error>(())
    });

    let mut app_server = TestAppServer::builder().build().await?;
    timeout(RPC_TIMEOUT, app_server.initialize()).await??;
    let auto_environment_id = app_server.auto_env()?.selection().environment_id.clone();

    add_environment(&mut app_server, "ready", &ready_exec_server_url).await?;
    add_environment(&mut app_server, "pending", &pending_exec_server_url).await?;
    add_environment(
        &mut app_server,
        "disconnected",
        &disconnected_exec_server_url,
    )
    .await?;
    timeout(RPC_TIMEOUT, ready_connected_rx).await??;
    timeout(RPC_TIMEOUT, pending_connected_rx).await??;
    timeout(RPC_TIMEOUT, disconnected_rx).await??;

    assert_eq!(
        wait_for_status(
            &mut app_server,
            &auto_environment_id,
            EnvironmentStatusKind::Ready,
        )
        .await?,
        EnvironmentStatusResponse {
            status: EnvironmentStatusKind::Ready,
            error: None,
        }
    );
    assert_eq!(
        wait_for_status(&mut app_server, "ready", EnvironmentStatusKind::Ready).await?,
        EnvironmentStatusResponse {
            status: EnvironmentStatusKind::Ready,
            error: None,
        }
    );
    assert_eq!(
        read_environment_status(&mut app_server, "pending").await?,
        EnvironmentStatusResponse {
            status: EnvironmentStatusKind::Pending,
            error: None,
        }
    );
    let disconnected = wait_for_status(
        &mut app_server,
        "disconnected",
        EnvironmentStatusKind::Disconnected,
    )
    .await?;
    let disconnected_error = disconnected.error.clone();
    assert!(disconnected_error.is_some());
    assert_eq!(
        disconnected,
        EnvironmentStatusResponse {
            status: EnvironmentStatusKind::Disconnected,
            error: disconnected_error,
        }
    );
    assert_eq!(
        read_environment_status(&mut app_server, "missing").await?,
        EnvironmentStatusResponse {
            status: EnvironmentStatusKind::Unknown,
            error: Some("unknown environment id `missing`".to_string()),
        }
    );

    let _ = ready_release_tx.send(());
    let _ = pending_release_tx.send(());
    timeout(RPC_TIMEOUT, ready_exec_server).await???;
    timeout(RPC_TIMEOUT, pending_exec_server).await???;
    timeout(RPC_TIMEOUT, disconnected_exec_server).await???;
    Ok(())
}

async fn add_environment(
    app_server: &mut TestAppServer,
    environment_id: &str,
    exec_server_url: &str,
) -> Result<()> {
    let params = EnvironmentAddParams {
        environment_id: environment_id.to_string(),
        exec_server_url: exec_server_url.to_string(),
        connect_timeout_ms: None,
    };
    let add_request_id = app_server
        .send_raw_request("environment/add", Some(serde_json::to_value(params)?))
        .await?;
    let add_response: JSONRPCResponse = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(add_request_id)),
    )
    .await??;
    let _: EnvironmentAddResponse = to_response(add_response)?;
    Ok(())
}

async fn read_environment_status(
    app_server: &mut TestAppServer,
    environment_id: &str,
) -> Result<EnvironmentStatusResponse> {
    let params = EnvironmentStatusParams {
        environment_id: environment_id.to_string(),
    };
    let request_id = app_server
        .send_raw_request("environment/status", Some(serde_json::to_value(params)?))
        .await?;
    let response: JSONRPCResponse = timeout(
        RPC_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn wait_for_status(
    app_server: &mut TestAppServer,
    environment_id: &str,
    expected: EnvironmentStatusKind,
) -> Result<EnvironmentStatusResponse> {
    timeout(RPC_TIMEOUT, async {
        loop {
            let response = read_environment_status(app_server, environment_id).await?;
            if response.status == expected {
                return Ok(response);
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await?
}
