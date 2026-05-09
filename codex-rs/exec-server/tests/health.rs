#![cfg(unix)]

mod common;

use common::exec_server::exec_server;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_server_serves_health_checks_alongside_websocket_endpoint() -> anyhow::Result<()> {
    let mut server = exec_server().await?;
    let http_base_url = server
        .websocket_url()
        .strip_prefix("ws://")
        .and_then(|url| url.strip_suffix("/ws"))
        .expect("websocket URL should use ws://.../ws");

    for path in ["/", "/readyz", "/healthz"] {
        let response = reqwest::get(format!("http://{http_base_url}{path}")).await?;
        assert_eq!(response.status(), reqwest::StatusCode::OK);
    }

    server.shutdown().await?;
    Ok(())
}
