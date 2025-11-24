use std::{
    net::{SocketAddr, TcpListener as StdTcpListener},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use socket2::TcpKeepalive;
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::{wrappers::TcpListenerStream, Stream};
use tracing::warn;

/// Binds a socket address for a [Router](super::Router)
///
/// An incoming stream, usable with [Router::serve_with_incoming](super::Router::serve_with_incoming),
/// of `AsyncRead + AsyncWrite` that communicate with clients that connect to a socket address.
#[derive(Debug)]
pub struct TcpIncoming {
    inner: TcpListenerStream,
    nodelay: Option<bool>,
    keepalive: Option<TcpKeepalive>,
}

impl TcpIncoming {
    /// Creates an instance by binding (opening) the specified socket address.
    ///
    /// Returns a TcpIncoming if the socket address was successfully bound.
    ///
    /// # Examples
    /// ```no_run
    /// # use tower_service::Service;
    /// # use http::{request::Request, response::Response};
    /// # use tonic::{body::Body, server::NamedService, transport::{Server, server::TcpIncoming}};
    /// # use core::convert::Infallible;
    /// # use std::error::Error;
    /// # fn main() { }  // Cannot have type parameters, hence instead define:
    /// # fn run<S>(some_service: S) -> Result<(), Box<dyn Error + Send + Sync>>
    /// # where
    /// #   S: Service<Request<Body>, Response = Response<Body>, Error = Infallible> + NamedService + Clone + Send + Sync + 'static,
    /// #   S::Future: Send + 'static,
    /// # {
    /// // Find a free port
    /// let mut port = 1322;
    /// let tinc = loop {
    ///    let addr = format!("127.0.0.1:{}", port).parse().unwrap();
    ///    match TcpIncoming::bind(addr) {
    ///       Ok(t) => break t,
    ///       Err(_) => port += 1
    ///    }
    /// };
    /// Server::builder()
    ///    .add_service(some_service)
    ///    .serve_with_incoming(tinc);
    /// # Ok(())
    /// # }
    pub fn bind(addr: SocketAddr) -> std::io::Result<Self> {
        let std_listener = StdTcpListener::bind(addr)?;
        std_listener.set_nonblocking(true)?;

        Ok(TcpListener::from_std(std_listener)?.into())
    }

    /// Sets the `TCP_NODELAY` option on the accepted connection.
    pub fn with_nodelay(self, nodelay: Option<bool>) -> Self {
        Self { nodelay, ..self }
    }

    /// Sets the `TCP_KEEPALIVE` option on the accepted connection.
    pub fn with_keepalive(self, keepalive: Option<Duration>) -> Self {
        let keepalive = keepalive.map(|t| TcpKeepalive::new().with_time(t));
        Self { keepalive, ..self }
    }

    /// Returns the local address that this tcp incoming is bound to.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.inner.as_ref().local_addr()
    }
}

impl From<TcpListener> for TcpIncoming {
    fn from(listener: TcpListener) -> Self {
        Self {
            inner: TcpListenerStream::new(listener),
            nodelay: None,
            keepalive: None,
        }
    }
}

impl Stream for TcpIncoming {
    type Item = std::io::Result<TcpStream>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let polled = Pin::new(&mut self.inner).poll_next(cx);

        if let Poll::Ready(Some(Ok(stream))) = &polled {
            set_accepted_socket_options(stream, self.nodelay, &self.keepalive);
        }

        polled
    }
}

// Consistent with hyper-0.14, this function does not return an error.
fn set_accepted_socket_options(
    stream: &TcpStream,
    nodelay: Option<bool>,
    keepalive: &Option<TcpKeepalive>,
) {
    if let Some(nodelay) = nodelay {
        if let Err(e) = stream.set_nodelay(nodelay) {
            warn!("error trying to set TCP_NODELAY: {e}");
        }
    }

    if let Some(keepalive) = keepalive {
        let sock_ref = socket2::SockRef::from(&stream);
        if let Err(e) = sock_ref.set_tcp_keepalive(keepalive) {
            warn!("error trying to set TCP_KEEPALIVE: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::transport::server::TcpIncoming;
    #[tokio::test]
    async fn one_tcpincoming_at_a_time() {
        let addr = "127.0.0.1:1322".parse().unwrap();
        {
            let _t1 = TcpIncoming::bind(addr).unwrap();
            let _t2 = TcpIncoming::bind(addr).unwrap_err();
        }
        let _t3 = TcpIncoming::bind(addr).unwrap();
    }
}
