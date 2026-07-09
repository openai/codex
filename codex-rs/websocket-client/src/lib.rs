//! Proxy-aware WebSocket connection setup shared by Codex API clients.

mod dialer;

use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use codex_http_client::BuildCustomCaTransportError;
use codex_http_client::HttpClientFactory;
use codex_http_client::build_rustls_client_config_with_custom_ca;
use futures::Sink;
use futures::Stream;
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
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

/// Connects WebSockets using the outbound proxy policy resolved by application configuration.
///
/// Construct this from the effective [`HttpClientFactory`] rather than selecting proxy behavior at
/// individual call sites. Each connection resolves its destination through that factory before
/// opening a socket.
#[derive(Clone)]
pub struct WebSocketConnector {
    http_client_factory: HttpClientFactory,
    tls_config: Arc<ClientConfig>,
}

impl WebSocketConnector {
    /// Creates a connector using native roots and any configured Codex custom CA bundle.
    pub fn new(
        http_client_factory: &HttpClientFactory,
    ) -> Result<Self, BuildCustomCaTransportError> {
        Ok(Self {
            http_client_factory: http_client_factory.clone(),
            tls_config: build_rustls_client_config_with_custom_ca()?,
        })
    }

    /// Connects a WebSocket after resolving the request destination through the configured proxy
    /// policy.
    pub async fn connect(
        &self,
        request: Request,
        config: WebSocketConfig,
    ) -> Result<(WebSocketConnection, Response), WebSocketError> {
        let proxy_route = self
            .http_client_factory
            .resolve_proxy_route(&request.uri().to_string());
        let (inner, response) =
            dialer::connect(request, config, Arc::clone(&self.tls_config), proxy_route).await?;
        Ok((WebSocketConnection { inner }, response))
    }
}

/// An established WebSocket independent of its direct, proxy, and TLS transport layers.
///
/// This implements [`Stream`] and [`Sink`] so protocol clients can process Tungstenite messages
/// without knowing which concrete network stream route selection produced.
pub struct WebSocketConnection {
    inner: ConnectionInner,
}

impl Stream for WebSocketConnection {
    type Item = Result<Message, WebSocketError>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut self.get_mut().inner {
            ConnectionInner::TransportDefault(stream) => Pin::new(stream).poll_next(context),
            ConnectionInner::Routed(stream) => Pin::new(stream).poll_next(context),
        }
    }
}

impl Sink<Message> for WebSocketConnection {
    type Error = WebSocketError;

    fn poll_ready(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        match &mut self.get_mut().inner {
            ConnectionInner::TransportDefault(stream) => Pin::new(stream).poll_ready(context),
            ConnectionInner::Routed(stream) => Pin::new(stream).poll_ready(context),
        }
    }

    fn start_send(self: Pin<&mut Self>, message: Message) -> Result<(), Self::Error> {
        match &mut self.get_mut().inner {
            ConnectionInner::TransportDefault(stream) => Pin::new(stream).start_send(message),
            ConnectionInner::Routed(stream) => Pin::new(stream).start_send(message),
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        match &mut self.get_mut().inner {
            ConnectionInner::TransportDefault(stream) => Pin::new(stream).poll_flush(context),
            ConnectionInner::Routed(stream) => Pin::new(stream).poll_flush(context),
        }
    }

    fn poll_close(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        match &mut self.get_mut().inner {
            ConnectionInner::TransportDefault(stream) => Pin::new(stream).poll_close(context),
            ConnectionInner::Routed(stream) => Pin::new(stream).poll_close(context),
        }
    }
}

pub(crate) enum ConnectionInner {
    TransportDefault(TungsteniteStream<MaybeTlsStream<TcpStream>>),
    Routed(TungsteniteStream<MaybeTlsStream<Box<dyn AsyncIo>>>),
}

/// Async network I/O carried through optional proxy and target TLS handshakes.
pub(crate) trait AsyncIo: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T> AsyncIo for T where T: AsyncRead + AsyncWrite + Send + Unpin {}
