//! Serves one HTTP/2 gRPC connection on stdin/stdout. Stdout contains only protocol
//! bytes, and closing the connection stops the host rather than accepting another client.

use std::io;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use anyhow::Context as _;
use anyhow::Result;
use codex_code_mode_protocol::grpc::code_mode_host_server::CodeModeHostServer;
use codex_code_mode_protocol::host::MAX_FRAME_BYTES;
use futures::StreamExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::ReadBuf;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;
use tonic::transport::server::Connected;

use crate::GrpcCodeModeHost;

pub(crate) async fn run() -> Result<()> {
    let closed = CancellationToken::new();
    let connection = StdioConnection {
        io: tokio::io::join(tokio::io::stdin(), tokio::io::stdout()),
        closed: closed.clone(),
    };
    // Keep the incoming stream open while the single connection is being served.
    let incoming = futures::stream::once(async { Ok::<_, io::Error>(connection) })
        .chain(futures::stream::pending());
    let server = Server::builder()
        .add_service(
            CodeModeHostServer::new(GrpcCodeModeHost::new())
                .max_decoding_message_size(MAX_FRAME_BYTES)
                .max_encoding_message_size(MAX_FRAME_BYTES),
        )
        .serve_with_incoming(incoming);
    tokio::select! {
        result = server => result.context("code-mode gRPC stdio connection failed"),
        _ = closed.cancelled() => Ok(()),
    }
}

struct StdioConnection {
    io: tokio::io::Join<tokio::io::Stdin, tokio::io::Stdout>,
    closed: CancellationToken,
}

impl Connected for StdioConnection {
    type ConnectInfo = ();

    fn connect_info(&self) -> Self::ConnectInfo {}
}

impl AsyncRead for StdioConnection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_read(cx, buf)
    }
}

impl AsyncWrite for StdioConnection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_shutdown(cx)
    }
}

impl Drop for StdioConnection {
    fn drop(&mut self) {
        self.closed.cancel();
    }
}
