use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::OutboundProxyRoute;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use rcgen::CertifiedKey;
use rcgen::generate_simple_self_signed;
use rustls::ClientConfig;
use rustls::RootCertStore;
use rustls::ServerConfig;
use rustls::pki_types::PrivateKeyDer;
use rustls::pki_types::PrivatePkcs8KeyDer;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::proxy::ProxyScheme;

use super::*;
use crate::AsyncIo;
use crate::WebSocketConnection;
use crate::WebSocketConnector;

#[tokio::test]
async fn public_connector_uses_factory_and_exposes_stream_and_sink() {
    let (target_addr, target_task) = start_plain_echo_websocket_server().await;
    let request = format!("ws://localhost:{}/v1/responses", target_addr.port())
        .into_client_request()
        .expect("websocket request should build");
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
    let connector = WebSocketConnector::new(&factory).expect("connector should build");

    let (mut websocket, _) = connector
        .connect(request, WebSocketConfig::default())
        .await
        .expect("websocket handshake should succeed");
    let expected = Message::Text("hello".into());
    websocket
        .send(expected.clone())
        .await
        .expect("websocket should send");
    let actual = websocket
        .next()
        .await
        .expect("websocket should receive a message")
        .expect("websocket message should be valid");
    assert_eq!(actual, expected);

    target_task.await.expect("target task should finish");
}

#[tokio::test]
async fn direct_route_connects_secure_websocket() {
    let (tls_config, acceptor) = test_tls_configs();
    let (target_addr, target_task) = start_tls_websocket_server(acceptor).await;
    let request = format!("wss://localhost:{}/v1/responses", target_addr.port())
        .into_client_request()
        .expect("websocket request should build");

    let (inner, _) = connect(
        request,
        WebSocketConfig::default(),
        tls_config,
        OutboundProxyRoute::Direct,
    )
    .await
    .expect("direct websocket handshake should succeed");
    drop(WebSocketConnection { inner });

    target_task.await.expect("target task should finish");
}

#[tokio::test]
async fn http_proxy_tunnels_secure_websocket_before_handshake() {
    assert_proxy_tunnels_secure_websocket(/*proxy_tls*/ false).await;
}

#[tokio::test]
async fn https_proxy_tunnels_secure_websocket_before_handshake() {
    assert_proxy_tunnels_secure_websocket(/*proxy_tls*/ true).await;
}

#[test]
fn https_proxy_defaults_to_port_443_and_preserves_explicit_port() {
    let default_port = ProxyEndpoint::parse("https://proxy.example")
        .expect("HTTPS proxy without a port should parse");
    let explicit_port = ProxyEndpoint::parse("https://proxy.example:8443")
        .expect("HTTPS proxy with a port should parse");

    assert_eq!(
        default_port,
        ProxyEndpoint {
            config: ProxyConfig {
                scheme: ProxyScheme::Http,
                host: "proxy.example".to_string(),
                port: 443,
                auth: None,
            },
            tls: true,
        }
    );
    assert_eq!(
        explicit_port,
        ProxyEndpoint {
            config: ProxyConfig {
                scheme: ProxyScheme::Http,
                host: "proxy.example".to_string(),
                port: 8443,
                auth: None,
            },
            tls: true,
        }
    );
}

#[tokio::test(start_paused = true)]
async fn happy_eyeballs_does_not_wait_for_stalled_preferred_family() {
    let stalled = "[2001:db8::1]:443"
        .parse::<SocketAddr>()
        .expect("stalled address should parse");
    let reachable = "127.0.0.1:443"
        .parse::<SocketAddr>()
        .expect("reachable address should parse");

    let connected = tokio::time::timeout(
        Duration::from_secs(1),
        connect_happy_eyeballs(vec![stalled, reachable], |address| async move {
            if address == stalled {
                std::future::pending::<()>().await;
            }
            Ok(address)
        }),
    )
    .await
    .expect("alternate family should start before timeout")
    .expect("alternate family should connect");

    assert_eq!(connected, reachable);
}

