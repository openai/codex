#![cfg(unix)]

mod common;

use common::exec_server::connect_websocket_when_ready;
use common::exec_server::exec_server;
use common::exec_server::exec_server_with_connection_token;
use pretty_assertions::assert_eq;
use tokio_tungstenite::tungstenite::Error;

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
async fn exec_server_connection_token_gates_websocket_only() -> anyhow::Result<()> {
    let mut server = exec_server_with_connection_token("secret").await?;
    let http_base_url = server
        .websocket_url()
        .strip_prefix("ws://")
        .expect("websocket URL should use ws://");
    let response = reqwest::get(format!("http://{http_base_url}/readyz")).await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let err = connect_websocket_when_ready(server.websocket_url())
        .await
        .expect_err("missing connection token should reject websocket upgrade");
    assert_unauthorized_websocket_error(err);

    server.shutdown().await?;
    Ok(())
}

fn assert_unauthorized_websocket_error(err: anyhow::Error) {
    let Some(websocket_error) = err.downcast_ref::<Error>() else {
        panic!("websocket rejection should be a tungstenite error");
    };
    let Error::Http(response) = websocket_error else {
        panic!("expected websocket HTTP rejection, got {websocket_error:?}");
    };
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
}
