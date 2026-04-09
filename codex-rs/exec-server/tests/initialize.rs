#![cfg(unix)]

mod common;

use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_exec_server::Environment;
use codex_exec_server::InitializeParams;
use codex_exec_server::InitializeResponse;
use common::exec_server::exec_server_in_cwd;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_accepts_initialize() -> anyhow::Result<()> {
    let remote_cwd = TempDir::new()?;
    let remote_cwd = remote_cwd.path().canonicalize()?;
    let mut server = exec_server_in_cwd(&remote_cwd).await?;
    let initialize_id = server
        .send_request(
            "initialize",
            serde_json::to_value(InitializeParams {
                client_name: "exec-server-test".to_string(),
            })?,
        )
        .await?;

    let response = server.next_event().await?;
    let JSONRPCMessage::Response(JSONRPCResponse { id, result }) = response else {
        panic!("expected initialize response");
    };
    assert_eq!(id, initialize_id);
    let initialize_response: InitializeResponse = serde_json::from_value(result)?;
    assert_eq!(initialize_response, InitializeResponse { cwd: remote_cwd });

    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_environment_uses_exec_server_cwd() -> anyhow::Result<()> {
    let remote_cwd = TempDir::new()?;
    let remote_cwd = remote_cwd.path().canonicalize()?;
    let mut server = exec_server_in_cwd(&remote_cwd).await?;

    let environment = Environment::create(Some(server.websocket_url().to_string())).await?;

    assert_eq!(environment.cwd(), remote_cwd.as_path());

    server.shutdown().await?;
    Ok(())
}