async fn start_plain_echo_websocket_server() -> (SocketAddr, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("target listener should bind");
    let address = listener
        .local_addr()
        .expect("target listener should have an address");
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("target should accept");
        let mut websocket = accept_async(stream)
            .await
            .expect("target websocket handshake should succeed");
        let message = websocket
            .next()
            .await
            .expect("target should receive a message")
            .expect("target websocket message should be valid");
        websocket
            .send(message)
            .await
            .expect("target should echo the message");
    });
    (address, task)
}

async fn assert_proxy_tunnels_secure_websocket(proxy_tls: bool) {
    let (tls_config, acceptor) = test_tls_configs();
    let (target_addr, target_task) = start_tls_websocket_server(acceptor.clone()).await;

    let proxy_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("proxy listener should bind");
    let proxy_addr = proxy_listener
        .local_addr()
        .expect("proxy listener should have an address");
    let connect_request = Arc::new(Mutex::new(None));
    let proxy_connect_request = Arc::clone(&connect_request);
    let proxy_task = tokio::spawn(async move {
        let (client, _) = proxy_listener.accept().await.expect("proxy should accept");
        let mut client: Box<dyn AsyncIo> = if proxy_tls {
            Box::new(
                acceptor
                    .accept(client)
                    .await
                    .expect("proxy TLS handshake should succeed"),
            )
        } else {
            Box::new(client)
        };
        let mut request = Vec::new();
        let mut byte = [0_u8; 1];
        while !request.ends_with(b"\r\n\r\n") {
            client
                .read_exact(&mut byte)
                .await
                .expect("proxy should read CONNECT request");
            request.push(byte[0]);
        }
        *proxy_connect_request.lock().await =
            Some(String::from_utf8(request).expect("CONNECT request should contain valid UTF-8"));

        let mut target = tokio::net::TcpStream::connect(target_addr)
            .await
            .expect("proxy should connect to target");
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await
            .expect("proxy should acknowledge CONNECT");
        let _ = tokio::io::copy_bidirectional(&mut client, &mut target).await;
    });

    let target_authority = format!("localhost:{}", target_addr.port());
    let proxy_scheme = if proxy_tls { "https" } else { "http" };
    let request = format!("wss://{target_authority}/v1/responses")
        .into_client_request()
        .expect("websocket request should build");
    let (inner, _) = connect(
        request,
        WebSocketConfig::default(),
        tls_config,
        OutboundProxyRoute::Proxy {
            url: format!("{proxy_scheme}://localhost:{}", proxy_addr.port()),
        },
    )
    .await
    .expect("proxied websocket handshake should succeed");
    drop(WebSocketConnection { inner });

    target_task.await.expect("target task should finish");
    proxy_task.await.expect("proxy task should finish");
    let request = connect_request
        .lock()
        .await
        .clone()
        .expect("proxy should record CONNECT request");
    let expected_request_line = format!("CONNECT {target_authority} HTTP/1.1");
    assert_eq!(request.lines().next(), Some(expected_request_line.as_str()));
}

async fn start_tls_websocket_server(acceptor: TlsAcceptor) -> (SocketAddr, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("target listener should bind");
    let address = listener
        .local_addr()
        .expect("target listener should have an address");
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("target should accept");
        let stream = acceptor
            .accept(stream)
            .await
            .expect("target TLS handshake should succeed");
        let mut websocket = accept_async(stream)
            .await
            .expect("target websocket handshake should succeed");
        let _ = websocket.close(None).await;
    });
    (address, task)
}

fn test_tls_configs() -> (Arc<ClientConfig>, TlsAcceptor) {
    ensure_rustls_crypto_provider();
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()])
            .expect("test certificate should generate");
    let certificate = cert.der().clone();
    let private_key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der()));
    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![certificate.clone()], private_key)
        .expect("test server config should build");

    let mut roots = RootCertStore::empty();
    roots
        .add(certificate)
        .expect("test certificate should be trusted");
    let client_config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    (
        Arc::new(client_config),
        TlsAcceptor::from(Arc::new(server_config)),
    )
}
