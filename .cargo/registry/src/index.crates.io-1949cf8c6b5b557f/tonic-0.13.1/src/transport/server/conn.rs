use std::net::SocketAddr;
use tokio::net::TcpStream;

#[cfg(feature = "_tls-any")]
use std::sync::Arc;
#[cfg(feature = "_tls-any")]
use tokio_rustls::rustls::pki_types::CertificateDer;
#[cfg(feature = "_tls-any")]
use tokio_rustls::server::TlsStream;

/// Trait that connected IO resources implement and use to produce info about the connection.
///
/// The goal for this trait is to allow users to implement
/// custom IO types that can still provide the same connection
/// metadata.
///
/// # Example
///
/// The `ConnectInfo` returned will be accessible through [request extensions][ext]:
///
/// ```
/// use tonic::{Request, transport::server::Connected};
///
/// // A `Stream` that yields connections
/// struct MyConnector {}
///
/// // Return metadata about the connection as `MyConnectInfo`
/// impl Connected for MyConnector {
///     type ConnectInfo = MyConnectInfo;
///
///     fn connect_info(&self) -> Self::ConnectInfo {
///         MyConnectInfo {}
///     }
/// }
///
/// #[derive(Clone)]
/// struct MyConnectInfo {
///     // Metadata about your connection
/// }
///
/// // The connect info can be accessed through request extensions:
/// # fn foo(request: Request<()>) {
/// let connect_info: &MyConnectInfo = request
///     .extensions()
///     .get::<MyConnectInfo>()
///     .expect("bug in tonic");
/// # }
/// ```
///
/// [ext]: crate::Request::extensions
pub trait Connected {
    /// The connection info type the IO resources generates.
    // all these bounds are necessary to set this as a request extension
    type ConnectInfo: Clone + Send + Sync + 'static;

    /// Create type holding information about the connection.
    fn connect_info(&self) -> Self::ConnectInfo;
}

/// Connection info for standard TCP streams.
///
/// This type will be accessible through [request extensions][ext] if you're using the default
/// non-TLS connector.
///
/// See [`Connected`] for more details.
///
/// [ext]: crate::Request::extensions
#[derive(Debug, Clone)]
pub struct TcpConnectInfo {
    /// Returns the local address of this connection.
    pub local_addr: Option<SocketAddr>,
    /// Returns the remote (peer) address of this connection.
    pub remote_addr: Option<SocketAddr>,
}

impl TcpConnectInfo {
    /// Return the local address the IO resource is connected.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }

    /// Return the remote address the IO resource is connected too.
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote_addr
    }
}

impl Connected for TcpStream {
    type ConnectInfo = TcpConnectInfo;

    fn connect_info(&self) -> Self::ConnectInfo {
        TcpConnectInfo {
            local_addr: self.local_addr().ok(),
            remote_addr: self.peer_addr().ok(),
        }
    }
}

impl Connected for tokio::io::DuplexStream {
    type ConnectInfo = ();

    fn connect_info(&self) -> Self::ConnectInfo {}
}

#[cfg(feature = "_tls-any")]
impl<T> Connected for TlsStream<T>
where
    T: Connected,
{
    type ConnectInfo = TlsConnectInfo<T::ConnectInfo>;

    fn connect_info(&self) -> Self::ConnectInfo {
        let (inner, session) = self.get_ref();
        let inner = inner.connect_info();

        let certs = session
            .peer_certificates()
            .map(|certs| certs.to_owned().into());

        TlsConnectInfo { inner, certs }
    }
}

/// Connection info for TLS streams.
///
/// This type will be accessible through [request extensions][ext] if you're using a TLS connector.
///
/// See [`Connected`] for more details.
///
/// [ext]: crate::Request::extensions
#[cfg(feature = "_tls-any")]
#[derive(Debug, Clone)]
pub struct TlsConnectInfo<T> {
    inner: T,
    certs: Option<Arc<Vec<CertificateDer<'static>>>>,
}

#[cfg(feature = "_tls-any")]
impl<T> TlsConnectInfo<T> {
    /// Get a reference to the underlying connection info.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the underlying connection info.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Return the set of connected peer TLS certificates.
    pub fn peer_certs(&self) -> Option<Arc<Vec<CertificateDer<'static>>>> {
        self.certs.clone()
    }
}
