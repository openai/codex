//! End-to-end checks for credential scope and protocol compatibility on CONNECT and SOCKS5.

use crate::CredentialProviderConfig;
use crate::NetworkMode;
use crate::NetworkProxyConfig;
use crate::NetworkProxyConstraints;
use crate::NetworkProxyState;
use crate::build_config_state;
use crate::connection_lifecycle::ProxyListeners;
use crate::runtime::ConfigReloader;
use crate::runtime::ConfigReloaderFuture;
use crate::runtime::ConfigState;
use pretty_assertions::assert_eq;
use rama_core::extensions::Extensions;
use rama_core::extensions::ExtensionsMut;
use rama_core::extensions::ExtensionsRef;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::net::TcpListener as StdTcpListener;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::io::DuplexStream;
use tokio::io::ReadBuf;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time::Duration;
use tokio::time::timeout;

struct TestStream {
    inner: DuplexStream,
    extensions: Extensions,
}

impl TestStream {
    fn new(inner: DuplexStream) -> Self {
        Self {
            inner,
            extensions: Extensions::new(),
        }
    }
}

impl AsyncRead for TestStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for TestStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

impl ExtensionsRef for TestStream {
    fn extensions(&self) -> &Extensions {
        &self.extensions
    }
}

impl ExtensionsMut for TestStream {
    fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }
}

#[tokio::test]
async fn tls_prefix_detection_accumulates_fragmented_reads() {
    let tls_prefix = [0x16, 0x03, 0x03, 0x00, 0x80];
    let (mut writer, reader) = tokio::io::duplex(16);
    let writer_task = tokio::spawn(async move {
        writer.write_all(&tls_prefix[..1]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        writer.write_all(&tls_prefix[1..]).await.unwrap();
    });

    let (protocol, mut stream) = crate::brokered_tunnel::peek_protocol(
        TestStream::new(reader),
        crate::brokered_tunnel::BrokeredProtocols {
            tls: true,
            http: true,
        },
    )
    .await
    .unwrap();
    let mut replayed = [0_u8; 5];
    stream.read_exact(&mut replayed).await.unwrap();

    assert_eq!(protocol, crate::brokered_tunnel::TunnelProtocol::Tls);
    assert_eq!(replayed, tls_prefix);
    writer_task.await.unwrap();
}

const TOKEN: &str = "local_abcdefghijklmnopqrstuvwxyzabcdef";

struct StaticReloader(ConfigState);

impl ConfigReloader for StaticReloader {
    fn source_label(&self) -> String {
        "HTTP tunnel test".to_string()
    }

    fn maybe_reload(&self) -> ConfigReloaderFuture<'_, Option<ConfigState>> {
        Box::pin(async { Ok(None) })
    }

    fn reload_now(&self) -> ConfigReloaderFuture<'_, ConfigState> {
        Box::pin(async { Ok(self.0.clone()) })
    }
}

#[derive(Clone, Copy, Debug)]
enum Transport {
    Connect,
    Socks5,
}

struct TestProxy {
    addr: SocketAddr,
    listeners: ProxyListeners,
    dummy: String,
    transport: Transport,
}

impl Drop for TestProxy {
    fn drop(&mut self) {
        self.listeners.cancel();
    }
}

impl TestProxy {
    fn start(target: SocketAddr, transport: Transport, mode: NetworkMode) -> Self {
        let mut config = NetworkProxyConfig {
            enabled: true,
            mode,
            mitm: true,
            credential_broker: true,
            allow_local_binding: true,
            credential_providers: BTreeMap::from([(
                "local".to_string(),
                CredentialProviderConfig {
                    env: vec!["LOCAL_TOKEN".to_string()],
                    patterns: vec!["^local_[a-z]{32}$".to_string()],
                    url_prefixes: vec![format!("http://{target}/v1")],
                    ..CredentialProviderConfig::default()
                },
            )]),
            ..NetworkProxyConfig::default()
        };
        config.set_allowed_domains(vec!["127.0.0.1".to_string()]);
        let config_state = build_config_state(
            config,
            NetworkProxyConstraints::default(),
            crate::Platform::native(),
        )
        .unwrap();
        let state = Arc::new(NetworkProxyState::with_reloader(
            config_state.clone(),
            Arc::new(StaticReloader(config_state)),
        ));
        let mut env = HashMap::from([("LOCAL_TOKEN".to_string(), TOKEN.to_string())]);
        state.virtualize_snapshot_credentials(&mut env, /*environment_id*/ None);
        let dummy = env.remove("LOCAL_TOKEN").unwrap();
        assert_ne!(dummy, TOKEN);
        let listener = StdTcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let mut listeners = ProxyListeners::new();
        match transport {
            Transport::Connect => listeners.spawn(move |guard| {
                crate::http_proxy::run_http_proxy_with_std_listener(
                    state, listener, /*policy_decider*/ None, /*environment_id*/ None,
                    guard,
                )
            }),
            Transport::Socks5 => listeners.spawn(move |guard| {
                crate::socks5::run_socks5_with_std_listener(
                    state, listener, /*policy_decider*/ None, /*environment_id*/ None,
                    /*enable_socks5_udp*/ false, guard,
                )
            }),
        }
        Self {
            addr,
            listeners,
            dummy,
            transport,
        }
    }

