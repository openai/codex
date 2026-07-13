#![cfg(unix)]

mod common;

use codex_exec_server::Environment;
use codex_exec_server::EnvironmentStatus;
use codex_exec_server::EnvironmentStatusKind;
use codex_exec_server::InitializeParams;
use codex_exec_server::InitializeResponse;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCResponse;
use common::exec_server::exec_server;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_serves_readyz_alongside_websocket_endpoint() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let http_base_url = server
        .websocket_url()
        .strip_prefix("ws://")
        .expect("websocket URL should use ws://");

    let response = reqwest::get(format!("http://{http_base_url}/readyz")).await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_environment_fetches_info_from_exec_server() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let environment = Environment::create_for_tests(Some(server.websocket_url().to_string()))?;
    assert!(environment.is_remote());

    let remote_info = environment.info().await?;
    let local_info = Environment::default_for_tests().info().await?;
    assert_eq!(remote_info, local_info);

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_reports_environment_status_over_websocket() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "exec-server-health-test".to_string(),
                resume_session_id: None,
            })?,
        )
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse {
        id,
        result: initialize_result,
    }) = server.next_event().await?
    else {
        panic!("expected initialize response");
    };
    assert_eq!(id, initialize_id);
    let _: InitializeResponse = serde_json::from_value(initialize_result)?;
    server
        .send_notification("initialized", serde_json::json!({}))
        .await?;

    let status_id = server
        .send_request("environment/status", serde_json::json!({}))
        .await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = server.next_event().await?
    else {
        panic!("expected environment status response");
    };
    assert_eq!(id, status_id);
    assert_eq!(
        serde_json::from_value::<EnvironmentStatus>(result)?,
        EnvironmentStatus {
            status: EnvironmentStatusKind::Ready,
        }
    );

    server.shutdown().await?;
    Ok(())
}
