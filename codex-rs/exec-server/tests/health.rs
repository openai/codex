#![cfg(unix)]

mod common;

use codex_exec_server::Environment;
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
