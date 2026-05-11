#![cfg(unix)]

mod common;

use codex_exec_server::ExecServerClient;
use codex_exec_server::RemoteExecServerConnectArgs;
use common::exec_server::http_exec_server;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_upgrade_exec_server_serves_readyz_and_accepts_clients() -> anyhow::Result<()> {
    let mut server = http_exec_server().await?;
    let http_base_url = server
        .websocket_url()
        .strip_prefix("ws+http://")
        .expect("HTTP-upgrade websocket URL should use ws+http://");

    let response = reqwest::get(format!("http://{http_base_url}/readyz")).await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let client = ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
        server.websocket_url().to_string(),
        "exec-server-health-test".to_string(),
    ))
    .await?;
    drop(client);

    server.shutdown().await?;
    Ok(())
}
