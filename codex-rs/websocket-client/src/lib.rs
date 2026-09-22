//! Proxy-aware WebSocket connection setup shared by Codex API clients, reusing the HTTP factory's
//! ChatGPT cookie store for secure handshakes.

mod dialer;

use std::io;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use codex_http_client::BuildCustomCaTransportError;
use codex_http_client::HttpClientFactory;
use codex_http_client::NetworkPermit;
use codex_http_client::NetworkPolicyDenied;
use codex_http_client::OutboundProxyRoute;
use codex_http_client::build_rustls_client_config_with_custom_ca;
use futures::FutureExt;
use futures::Sink;
use futures::Stream;
use futures::future::BoxFuture;
use futures::future::Either;
use rustls::ClientConfig;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream as TungsteniteStream;
use tokio_tungstenite::tungstenite::Error as WebSocketError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::handshake::client::Response;
use tokio_tungstenite::tungstenite::http::Uri;
use tokio_tungstenite::tungstenite::http::header::COOKIE;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

/// Recovers a deterministic policy denial from Tungstenite's I/O error boundary.
/// Callers must preserve this distinction when deciding whether to retry.
pub fn network_policy_denial(error: &WebSocketError) -> Option<NetworkPolicyDenied> {
    match error {
        WebSocketError::Io(error) => error
            .get_ref()?
            .downcast_ref::<NetworkPolicyDenied>()
            .copied(),
        _ => None,
    }
}

/// Connects WebSockets using the outbound proxy policy resolved by application configuration.
///
/// Construct this from the effective [`HttpClientFactory`] rather than selecting proxy behavior at
/// individual call sites. Each connection resolves its destination through that factory before
/// opening a socket.
#[derive(Clone)]
pub struct WebSocketConnector {
    http_client_factory: HttpClientFactory,
    tls_config: Option<Arc<ClientConfig>>,
    tcp_nodelay: TcpNodelay,
}

/// Selects the TLS configuration used for WebSocket connections.
#[derive(Clone)]
pub enum WebSocketTlsMode {
    /// Build an explicit TLS configuration from native roots and configured Codex custom CAs.
    ExplicitCodexTls,
    /// Use an existing Rustls configuration.
    Rustls(Arc<ClientConfig>),
    /// Let Tungstenite build its default TLS configuration when the target requires TLS.
    TungsteniteDefault,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TcpNodelay {
    Default,
    Enabled,
}

impl WebSocketConnector {
    /// Creates a connector using native roots and any configured Codex custom CA bundle.
    pub fn new(
        http_client_factory: &HttpClientFactory,
    ) -> Result<Self, BuildCustomCaTransportError> {
        Self::new_with_tls_mode(http_client_factory, WebSocketTlsMode::ExplicitCodexTls)
    }

    /// Creates a connector with the selected WebSocket TLS configuration.
    ///
    /// With [`WebSocketTlsMode::TungsteniteDefault`], HTTPS proxy connections still build Codex
    /// TLS configuration when they establish their proxy tunnel.
    pub fn new_with_tls_mode(
        http_client_factory: &HttpClientFactory,
        tls_mode: WebSocketTlsMode,
    ) -> Result<Self, BuildCustomCaTransportError> {
        let tls_config = match tls_mode {
            WebSocketTlsMode::ExplicitCodexTls => {
                Some(build_rustls_client_config_with_custom_ca()?)
            }
            WebSocketTlsMode::Rustls(config) => Some(config),
            WebSocketTlsMode::TungsteniteDefault => None,
        };
        Ok(Self {
            http_client_factory: http_client_factory.clone(),
            tls_config,
            tcp_nodelay: TcpNodelay::Default,
        })
    }

    /// Disables Nagle's algorithm for latency-sensitive WebSocket connections.
    pub fn with_tcp_nodelay(mut self) -> Self {
        self.tcp_nodelay = TcpNodelay::Enabled;
        self
    }

    /// Connects a WebSocket after resolving the request destination through the configured proxy
    /// policy.
    pub async fn connect(
        &self,
        request: Request,
        config: WebSocketConfig,
    ) -> Result<(WebSocketConnection, Response), WebSocketError> {
        let route = self
            .http_client_factory
            .resolve_proxy_route_async(request.uri().to_string());
        self.connect_with_route(request, config, route, /*loopback_direct*/ false)
            .await
    }

    /// Connects to a validated loopback destination without consulting proxy settings.
    /// Application destination policy still applies before dialing.
    pub async fn connect_loopback_direct(
        &self,
        request: Request,
        config: WebSocketConfig,
    ) -> Result<(WebSocketConnection, Response), WebSocketError> {
        if !is_loopback_destination(request.uri()) {
            return Err(WebSocketError::Io(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "direct WebSocket connections require a loopback destination",
            )));
        }
        self.connect_with_route(
            request,
            config,
            std::future::ready(Ok(OutboundProxyRoute::Direct)),
            /*loopback_direct*/ true,
        )
        .await
    }

