//! Route-aware WebSocket connection setup.

use codex_http_client::OutboundProxyRoute;
use tokio::net::TcpStream;
use tokio_tungstenite::Connector;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::client_async_tls_with_config;
use tokio_tungstenite::connect_async_tls_with_config;
use tokio_tungstenite::proxy::connect_via_proxy;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::error::UrlError;
use tokio_tungstenite::tungstenite::handshake::client::Request;
use tokio_tungstenite::tungstenite::handshake::client::Response;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::proxy::ProxyConfig;

/// Connects a WebSocket using the resolved outbound proxy route.
pub(crate) async fn connect(
    request: Request,
    config: Option<WebSocketConfig>,
    connector: Option<Connector>,
    proxy_route: OutboundProxyRoute,
) -> Result<(WebSocketStream<MaybeTlsStream<TcpStream>>, Response), WsError> {
    match proxy_route {
        OutboundProxyRoute::TransportDefault => {
            connect_async_tls_with_config(
                request, config, false, // Preserve tungstenite's recommended Nagle default.
                connector,
            )
            .await
        }
        OutboundProxyRoute::Direct => {
            let host = request
                .uri()
                .host()
                .ok_or(WsError::Url(UrlError::NoHostName))?;
            let port = websocket_port(&request)?;
            let address = host_port(host, port);
            let stream = TcpStream::connect(address).await.map_err(WsError::Io)?;
            client_async_tls_with_config(request, stream, config, connector).await
        }
        OutboundProxyRoute::Proxy { url } => {
            let proxy = ProxyConfig::parse(&url).map_err(|_| {
                WsError::Url(UrlError::InvalidProxyConfig("<redacted>".to_string()))
            })?;
            let host = request
                .uri()
                .host()
                .ok_or(WsError::Url(UrlError::NoHostName))?;
            let port = websocket_port(&request)?;
            let stream = TcpStream::connect(proxy.authority())
                .await
                .map_err(WsError::Io)?;
            let stream = connect_via_proxy(stream, &proxy, host, port).await?;
            client_async_tls_with_config(request, stream, config, connector).await
        }
    }
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

#[cfg(test)]
#[path = "websocket_connector_tests.rs"]
mod tests;
