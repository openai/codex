//! Route-aware WebSocket connection setup.

use std::collections::VecDeque;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use codex_http_client::OutboundProxyRoute;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio::time::sleep_until;
use tokio_rustls::TlsConnector;
use tokio_tungstenite::Connector;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::client_async_tls_with_config;
use tokio_tungstenite::connect_async_tls_with_config;
use tokio_tungstenite::proxy::connect_via_proxy;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::error::TlsError;
use tokio_tungstenite::tungstenite::error::UrlError;
use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::handshake::client::Response;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::proxy::ProxyConfig;

const HAPPY_EYEBALLS_DELAY: Duration = Duration::from_millis(250);

/// An established WebSocket whose concrete network stream depends on route selection.
pub(crate) enum ConnectedWebSocket {
    /// A connection established by `tokio-tungstenite`'s default transport.
    TransportDefault(WebSocketStream<MaybeTlsStream<TcpStream>>),
    /// A connection established over a caller-selected direct or proxy stream.
    Routed(WebSocketStream<MaybeTlsStream<Box<dyn AsyncIo>>>),
}

/// Async network I/O that can be carried through a proxy tunnel and target TLS handshake.
pub(crate) trait AsyncIo: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T> AsyncIo for T where T: AsyncRead + AsyncWrite + Send + Unpin {}

struct ProxyEndpoint {
    config: ProxyConfig,
    tls: bool,
}

impl ProxyEndpoint {
    fn parse(url: &str) -> Result<Self, WsError> {
        let mut parsed_url = url::Url::parse(url).map_err(|_| invalid_proxy_config())?;
        let tls = parsed_url.scheme() == "https";
        if tls {
            if parsed_url.port().is_none() {
                parsed_url
                    .set_port(Some(443))
                    .map_err(|_| invalid_proxy_config())?;
            }
            parsed_url
                .set_scheme("http")
                .map_err(|_| invalid_proxy_config())?;
        }
        let config = ProxyConfig::parse(parsed_url.as_str()).map_err(|error| match error {
            WsError::Url(UrlError::UnsupportedProxyScheme) => error,
            _ => invalid_proxy_config(),
        })?;
        Ok(Self { config, tls })
    }
}

/// Connects a WebSocket using the resolved outbound proxy route.
pub(crate) async fn connect(
    request: Request,
    config: Option<WebSocketConfig>,
    tls_config: Arc<ClientConfig>,
    proxy_route: OutboundProxyRoute,
) -> Result<(ConnectedWebSocket, Response), WsError> {
    let stream: Box<dyn AsyncIo> = match proxy_route {
        OutboundProxyRoute::TransportDefault => {
            let (stream, response) = connect_async_tls_with_config(
                request,
                config,
                false, // Preserve tungstenite's recommended Nagle default.
                Some(Connector::Rustls(tls_config)),
            )
            .await?;
            return Ok((ConnectedWebSocket::TransportDefault(stream), response));
        }
        OutboundProxyRoute::Direct => {
            let host = request
                .uri()
                .host()
                .ok_or(WsError::Url(UrlError::NoHostName))?;
            let port = websocket_port(&request)?;
            let address = host_port(host, port);
            Box::new(connect_tcp(address).await.map_err(WsError::Io)?)
        }
        OutboundProxyRoute::Proxy { url } => {
            let proxy = ProxyEndpoint::parse(&url)?;
            let host = request
                .uri()
                .host()
                .ok_or(WsError::Url(UrlError::NoHostName))?;
            let port = websocket_port(&request)?;
            let stream = connect_tcp(proxy.config.authority())
                .await
                .map_err(WsError::Io)?;
            let stream: Box<dyn AsyncIo> = if proxy.tls {
                let server_name = ServerName::try_from(proxy.config.host.clone())
                    .map_err(|_| WsError::Tls(TlsError::InvalidDnsName))?;
                let stream = TlsConnector::from(Arc::clone(&tls_config))
                    .connect(server_name, stream)
                    .await
                    .map_err(WsError::Io)?;
                Box::new(stream)
            } else {
                Box::new(stream)
            };
            connect_via_proxy(stream, &proxy.config, host, port).await?
        }
    };

    let (stream, response) =
        client_async_tls_with_config(request, stream, config, Some(Connector::Rustls(tls_config)))
            .await?;
    Ok((ConnectedWebSocket::Routed(stream), response))
}

