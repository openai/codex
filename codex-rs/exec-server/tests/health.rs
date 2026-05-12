#![cfg(unix)]

mod common;

use common::exec_server::exec_server;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_serves_kubernetes_style_probe_routes() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let http_base_url = server
        .websocket_url()
        .strip_prefix("ws://")
        .expect("websocket URL should use ws://");

    let readyz = reqwest::get(format!("http://{http_base_url}/readyz")).await?;
    assert_eq!(readyz.status(), reqwest::StatusCode::OK);

    let healthz = reqwest::get(format!("http://{http_base_url}/healthz")).await?;
    assert_eq!(healthz.status(), reqwest::StatusCode::OK);

    let legacy_health = reqwest::get(format!("http://{http_base_url}/health")).await?;
    assert_eq!(legacy_health.status(), reqwest::StatusCode::NOT_FOUND);

    server.shutdown().await?;
    Ok(())
}
