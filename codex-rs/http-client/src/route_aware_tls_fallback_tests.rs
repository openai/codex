use std::io;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use bytes::Bytes;
use futures::stream;
use pretty_assertions::assert_eq;
use rcgen::CertifiedKey;
use rcgen::generate_simple_self_signed;
use rustls_pki_types::PrivateKeyDer;

use super::ClientRouteClass;
use super::HttpClient;
use super::HttpClientFactory;
use super::Method;
use super::OutboundProxyPolicy;
use super::OutboundProxyRoute;
use super::RouteAwareClientPool;
use super::RouteAwareRequestError;
use super::SelectedTlsBackend;
use crate::tls_backend_fallback::should_retry_with_rustls;

const PROTOCOL_VERSION_TLS_ALERT: &[u8] = &[21, 3, 3, 0, 2, 2, 70];

type SuccessfulTlsFallbackServer = (String, HttpClient, mpsc::Receiver<io::Result<Vec<String>>>);

#[tokio::test]
async fn default_pool_does_not_retry_a_native_tls_protocol_failure() {
    let (url, attempts, stop_server) =
        spawn_protocol_version_rejection_server(/*maximum_attempts*/ 2)
            .expect("TLS rejection server should start");
    let pool = RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Other,
    );
    let request = reqwest::Request::new(
        Method::POST,
        reqwest::Url::parse(&url).expect("valid HTTPS URL"),
    );

    let error = pool
        .send_with_resolver(request, |_| async { Ok(OutboundProxyRoute::Direct) })
        .await
        .expect_err("ordinary traffic should preserve the native TLS failure");

    let _ = stop_server.send(());
    assert!(error.is_connect());
    assert_eq!(
        attempts
            .recv()
            .expect("TLS server should finish")
            .expect("TLS server should reject one connection"),
        1
    );
}

#[tokio::test]
async fn retries_a_native_tls_protocol_failure_once_with_rustls() {
    let (url, attempts, stop_server) =
        spawn_protocol_version_rejection_server(/*maximum_attempts*/ 2)
            .expect("TLS rejection server should start");
    let pool = RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Other,
    )
    .with_tls_backend_fallback();
    let destination = reqwest::Url::parse(&url).expect("valid HTTPS URL");
    let mut request = reqwest::Request::new(Method::POST, destination.clone());
    *request.body_mut() = Some(Bytes::from_static(b"mcp-initialize").into());

    let error = pool
        .send_with_resolver(request, |_| async { Ok(OutboundProxyRoute::Direct) })
        .await
        .expect_err("both TLS handshakes should be rejected");

    let _ = stop_server.send(());
    assert!(error.is_connect());
    assert_eq!(
        attempts
            .recv()
            .expect("TLS server should finish")
            .expect("TLS server should reject both connections"),
        2,
        "native TLS protocol failure should retry with rustls: {error:?}"
    );
    let rustls_clients = pool.rustls_clients.as_ref().expect("TLS fallback cache");
    assert_eq!(
        (
            rustls_clients.requires_rustls(&destination, &OutboundProxyRoute::Direct),
            rustls_clients
                .client_for_route(&OutboundProxyRoute::Direct)
                .is_some(),
        ),
        (false, false)
    );
}

#[tokio::test]
async fn retries_a_native_tls_failure_after_another_request_caches_rustls() {
    let (url, attempts, stop_server) =
        spawn_protocol_version_rejection_server(/*maximum_attempts*/ 2)
            .expect("TLS rejection server should start");
    let pool = RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Other,
    )
    .with_tls_backend_fallback();
    let destination = reqwest::Url::parse(&url).expect("valid HTTPS URL");
    let (route, native_client, selected_tls_backend) = pool
        .client_for_url_with_resolver(destination.as_str(), |_| async {
            Ok(OutboundProxyRoute::Direct)
        })
        .await
        .expect("native TLS client should resolve");
    assert_eq!(selected_tls_backend, SelectedTlsBackend::TransportDefault);

    let request = reqwest::Request::new(Method::POST, destination.clone());
    let replay = request.try_clone().expect("request should be replayable");
    let error = native_client
        .execute_without_request_logging(request)
        .await
        .expect_err("native TLS handshake should fail");
    let rustls_client = pool
        .rustls_client_for_route(&route)
        .expect("rustls fallback client should build");
    pool.rustls_clients
        .as_ref()
        .expect("TLS fallback cache")
        .remember(&destination, &route, rustls_client);

    let error = pool
        .retry_with_rustls(
            &destination,
            &route,
            selected_tls_backend,
            Some(&replay),
            error,
            /*timeout_deadline*/ None,
        )
        .await
        .expect_err("both TLS handshakes should be rejected");

    let _ = stop_server.send(());
    assert!(error.is_connect());
    assert_eq!(
        attempts
            .recv()
            .expect("TLS server should finish")
            .expect("TLS server should reject both connections"),
        2,
        "an in-flight native TLS failure should retry despite a concurrent cache update"
    );
}

