use std::io;
use std::path::PathBuf;

use codex_builtin_mcps::BuiltinMcpServer;
use codex_rmcp_client::InProcessTransportFactory;
use futures::FutureExt;
use futures::future::BoxFuture;

#[derive(Clone)]
pub(crate) struct BuiltinMcpServerFactory {
    server: BuiltinMcpServer,
    codex_home: PathBuf,
}

impl BuiltinMcpServerFactory {
    pub(crate) fn new(server: BuiltinMcpServer, codex_home: PathBuf) -> Self {
        Self { server, codex_home }
    }
}

impl InProcessTransportFactory for BuiltinMcpServerFactory {
    fn open(&self) -> BoxFuture<'static, io::Result<tokio::io::DuplexStream>> {
        let server = self.server;
        let codex_home = self.codex_home.clone();
        async move {
            let (client_transport, server_transport) = tokio::io::duplex(64 * 1024);
            tokio::spawn(async move {
                if let Err(err) = server.serve(&codex_home, server_transport).await {
                    tracing::warn!(
                        server = server.name(),
                        "built-in MCP server exited: {err:#}"
                    );
                }
            });
            Ok(client_transport)
        }
        .boxed()
    }
}