    async fn connect(&self, target: SocketAddr) -> TcpStream {
        let mut stream = TcpStream::connect(self.addr).await.unwrap();
        match self.transport {
            Transport::Connect => {
                stream
                    .write_all(
                        format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n").as_bytes(),
                    )
                    .await
                    .unwrap();
                assert!(read_headers(&mut stream).await.starts_with("HTTP/1.1 200"));
            }
            Transport::Socks5 => {
                stream.write_all(&[5, 1, 0]).await.unwrap();
                let mut greeting = [0; 2];
                stream.read_exact(&mut greeting).await.unwrap();
                assert_eq!(greeting, [5, 0]);
                let mut request = vec![5, 1, 0, 1, 127, 0, 0, 1];
                request.extend_from_slice(&target.port().to_be_bytes());
                stream.write_all(&request).await.unwrap();
                let mut response = [0; 10];
                stream.read_exact(&mut response).await.unwrap();
                assert_eq!(&response[..4], &[5, 0, 0, 1]);
            }
        }
        stream
    }
}

async fn read_headers(stream: &mut TcpStream) -> String {
    timeout(Duration::from_secs(3), async {
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            assert!(bytes.len() < 16384);
            bytes.push(stream.read_u8().await.unwrap());
        }
        String::from_utf8(bytes).unwrap()
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn plaintext_tunnels_translate_only_the_configured_url_prefix() {
    for transport in [Transport::Connect, Transport::Socks5] {
        let target = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = target.local_addr().unwrap();
        let proxy = TestProxy::start(addr, transport, NetworkMode::Full);
        let mut client = proxy.connect(addr).await;
        for (method, path, body, expected) in [
            ("GET", "/v1/models", "", TOKEN),
            ("OPTIONS", "*", "", proxy.dummy.as_str()),
            ("POST", "/v1/responses", "request body", TOKEN),
            ("GET", "/v10/models", "", proxy.dummy.as_str()),
            ("GET", "/v1/%2e%2e/private", "", proxy.dummy.as_str()),
        ] {
            client.write_all(format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {}\r\nContent-Length: {}\r\n\r\n{body}", proxy.dummy, body.len()).as_bytes()).await.unwrap();
            let (mut upstream, _) = timeout(Duration::from_secs(3), target.accept())
                .await
                .unwrap()
                .unwrap();
            let request = read_headers(&mut upstream).await;
            assert!(request.starts_with(&format!("{method} {path} HTTP/1.1\r\n")));
            assert!(
                request
                    .lines()
                    .any(|line| line
                        .eq_ignore_ascii_case(&format!("authorization: Bearer {expected}"))),
                "{transport:?}, {path}: {request:?}"
            );
            let mut received_body = vec![0; body.len()];
            timeout(
                Duration::from_secs(3),
                upstream.read_exact(&mut received_body),
            )
            .await
            .unwrap()
            .unwrap();
            assert_eq!(received_body, body.as_bytes());
            upstream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
            assert!(read_headers(&mut client).await.starts_with("HTTP/1.1 200"));
        }
    }
}

#[tokio::test]
async fn plaintext_tunnels_accept_http2_clients() {
    use rama_core::Service;
    use rama_core::error::BoxError;
    use rama_core::service::service_fn;
    use rama_http::Body;
    use rama_http::Request;
    use rama_http::Version;
    use rama_http_backend::client::HttpConnector;
    use rama_net::client::EstablishedClientConnection;
    for transport in [Transport::Connect, Transport::Socks5] {
        let target = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = target.local_addr().unwrap();
        let proxy = TestProxy::start(addr, transport, NetworkMode::Full);
        let stream = Arc::new(tokio::sync::Mutex::new(Some(rama_tcp::TcpStream::from(
            proxy.connect(addr).await,
        ))));
        let (captured_tx, mut captured_rx) = tokio::sync::mpsc::channel(1);
        let target_task = tokio::spawn(async move {
            let (stream, _) = target.accept().await.unwrap();
            rama_http_backend::server::HttpServer::auto(rama_core::rt::Executor::default())
                .service(service_fn(move |request: Request| {
                    let captured_tx = captured_tx.clone();
                    async move {
                        captured_tx
                            .send(request.headers().get("authorization").cloned())
                            .await
                            .unwrap();
                        Ok::<_, std::convert::Infallible>(rama_http::Response::new(Body::empty()))
                    }
                }))
                .serve(rama_tcp::TcpStream::from(stream))
                .await
                .unwrap();
        });
        let connector = HttpConnector::<_, Body>::new(service_fn(move |req: Request| {
            let stream = stream.clone();
            async move {
                Ok::<_, BoxError>(EstablishedClientConnection {
                    input: req,
                    conn: stream.lock().await.take().unwrap(),
                })
            }
        }));
        let request = Request::builder()
            .version(Version::HTTP_2)
            .uri(format!("http://{addr}/v1/models"))
            .header("authorization", format!("Bearer {}", proxy.dummy))
            .body(Body::empty())
            .unwrap();
        let EstablishedClientConnection { input, conn } = connector.serve(request).await.unwrap();
        let response = tokio::spawn(async move { conn.serve(input).await });
        assert_eq!(
            timeout(Duration::from_secs(3), captured_rx.recv())
                .await
                .unwrap()
                .unwrap(),
            Some(rama_http::HeaderValue::from_str(&format!("Bearer {TOKEN}")).unwrap())
        );
        assert_eq!(
            timeout(Duration::from_secs(3), response)
                .await
                .unwrap()
                .unwrap()
                .unwrap()
                .status(),
            rama_http::StatusCode::OK
        );
        target_task.abort();
    }
}

#[tokio::test]
async fn plaintext_tunnels_reject_retargeting_and_nested_connect() {
    for transport in [Transport::Connect, Transport::Socks5] {
        let target = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = target.local_addr().unwrap();
        let proxy = TestProxy::start(addr, transport, NetworkMode::Full);
        for (method, uri, host, status) in [
            ("GET", "/v1".to_string(), "127.0.0.1:1".to_string(), 400),
            (
                "GET",
                "http://127.0.0.1:1/v1".to_string(),
                addr.to_string(),
                400,
            ),
            ("GET", format!("https://{addr}/v1"), addr.to_string(), 400),
            ("CONNECT", addr.to_string(), addr.to_string(), 405),
        ] {
            let mut client = proxy.connect(addr).await;
            client.write_all(format!("{method} {uri} HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n", proxy.dummy).as_bytes()).await.unwrap();
            let response = read_headers(&mut client).await;
            assert!(
                response.starts_with(&format!("HTTP/1.1 {status}")),
                "{transport:?}: {response:?}"
            );
        }
        assert!(
            timeout(Duration::from_millis(50), target.accept())
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn plaintext_tunnels_preserve_server_first_and_client_first_opaque_bytes() {
    for transport in [Transport::Connect, Transport::Socks5] {
        for server_first in [true, false] {
            let target = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
            let addr = target.local_addr().unwrap();
            let proxy = TestProxy::start(addr, transport, NetworkMode::Full);
            let mut client = proxy.connect(addr).await;
            if !server_first {
                client.write_all(b"PING").await.unwrap();
            }
            let (mut upstream, _) = timeout(Duration::from_secs(3), target.accept())
                .await
                .unwrap()
                .unwrap();
            upstream.write_all(b"SSH-2.0-server\r\n").await.unwrap();
            let mut banner = [0; 16];
            timeout(Duration::from_secs(3), client.read_exact(&mut banner))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(&banner, b"SSH-2.0-server\r\n");
            if server_first {
                client.write_all(b"PING").await.unwrap();
            }
            let mut request = [0; 4];
            timeout(Duration::from_secs(3), upstream.read_exact(&mut request))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(&request, b"PING");
        }
    }
}

#[tokio::test]
async fn plaintext_tunnels_preserve_http_upgrades() {
    for (transport, protocol) in [Transport::Connect, Transport::Socks5]
        .into_iter()
        .flat_map(|transport| ["test-echo", "h2c"].map(move |protocol| (transport, protocol)))
    {
        let target = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = target.local_addr().unwrap();
        let proxy = TestProxy::start(addr, transport, NetworkMode::Full);
        let mut client = proxy.connect(addr).await;
        client.write_all(format!("GET /v1 HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {}\r\nConnection: upgrade, HTTP2-Settings, X-Remove\r\nUpgrade: {protocol}\r\nHTTP2-Settings: AAMAAABk\r\nX-Remove: hop-local\r\n\r\n", proxy.dummy).as_bytes()).await.unwrap();
        let (mut upstream, _) = timeout(Duration::from_secs(3), target.accept())
            .await
            .unwrap()
            .unwrap();
        let request = read_headers(&mut upstream).await;
        assert!(
            request
                .lines()
                .any(|line| line.eq_ignore_ascii_case(&format!("Authorization: Bearer {TOKEN}")))
        );
        assert!(
            request
                .lines()
                .any(|line| line.eq_ignore_ascii_case(&format!("Upgrade: {protocol}")))
        );
        assert_eq!(
            request
                .lines()
                .any(|line| line.eq_ignore_ascii_case("HTTP2-Settings: AAMAAABk")),
            protocol == "h2c"
        );
        let connection = if protocol == "h2c" {
            "upgrade, http2-settings"
        } else {
            "upgrade"
        };
        assert!(
            request
                .lines()
                .any(|line| line.eq_ignore_ascii_case(&format!("Connection: {connection}")))
        );
        assert!(!request.to_ascii_lowercase().contains("x-remove"));
        upstream.write_all(format!("HTTP/1.1 101 Switching Protocols\r\nConnection: upgrade\r\nUpgrade: {protocol}\r\n\r\nbanner").as_bytes()).await.unwrap();
        assert!(read_headers(&mut client).await.starts_with("HTTP/1.1 101"));
        let mut banner = [0; 6];
        timeout(Duration::from_secs(3), client.read_exact(&mut banner))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&banner, b"banner");
        client.write_all(b"PING").await.unwrap();
        let mut payload = [0; 4];
        timeout(Duration::from_secs(3), upstream.read_exact(&mut payload))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&payload, b"PING");
    }
}

#[tokio::test]
async fn limited_plaintext_tunnels_allow_reads_but_reject_writes_and_opaque_traffic() {
    for transport in [Transport::Connect, Transport::Socks5] {
        let target = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = target.local_addr().unwrap();
        let proxy = TestProxy::start(addr, transport, NetworkMode::Limited);
        for method in ["GET", "HEAD", "OPTIONS"] {
            let mut client = proxy.connect(addr).await;
            client.write_all(format!("{method} /v1 HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n", proxy.dummy).as_bytes()).await.unwrap();
            let (mut upstream, _) = timeout(Duration::from_secs(3), target.accept())
                .await
                .unwrap()
                .unwrap();
            let request = read_headers(&mut upstream).await;
            assert!(
                request.lines().any(
                    |line| line.eq_ignore_ascii_case(&format!("Authorization: Bearer {TOKEN}"))
                )
            );
            upstream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
            assert!(read_headers(&mut client).await.starts_with("HTTP/1.1 200"));
        }
        for (method, extra_headers) in [
            ("POST", ""),
            ("GET", "Upgrade: test-echo\r\nConnection: upgrade\r\n"),
        ] {
            let mut client = proxy.connect(addr).await;
            client.write_all(format!("{method} /v1 HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {}\r\nContent-Length: 0\r\n{extra_headers}\r\n", proxy.dummy).as_bytes()).await.unwrap();
            assert!(read_headers(&mut client).await.starts_with("HTTP/1.1 403"));
        }
        let mut client = proxy.connect(addr).await;
        client.write_all(b"PING").await.unwrap();
        let mut byte = [0];
        let result = timeout(Duration::from_secs(3), client.read(&mut byte))
            .await
            .unwrap();
        assert!(matches!(result, Ok(0) | Err(_)));
        assert!(
            timeout(Duration::from_millis(50), target.accept())
                .await
                .is_err()
        );
    }
}
