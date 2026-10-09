use std::net::SocketAddr;

use anyhow::Context;
use anyhow::Result;

use crate::grpc_transport;

/// The default transport retains the standalone host's original stdio behavior.
pub const DEFAULT_LISTEN_URL: &str = "stdio";

#[derive(Debug, Clone, Eq, PartialEq)]
enum ListenTransport {
    Stdio,
    GrpcStdio,
    Grpc(SocketAddr),
}

pub(crate) async fn run_transport(listen_url: &str) -> Result<()> {
    match parse_listen_url(listen_url)? {
        ListenTransport::Stdio => crate::run_stdio().await,
        ListenTransport::GrpcStdio => crate::grpc_stdio::run().await,
        ListenTransport::Grpc(bind_address) => grpc_transport::run_tcp_listener(bind_address).await,
    }
}

fn parse_listen_url(listen_url: &str) -> Result<ListenTransport> {
    if listen_url == "grpc+stdio://" {
        return Ok(ListenTransport::GrpcStdio);
    }
    if matches!(listen_url, "stdio" | "stdio://") {
        return Ok(ListenTransport::Stdio);
    }

    if let Some(socket_addr) = listen_url.strip_prefix("grpc://") {
        return socket_addr
            .parse::<SocketAddr>()
            .map(ListenTransport::Grpc)
            .with_context(|| {
                format!("invalid gRPC --listen URL `{listen_url}`; expected `grpc://IP:PORT`")
            });
    }

    anyhow::bail!(
        "unsupported --listen URL `{listen_url}`; expected `grpc://IP:PORT`, `grpc+stdio://`, `stdio`, or `stdio://`"
    );
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
