use std::io;
use std::io::Write as _;
use std::net::SocketAddr;

use anyhow::Context;
use anyhow::Result;
use codex_code_mode_protocol::grpc::code_mode_host_server::CodeModeHostServer;
use codex_code_mode_protocol::host::MAX_FRAME_BYTES;
use tokio::net::TcpListener;
use tonic::transport::Server;
use tonic::transport::server::TcpIncoming;
use tracing::info;

use crate::GrpcCodeModeHost;

pub(super) async fn run_tcp_listener(bind_address: SocketAddr) -> Result<()> {
    let listener = bind_tcp_listener(bind_address).await?;
    let local_address = listener
        .local_addr()
        .context("failed to read code-mode gRPC listen address")?;
    info!("codex-code-mode-host listening on http://{local_address}");
    println!("http://{local_address}");
    io::stdout()
        .flush()
        .context("failed to publish code-mode gRPC listen address")?;

    Server::builder()
        .add_service(
            CodeModeHostServer::new(GrpcCodeModeHost::new())
                .max_decoding_message_size(MAX_FRAME_BYTES)
                .max_encoding_message_size(MAX_FRAME_BYTES),
        )
        .serve_with_incoming(listener)
        .await
        .context("code-mode gRPC TCP listener failed")
}

pub(super) async fn bind_tcp_listener(bind_address: SocketAddr) -> Result<TcpIncoming> {
    let listener = TcpListener::bind(bind_address)
        .await
        .with_context(|| format!("failed to bind code-mode gRPC host to {bind_address}"))?;
    Ok(TcpIncoming::from(listener).with_nodelay(Some(true)))
}