fn invalid_proxy_config() -> WsError {
    WsError::Url(UrlError::InvalidProxyConfig("<redacted>".to_string()))
}

fn websocket_port(request: &Request) -> Result<u16, WsError> {
    request
        .uri()
        .port_u16()
        .or_else(|| match request.uri().scheme_str() {
            Some("ws") => Some(80),
            Some("wss") => Some(443),
            _ => None,
        })
        .ok_or(WsError::Url(UrlError::UnsupportedUrlScheme))
}

fn host_port(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// Preserves the Happy Eyeballs behavior of `tokio-tungstenite`'s built-in dialer.
///
/// Explicit routes require a caller-provided TCP stream, so they cannot use that private dialer
/// directly. Keep the same family interleaving and 250 ms attempt delay here to avoid regressing
/// direct or proxy connections when one address family is unreachable.
async fn connect_tcp(address: String) -> io::Result<TcpStream> {
    let mut resolved_addresses = tokio::net::lookup_host(address).await?;
    let Some(first_address) = resolved_addresses.next() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "could not resolve to any address",
        ));
    };

    let first_is_ipv4 = first_address.is_ipv4();
    let mut preferred = VecDeque::new();
    let mut alternate = VecDeque::new();
    for address in resolved_addresses {
        if address.is_ipv4() == first_is_ipv4 {
            preferred.push_back(address);
        } else {
            alternate.push_back(address);
        }
    }

    let mut addresses = VecDeque::<SocketAddr>::new();
    while !preferred.is_empty() || !alternate.is_empty() {
        if let Some(address) = alternate.pop_front() {
            addresses.push_back(address);
        }
        if let Some(address) = preferred.pop_front() {
            addresses.push_back(address);
        }
    }

    let mut attempts = FuturesUnordered::new();
    attempts.push(TcpStream::connect(first_address));
    let mut next_attempt_at = Instant::now() + HAPPY_EYEBALLS_DELAY;
    let mut last_error = None;

    loop {
        if addresses.is_empty() {
            match attempts.next().await {
                Some(Ok(stream)) => return Ok(stream),
                Some(Err(error)) => {
                    if attempts.is_empty() {
                        return Err(error);
                    }
                    last_error = Some(error);
                }
                None => {
                    return Err(last_error.unwrap_or_else(|| {
                        io::Error::other("connection attempts ended without an error")
                    }));
                }
            }
            continue;
        }

        tokio::select! {
            result = attempts.next() => {
                match result {
                    Some(Ok(stream)) => return Ok(stream),
                    Some(Err(error)) => {
                        last_error = Some(error);
                        let address = take_next_address(&mut addresses)?;
                        attempts.push(TcpStream::connect(address));
                        next_attempt_at = Instant::now() + HAPPY_EYEBALLS_DELAY;
                    }
                    None => {
                        let address = take_next_address(&mut addresses)?;
                        attempts.push(TcpStream::connect(address));
                        next_attempt_at = Instant::now() + HAPPY_EYEBALLS_DELAY;
                    }
                }
            }
            _ = sleep_until(next_attempt_at) => {
                let address = take_next_address(&mut addresses)?;
                attempts.push(TcpStream::connect(address));
                next_attempt_at = Instant::now() + HAPPY_EYEBALLS_DELAY;
            }
        }
    }
}

fn take_next_address(addresses: &mut VecDeque<SocketAddr>) -> io::Result<SocketAddr> {
    addresses
        .pop_front()
        .ok_or_else(|| io::Error::other("connection address queue unexpectedly empty"))
}

#[cfg(test)]
#[path = "websocket_connector_tests.rs"]
mod tests;