#[tokio::test]
async fn does_not_retry_a_cached_rustls_tls_protocol_failure() {
    let (url, attempts, stop_server) =
        spawn_protocol_version_rejection_server(/*maximum_attempts*/ 2)
            .expect("TLS rejection server should start");
    let pool = RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Other,
    )
    .with_tls_backend_fallback();
    let destination = reqwest::Url::parse(&url).expect("valid HTTPS URL");
    let rustls_client = pool
        .rustls_client_for_route(&OutboundProxyRoute::Direct)
        .expect("rustls fallback client should build");
    pool.rustls_clients
        .as_ref()
        .expect("TLS fallback cache")
        .remember(&destination, &OutboundProxyRoute::Direct, rustls_client);
    let request = reqwest::Request::new(Method::POST, destination);

    let error = pool
        .send_with_resolver(request, |_| async { Ok(OutboundProxyRoute::Direct) })
        .await
        .expect_err("cached rustls TLS handshake should fail");

    let _ = stop_server.send(());
    assert!(error.is_connect());
    assert_eq!(
        attempts
            .recv()
            .expect("TLS server should finish")
            .expect("TLS server should reject one connection"),
        1,
        "a cached rustls client must not retry against itself"
    );
}

#[tokio::test]
async fn successful_rustls_fallback_replays_the_request_and_reuses_the_destination() {
    let (url, trusted_rustls_client, observed_requests) =
        spawn_successful_tls_fallback_server().expect("TLS fallback server should start");
    let pool = RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Other,
    )
    .with_tls_backend_fallback();
    let destination = reqwest::Url::parse(&url).expect("valid HTTPS URL");
    let existing_destination =
        reqwest::Url::parse("https://another-mcp.example.com/mcp").expect("valid HTTPS URL");
    let rustls_clients = pool.rustls_clients.as_ref().expect("TLS fallback cache");

    // Another destination may already have established a trusted rustls client on this route.
    // The test destination must still attempt native TLS before reusing that route-level client.
    rustls_clients.remember(
        &existing_destination,
        &OutboundProxyRoute::Direct,
        trusted_rustls_client,
    );
    assert!(!rustls_clients.requires_rustls(&destination, &OutboundProxyRoute::Direct));

    let mut initialize_request = reqwest::Request::new(Method::POST, destination.clone());
    *initialize_request.body_mut() = Some(Bytes::from_static(b"mcp-initialize").into());
    let initialize_response = pool
        .send_with_resolver(initialize_request, |_| async {
            Ok(OutboundProxyRoute::Direct)
        })
        .await
        .expect("native TLS protocol failure should recover with rustls");

    assert_eq!(
        initialize_response
            .text()
            .await
            .expect("fallback response body should be readable"),
        "ok"
    );
    assert!(rustls_clients.requires_rustls(&destination, &OutboundProxyRoute::Direct));

    let mut tool_request = reqwest::Request::new(Method::POST, destination);
    *tool_request.body_mut() = Some(Bytes::from_static(b"mcp-tool-call").into());
    let tool_response = pool
        .send_with_resolver(tool_request, |_| async { Ok(OutboundProxyRoute::Direct) })
        .await
        .expect("remembered destination should reuse rustls without another native TLS attempt");

    assert_eq!(
        tool_response
            .text()
            .await
            .expect("cached rustls response body should be readable"),
        "ok"
    );
    assert_eq!(
        observed_requests
            .recv_timeout(Duration::from_secs(5))
            .expect("TLS fallback server should finish")
            .expect("TLS fallback server should capture both requests"),
        vec!["mcp-initialize".to_string(), "mcp-tool-call".to_string()]
    );
}

#[tokio::test]
async fn retries_a_tls_protocol_failure_when_request_url_contains_certificate_markers() {
    let (url, attempts, stop_server) =
        spawn_protocol_version_rejection_server(/*maximum_attempts*/ 2)
            .expect("TLS rejection server should start");
    let pool = RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Other,
    )
    .with_tls_backend_fallback();
    let mut destination = reqwest::Url::parse(&url).expect("valid HTTPS URL");
    destination.set_path("/certificate/hostname/expired/revoked/mcp");
    let request = reqwest::Request::new(Method::POST, destination);

    let error = pool
        .send_with_resolver(request, |_| async { Ok(OutboundProxyRoute::Direct) })
        .await
        .expect_err("both TLS handshakes should be rejected");

    let _ = stop_server.send(());
    assert!(error.is_connect());
    assert_eq!(
        attempts
            .recv()
            .expect("TLS server should finish")
            .expect("TLS server should reject both connections"),
        2,
        "request URL text should not prevent TLS backend fallback: {error:?}"
    );
}