    async fn connect_with_route(
        &self,
        mut request: Request,
        config: WebSocketConfig,
        route: impl std::future::Future<Output = io::Result<OutboundProxyRoute>>,
        loopback_direct: bool,
    ) -> Result<(WebSocketConnection, Response), WebSocketError> {
        let uri = request.uri().clone();
        let url = url::Url::parse(&uri.to_string()).map_err(|_| {
            WebSocketError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid WebSocket URL",
            ))
        })?;
        let permit = self
            .http_client_factory
            .network_policy()
            .acquire(&url)
            .map_err(policy_error)?;
        let connect = async {
            if !request.headers().contains_key(COOKIE)
                && let Some(cookies) = self.http_client_factory.chatgpt_cookie_header(&uri)
            {
                request.headers_mut().insert(COOKIE, cookies);
            }
            let proxy_route = route.await.map_err(WebSocketError::Io)?;
            let result = dialer::connect(
                request,
                config,
                self.tls_config.clone(),
                proxy_route,
                self.tcp_nodelay,
                loopback_direct,
            )
            .boxed()
            .await;
            // Like HTTP responses, rejected upgrades can also refresh infrastructure cookies.
            match &result {
                Ok((_, response)) => self
                    .http_client_factory
                    .store_chatgpt_response_cookies(&uri, response.headers()),
                Err(WebSocketError::Http(response)) => self
                    .http_client_factory
                    .store_chatgpt_response_cookies(&uri, response.headers()),
                Err(_) => {}
            }
            result
        };
        let (inner, response) = permit.run(connect).await.map_err(policy_error)??;
        Ok((WebSocketConnection::new(inner, permit), response))
    }
}

fn is_loopback_destination(uri: &Uri) -> bool {
    let Some(host) = uri.host() else {
        return false;
    };
    let ip_address = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    host.eq_ignore_ascii_case("localhost")
        || ip_address
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

/// An established WebSocket independent of its direct, proxy, and TLS transport layers.
///
/// This implements [`Stream`] and [`Sink`] so protocol clients can process Tungstenite messages
/// without knowing which concrete network stream route selection produced. Independent revocation
/// waits preserve both task wakers when the stream and sink are split.
pub struct WebSocketConnection {
    inner: Option<ConnectionInner>,
    permit: NetworkPermit,
    read_revoked: BoxFuture<'static, ()>,
    write_revoked: BoxFuture<'static, ()>,
    read_terminated: bool,
}

enum ConnectionDirection {
    Read,
    Write,
}

impl std::fmt::Debug for WebSocketConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSocketConnection")
            .finish_non_exhaustive()
    }
}

impl WebSocketConnection {
    fn new(inner: ConnectionInner, permit: NetworkPermit) -> Self {
        let read_permit = permit.clone();
        let write_permit = permit.clone();
        Self {
            inner: Some(inner),
            permit,
            read_revoked: async move { read_permit.revoked().await }.boxed(),
            write_revoked: async move { write_permit.revoked().await }.boxed(),
            read_terminated: false,
        }
    }

    fn poll_policy(
        &mut self,
        context: &mut Context<'_>,
        direction: ConnectionDirection,
    ) -> Result<(), WebSocketError> {
        let revoked = match direction {
            ConnectionDirection::Read => &mut self.read_revoked,
            ConnectionDirection::Write => &mut self.write_revoked,
        };
        if self.permit.check().is_err() || revoked.poll_unpin(context).is_ready() {
            self.inner = None;
            return Err(policy_error(NetworkPolicyDenied::Revoked));
        }
        Ok(())
    }
}

fn policy_error(denied: NetworkPolicyDenied) -> WebSocketError {
    WebSocketError::Io(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        denied,
    ))
}

impl Stream for WebSocketConnection {
    type Item = Result<Message, WebSocketError>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let connection = self.get_mut();
        if connection.read_terminated {
            return Poll::Ready(None);
        }
        if let Err(error) = connection.poll_policy(context, ConnectionDirection::Read) {
            connection.read_terminated = true;
            return Poll::Ready(Some(Err(error)));
        }
        let next = match &mut connection.inner {
            Some(stream) => Pin::new(stream).poll_next(context),
            None => Poll::Ready(None),
        };
        if matches!(&next, Poll::Ready(None)) {
            connection.read_terminated = true;
        }
        next
    }
}

impl Sink<Message> for WebSocketConnection {
    type Error = WebSocketError;

    fn poll_ready(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        let connection = self.get_mut();
        connection.poll_policy(context, ConnectionDirection::Write)?;
        match &mut connection.inner {
            Some(stream) => Pin::new(stream).poll_ready(context),
            None => Poll::Ready(Err(WebSocketError::ConnectionClosed)),
        }
    }

    fn start_send(self: Pin<&mut Self>, message: Message) -> Result<(), Self::Error> {
        let connection = self.get_mut();
        if let Err(error) = connection.permit.check() {
            connection.inner = None;
            return Err(policy_error(error));
        }
        match &mut connection.inner {
            Some(stream) => Pin::new(stream).start_send(message),
            None => Err(WebSocketError::ConnectionClosed),
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        let connection = self.get_mut();
        connection.poll_policy(context, ConnectionDirection::Write)?;
        match &mut connection.inner {
            Some(stream) => Pin::new(stream).poll_flush(context),
            None => Poll::Ready(Err(WebSocketError::ConnectionClosed)),
        }
    }

    fn poll_close(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        let connection = self.get_mut();
        connection.poll_policy(context, ConnectionDirection::Write)?;
        match &mut connection.inner {
            Some(stream) => Pin::new(stream).poll_close(context),
            None => Poll::Ready(Ok(())),
        }
    }
}

pub(crate) type ConnectionInner = Either<
    TungsteniteStream<MaybeTlsStream<TcpStream>>,
    TungsteniteStream<MaybeTlsStream<Box<dyn AsyncIo>>>,
>;

/// Async network I/O carried through optional proxy and target TLS handshakes.
pub(crate) trait AsyncIo: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T> AsyncIo for T where T: AsyncRead + AsyncWrite + Send + Unpin {}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "cookie_tests.rs"]
mod cookie_tests;

#[cfg(test)]
#[path = "network_policy_tests.rs"]
mod network_policy_tests;
