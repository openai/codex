use std::net::SocketAddr;

use anyhow::Context;
use anyhow::Result;

use crate::grpc_transport;

pub const DEFAULT_LISTEN_URL: &str = "grpc+stdio://";

#[derive(Debug, Clone, Eq, PartialEq)]
enum ListenTransport {
    GrpcStdio,
    Grpc(SocketAddr),
}

pub(crate) async fn run_transport(listen_url: &str, host: crate::GrpcCodeModeHost) -> Result<()> {
    match parse_listen_url(listen_url)? {
        ListenTransport::GrpcStdio => crate::grpc_stdio::run(host).await,
        ListenTransport::Grpc(bind_address) => {
            grpc_transport::run_tcp_listener(bind_address, host).await
        }
    }
}

fn parse_listen_url(listen_url: &str) -> Result<ListenTransport> {
    if matches!(listen_url, "grpc+stdio://" | "stdio" | "stdio://") {
        return Ok(ListenTransport::GrpcStdio);
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