#[tokio::test]
async fn does_not_retry_a_non_replayable_streaming_request() {
    let (url, attempts, stop_server) =
        spawn_protocol_version_rejection_server(/*maximum_attempts*/ 2)
            .expect("TLS rejection server should start");
    let pool = RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Other,
    )
    .with_tls_backend_fallback();
    let mut request = reqwest::Request::new(
        Method::POST,
        reqwest::Url::parse(&url).expect("valid HTTPS URL"),
    );
    *request.body_mut() = Some(reqwest::Body::wrap_stream(stream::iter(vec![Ok::<
        _,
        io::Error,
    >(
        Bytes::from_static(b"streaming body"),
    )])));

    let error = pool
        .send_with_resolver(request, |_| async { Ok(OutboundProxyRoute::Direct) })
        .await
        .expect_err("non-replayable request should preserve the native TLS failure");

    let _ = stop_server.send(());
    assert!(error.is_connect());
    let RouteAwareRequestError::Request(ref request_error) = error else {
        panic!("expected native TLS request error, got {error:?}");
    };
    assert!(
        should_retry_with_rustls(request_error),
        "native TLS protocol failure should be retryable: {request_error:?}"
    );
    assert_eq!(
        attempts
            .recv()
            .expect("TLS server should finish")
            .expect("TLS server should reject one connection"),
        1
    );
}

fn spawn_successful_tls_fallback_server() -> io::Result<SuccessfulTlsFallbackServer> {
    codex_utils_rustls_provider::ensure_rustls_crypto_provider();
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["127.0.0.1".to_string()]).map_err(io::Error::other)?;
    let certificate = cert.der().clone();
    let private_key = PrivateKeyDer::from(signing_key);
    let tls_config = Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate.clone()], private_key)
            .map_err(io::Error::other)?,
    );
    let trusted_rustls_client = HttpClient::new(
        reqwest::Client::builder()
            .use_rustls_tls()
            .add_root_certificate(
                reqwest::Certificate::from_der(certificate.as_ref()).map_err(io::Error::other)?,
            )
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(io::Error::other)?,
    );

    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let address = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    let (requests_tx, requests_rx) = mpsc::channel();

    thread::spawn(move || {
        let result = (|| -> io::Result<Vec<String>> {
            let mut observed_requests = Vec::new();
            for connection_index in 0..3 {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            if Instant::now() >= deadline {
                                return Err(io::Error::new(
                                    io::ErrorKind::TimedOut,
                                    "timed out waiting for the TLS fallback client",
                                ));
                            }
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => return Err(error),
                    }
                };
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                stream.set_write_timeout(Some(Duration::from_secs(5)))?;

                if connection_index == 0 {
                    let mut client_hello = [0_u8; 2_048];
                    if stream.read(&mut client_hello)? == 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "native TLS peer closed before sending a ClientHello",
                        ));
                    }
                    stream.write_all(PROTOCOL_VERSION_TLS_ALERT)?;
                    stream.flush()?;
                    continue;
                }

                let connection = rustls::ServerConnection::new(Arc::clone(&tls_config))
                    .map_err(io::Error::other)?;
                let mut tls = rustls::StreamOwned::new(connection, stream);
                let mut request = Vec::new();
                let mut chunk = [0_u8; 1_024];
                let header_end = loop {
                    let bytes_read = tls.read(&mut chunk)?;
                    if bytes_read == 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "rustls peer closed before sending HTTP headers",
                        ));
                    }
                    request.extend_from_slice(&chunk[..bytes_read]);
                    if let Some(header_end) =
                        request.windows(4).position(|window| window == b"\r\n\r\n")
                    {
                        break header_end + 4;
                    }
                };
                let headers =
                    std::str::from_utf8(&request[..header_end]).map_err(io::Error::other)?;
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then_some(value.trim())
                    })
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length")
                    })?
                    .parse::<usize>()
                    .map_err(io::Error::other)?;
                while request.len() < header_end + content_length {
                    let bytes_read = tls.read(&mut chunk)?;
                    if bytes_read == 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "rustls peer closed before sending the HTTP body",
                        ));
                    }
                    request.extend_from_slice(&chunk[..bytes_read]);
                }
                let body = std::str::from_utf8(&request[header_end..header_end + content_length])
                    .map_err(io::Error::other)?;
                observed_requests.push(body.to_string());
                tls.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                )?;
                tls.flush()?;
            }
            Ok(observed_requests)
        })();
        let _ = requests_tx.send(result);
    });

    Ok((
        format!("https://{address}/mcp"),
        trusted_rustls_client,
        requests_rx,
    ))
}

fn spawn_protocol_version_rejection_server(
    maximum_attempts: usize,
) -> io::Result<(String, mpsc::Receiver<io::Result<usize>>, mpsc::Sender<()>)> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let address = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    let (attempts_tx, attempts_rx) = mpsc::channel();
    let (stop_tx, stop_rx) = mpsc::channel();

    thread::spawn(move || {
        let result = (|| -> io::Result<usize> {
            let mut attempts = 0;
            let deadline = Instant::now() + Duration::from_secs(5);
            while attempts < maximum_attempts && Instant::now() < deadline {
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_nonblocking(false)?;
                        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                        let mut client_hello = [0_u8; 2_048];
                        if stream.read(&mut client_hello)? == 0 {
                            return Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "TLS peer closed before sending a ClientHello",
                            ));
                        }
                        stream.write_all(PROTOCOL_VERSION_TLS_ALERT)?;
                        attempts += 1;
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => return Err(error),
                }
            }
            Ok(attempts)
        })();
        let _ = attempts_tx.send(result);
    });

    Ok((format!("https://{address}/mcp"), attempts_rx, stop_tx))
}
