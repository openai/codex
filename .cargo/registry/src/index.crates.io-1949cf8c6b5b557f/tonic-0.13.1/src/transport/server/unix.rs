use super::Connected;
use std::sync::Arc;

/// Connection info for Unix domain socket streams.
///
/// This type will be accessible through [request extensions][ext] if you're using
/// a unix stream.
///
/// See [Connected] for more details.
///
/// [ext]: crate::Request::extensions
#[derive(Clone, Debug)]
pub struct UdsConnectInfo {
    /// Peer address. This will be "unnamed" for client unix sockets.
    pub peer_addr: Option<Arc<tokio::net::unix::SocketAddr>>,
    /// Process credentials for the unix socket.
    pub peer_cred: Option<tokio::net::unix::UCred>,
}

impl Connected for tokio::net::UnixStream {
    type ConnectInfo = UdsConnectInfo;

    fn connect_info(&self) -> Self::ConnectInfo {
        UdsConnectInfo {
            peer_addr: self.peer_addr().ok().map(Arc::new),
            peer_cred: self.peer_cred().ok(),
        }
    }
}
