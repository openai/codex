use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use bytes::Bytes;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

use super::Forwarder;
use crate::ExecServerTelemetry;
use crate::noise_relay::stream_handler::NoiseStreamConnection;

#[tokio::test]
async fn transport_disconnect_cancels_an_unfinished_websocket_handshake() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let forwarder = Forwarder::new(
        format!("ws://{}", listener.local_addr()?),
        &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ExecServerTelemetry::default(),
    )?;
    let (_incoming, incoming_rx) = mpsc::channel(1);
    let (outgoing_tx, _outgoing) = mpsc::channel(1);
    let (disconnected, disconnected_rx) = watch::channel(false);
    let connection = NoiseStreamConnection {
        incoming_rx,
        outgoing_tx,
        disconnected_rx,
        writer_task: tokio::spawn(async {}),
    };
    let task = tokio::spawn(forwarder.run_connection(connection));
    let deadline = Duration::from_secs(5);
    let (mut socket, _) = timeout(deadline, listener.accept()).await??;
    disconnected.send(true)?;
    timeout(deadline, task).await??;
    timeout(deadline, socket.read_to_end(&mut Vec::new())).await??;
    Ok(())
}

#[tokio::test]
async fn destination_close_is_acknowledged() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let forwarder = Forwarder::new(
        format!("ws://{}", listener.local_addr()?),
        &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ExecServerTelemetry::default(),
    )?;
    let (_incoming, incoming_rx) = mpsc::channel(1);
    let (outgoing_tx, mut outgoing) = mpsc::channel(1);
    let (_disconnected, disconnected_rx) = watch::channel(false);
    let task = tokio::spawn(forwarder.run_connection(NoiseStreamConnection {
        incoming_rx,
        outgoing_tx,
        disconnected_rx,
        writer_task: tokio::spawn(async {}),
    }));
    let deadline = Duration::from_secs(5);
    let (socket, _) = timeout(deadline, listener.accept()).await??;
    let mut socket = timeout(deadline, accept_async(socket)).await??;
    let response = Bytes::from_static(b"final response");
    socket.send(Message::Binary(response.clone())).await?;
    let close = CloseFrame {
        code: CloseCode::Normal,
        reason: "finished".into(),
    };
    socket.close(Some(close.clone())).await?;

    let reply = timeout(deadline, socket.next())
        .await?
        .context("destination should receive the Close acknowledgement")??;
    assert_eq!(reply, Message::Close(Some(close)));
    timeout(deadline, task).await??;
    assert_eq!(outgoing.recv().await, Some(response));
    assert_eq!(outgoing.recv().await, None);
    Ok(())
}
