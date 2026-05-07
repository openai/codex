use std::io;

use futures::future::BoxFuture;
use tokio::io::DuplexStream;

/// Recreates a fresh in-process MCP byte stream whenever the client needs one.
///
/// Implementations are expected to start the paired server side before
/// returning the client stream. The factory is retained by [`crate::RmcpClient`]
/// so reconnects can rebuild the transport without knowing which built-in
/// server produced it.
pub trait InProcessTransportFactory: Send + Sync {
    fn open(&self) -> BoxFuture<'static, io::Result<DuplexStream>>;
}
