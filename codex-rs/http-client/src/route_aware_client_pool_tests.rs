use std::collections::HashMap;
use std::io;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use pretty_assertions::assert_eq;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;

use super::*;
use crate::OutboundProxyPolicy;

#[test]
fn request_builder_debug_redacts_url_secrets() {
    let pool = RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Api,
    );
    let request = pool.get(
        "https://username:password@private.example/secret-path?sig=query-secret#fragment-secret",
    );

    assert_eq!(
        format!("{request:?}"),
        concat!(
            "RouteAwareRequestBuilder { pool: RouteAwareClientPool { ",
            "http_client_factory: HttpClientFactory { outbound_proxy_policy: ReqwestDefault }, ",
            "route_class: Api, .. }, method: Some(GET), ",
            "url: Some(\"<redacted>\"), .. }"
        )
    );
}

#[tokio::test]
async fn forwards_exact_urls_and_caches_clients_by_resolved_route() {
    let pool = RouteAwareClientPool::with_builder(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Api,
        HttpClientBuilder::new(),
    );

    let direct_url = "https://example.com/first?target=direct";
    let same_route_url = "https://example.com/second?target=direct%202";
    let proxy_url = "https://example.com/third?target=proxy";
    let resolver = FakeRouteResolver::new(HashMap::from([
        (direct_url.to_string(), OutboundProxyRoute::Direct),
        (same_route_url.to_string(), OutboundProxyRoute::Direct),
        (
            proxy_url.to_string(),
            OutboundProxyRoute::Proxy {
                url: "http://proxy.example".to_string(),
                no_proxy: None,
            },
        ),
    ]));

    resolve_with(&pool, &resolver, direct_url)
        .await
        .expect("first client should build");
    resolve_with(&pool, &resolver, same_route_url)
        .await
        .expect("second client should reuse the route");
    resolve_with(&pool, &resolver, proxy_url)
        .await
        .expect("proxy client should build separately");

    assert_eq!(pool.clients.lock().expect("client cache lock").len(), 2);
    assert_eq!(
        resolver.observed_urls(),
        vec![
            direct_url.to_string(),
            same_route_url.to_string(),
            proxy_url.to_string(),
        ]
    );
}

#[tokio::test]
async fn reqwest_default_route_preserves_transport_redirects() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("redirect listener should bind");
    let address = listener
        .local_addr()
        .expect("redirect listener should have an address");
    listener
        .set_nonblocking(true)
        .expect("redirect listener should become nonblocking");
    let server = std::thread::spawn(move || {
        let mut request_lines = Vec::new();
        for response in [
            "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
        ] {
            let deadline = Instant::now() + Duration::from_secs(2);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "redirect server should receive the next request"
                        );
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("redirect server should accept: {error}"),
                }
            };
            let mut buffer = [0_u8; 1024];
            let size = stream
                .read(&mut buffer)
                .expect("redirect server should read request");
            let request = String::from_utf8_lossy(&buffer[..size]);
            request_lines.push(
                request
                    .lines()
                    .next()
                    .expect("request should have a request line")
                    .to_string(),
            );
            stream
                .write_all(response.as_bytes())
                .expect("redirect server should write response");
        }
        request_lines
    });
    let pool = RouteAwareClientPool::with_builder(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Api,
        HttpClientBuilder::new(),
    );
    let initial_url = format!("http://{address}/start");
    let request = reqwest::Request::new(
        Method::GET,
        reqwest::Url::parse(&initial_url).expect("request URL should parse"),
    );

    let response = pool
        .send_with_resolver(request, |_| async { Ok(OutboundProxyRoute::Direct) })
        .await
        .expect("default-routed request should follow redirect");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.url().as_str(), format!("http://{address}/final"));
    assert_eq!(
        server.join().expect("redirect server should finish"),
        vec![
            "GET /start HTTP/1.1".to_string(),
            "GET /final HTTP/1.1".to_string(),
        ]
    );
}

#[tokio::test]
async fn no_redirect_pool_returns_redirect_response() {
    for outbound_proxy_policy in [
        OutboundProxyPolicy::ReqwestDefault,
        OutboundProxyPolicy::RespectSystemProxy,
    ] {
        let (address, server) = spawn_response_server(vec![
            "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_string(),
        ]);
        let pool = RouteAwareClientPool::new_without_redirects(
            HttpClientFactory::new(outbound_proxy_policy),
            ClientRouteClass::Api,
        );
        let initial_url = format!("http://{address}/start");
        let request = reqwest::Request::new(
            Method::GET,
            reqwest::Url::parse(&initial_url).expect("request URL should parse"),
        );

        let response = pool
            .send_with_resolver(request, |_| async { Ok(OutboundProxyRoute::Direct) })
            .await
            .expect("no-redirect request should finish");

        assert_eq!(response.status(), StatusCode::FOUND);
        let requests = server.join().expect("redirect server should finish");
        assert_eq!(requests.len(), 1);
        assert!(requests[0].starts_with("GET /start HTTP/1.1\r\n"));
    }
}

#[tokio::test]
async fn bounds_cached_routes_and_rebuilds_an_evicted_route() {
    let pool = RouteAwareClientPool::with_builder(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Api,
        HttpClientBuilder::new(),
    );
    let routes = (0..=MAX_CACHED_ROUTES)
        .map(|index| {
            (
                format!("https://target-{index}.example"),
                OutboundProxyRoute::Proxy {
                    url: format!("http://proxy-{index}.example"),
                    no_proxy: None,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let resolver = FakeRouteResolver::new(routes.clone());

    for request_url in routes.keys() {
        resolve_with(&pool, &resolver, request_url)
            .await
            .expect("client should build");
    }
    let evicted_route = {
        let clients = pool.clients.lock().expect("client cache lock");
        assert_eq!(clients.len(), MAX_CACHED_ROUTES);
        routes
            .iter()
            .find(|(_, route)| !clients.contains_key(*route))
            .map(|(request_url, _)| request_url.clone())
            .expect("one route should have been evicted")
    };

    resolve_with(&pool, &resolver, &evicted_route)
        .await
        .expect("evicted client should rebuild");

    let clients = pool.clients.lock().expect("client cache lock");
    assert_eq!(clients.len(), MAX_CACHED_ROUTES);
    assert!(clients.contains_key(&routes[&evicted_route]));
}

#[tokio::test]
async fn request_timeout_covers_route_selection() {
    let pool = manual_redirect_pool();
    let mut request = reqwest::Request::new(
        Method::GET,
        reqwest::Url::parse("http://route-selection-timeout.test/start")
            .expect("request URL should parse"),
    );
    *request.timeout_mut() = Some(Duration::from_millis(10));
    let resolver_calls = Arc::new(AtomicUsize::new(0));
    let observed_resolver_calls = Arc::clone(&resolver_calls);

    let error = pool
        .send_with_resolver(request, move |_| {
            observed_resolver_calls.fetch_add(1, Ordering::SeqCst);
            async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(OutboundProxyRoute::Direct)
            }
        })
        .await
        .expect_err("request should time out during route selection");

    assert!(matches!(error, RouteAwareRequestError::Timeout));
    assert_eq!(resolver_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn request_timeout_is_shared_across_redirect_hops() {
    let (address, server) = spawn_response_server(vec![
        "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_string(),
    ]);
    let pool = manual_redirect_pool();
    let mut request = reqwest::Request::new(
        Method::GET,
        reqwest::Url::parse(&format!("http://{address}/start")).expect("request URL should parse"),
    );
    *request.timeout_mut() = Some(Duration::from_secs(2));
    let resolver_calls = Arc::new(AtomicUsize::new(0));
    let observed_resolver_calls = Arc::clone(&resolver_calls);

    let error = pool
        .send_with_resolver(request, move |_| {
            let resolver_call = observed_resolver_calls.fetch_add(1, Ordering::SeqCst);
            async move {
                let delay = if resolver_call == 0 {
                    Duration::from_millis(500)
                } else {
                    Duration::from_millis(1_750)
                };
                tokio::time::sleep(delay).await;
                Ok(OutboundProxyRoute::Direct)
            }
        })
        .await
        .expect_err("redirect chain should exceed its shared timeout");

    assert!(matches!(error, RouteAwareRequestError::Timeout));
    assert_eq!(resolver_calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        server.join().expect("redirect server should finish").len(),
        1
    );
}

#[tokio::test]
async fn rejects_replayable_redirect_to_unsupported_scheme() {
    let (address, server) = spawn_response_server(vec![
        "HTTP/1.1 307 Temporary Redirect\r\nLocation: ftp://example.com/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_string(),
    ]);
    let pool = manual_redirect_pool();
    let request = reqwest::Request::new(
        Method::GET,
        reqwest::Url::parse(&format!("http://{address}/start")).expect("request URL should parse"),
    );

    let error = pool
        .send_with_resolver(request, |_| async { Ok(OutboundProxyRoute::Direct) })
        .await
        .expect_err("unsupported redirect scheme should fail");

    assert!(matches!(
        error,
        RouteAwareRequestError::UnsupportedRedirectScheme(scheme) if scheme == "ftp"
    ));
    assert_eq!(
        server.join().expect("redirect server should finish").len(),
        1
    );
}

#[tokio::test]
async fn rejects_redirects_beyond_the_limit() {
    let responses = (0..=MAX_REDIRECTS)
        .map(|redirect| {
            format!(
                "HTTP/1.1 302 Found\r\nLocation: /hop/{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                redirect + 1
            )
        })
        .collect();
    let (address, server) = spawn_response_server(responses);
    let pool = manual_redirect_pool();
    let request = reqwest::Request::new(
        Method::GET,
        reqwest::Url::parse(&format!("http://{address}/start")).expect("request URL should parse"),
    );

    let error = pool
        .send_with_resolver(request, |_| async { Ok(OutboundProxyRoute::Direct) })
        .await
        .expect_err("redirect chain should stop at the limit");
    let requests = server.join().expect("redirect server should finish");

    assert!(matches!(error, RouteAwareRequestError::TooManyRedirects));
    assert_eq!(requests.len(), MAX_REDIRECTS + 1);
    assert_eq!(
        requests.last().and_then(|request| request.lines().next()),
        Some("GET /hop/10 HTTP/1.1")
    );
}

#[tokio::test]
async fn disabled_pool_logging_does_not_expose_request_or_response_data() {
    let (address, server) = spawn_response_server(vec![
        "HTTP/1.1 200 OK\r\nx-sensitive-response: response-secret-value\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
            .to_string(),
    ]);
    let pool = RouteAwareClientPool::with_builder(
        HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        ClientRouteClass::Api,
        HttpClientBuilder::new().without_request_logging(),
    );
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(TestLogWriter {
                buffer: Arc::clone(&buffer),
            })
            .with_filter(
                tracing_subscriber::filter::Targets::new()
                    .with_target("codex_http_client", tracing::Level::TRACE),
            ),
    );
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::debug!(target: "codex_http_client", "log capture sentinel");
    let request_url = format!(
        "http://auth-user:password-secret-value@{address}/token?client_secret=query-secret-value"
    );
    let mut request = reqwest::Request::new(
        Method::POST,
        reqwest::Url::parse(&request_url).expect("request URL should parse"),
    );
    request.headers_mut().insert(
        "x-sensitive-request",
        HeaderValue::from_static("request-header-secret-value"),
    );
    *request.body_mut() = Some("request-body-secret-value".into());
    *request.timeout_mut() = Some(Duration::from_secs(2));

    let response = pool
        .send_with_resolver(request, |_| async { Ok(OutboundProxyRoute::Direct) })
        .await
        .expect("route-aware request should succeed");
    assert_eq!(response.status(), StatusCode::OK);
    server.join().expect("server thread should finish");

    let unresponsive_listener =
        TcpListener::bind(("127.0.0.1", 0)).expect("unresponsive listener should bind");
    let unresponsive_address = unresponsive_listener
        .local_addr()
        .expect("unresponsive listener should have an address");
    let failure_url = format!(
        "http://auth-user:failure-password-secret-value@{unresponsive_address}/token?client_secret=failure-query-secret-value"
    );
    let mut request = reqwest::Request::new(
        Method::POST,
        reqwest::Url::parse(&failure_url).expect("failure URL should parse"),
    );
    *request.timeout_mut() = Some(Duration::from_millis(100));

    let error = pool
        .send_with_resolver(request, |_| async { Ok(OutboundProxyRoute::Direct) })
        .await
        .expect_err("request to an unresponsive listener should time out");
    assert!(error.is_timeout());

    let logs = String::from_utf8(buffer.lock().expect("log buffer lock").clone())
        .expect("logs should be UTF-8");
    assert!(logs.contains("log capture sentinel"));
    for secret in [
        "password-secret-value",
        "query-secret-value",
        "request-header-secret-value",
        "request-body-secret-value",
        "response-secret-value",
        "failure-password-secret-value",
        "failure-query-secret-value",
    ] {
        assert!(!logs.contains(secret), "logs exposed {secret}:\n{logs}");
    }
}

#[derive(Clone)]
struct FakeRouteResolver {
    routes: Arc<HashMap<String, OutboundProxyRoute>>,
    observed_urls: Arc<Mutex<Vec<String>>>,
}

impl FakeRouteResolver {
    fn new(routes: HashMap<String, OutboundProxyRoute>) -> Self {
        Self {
            routes: Arc::new(routes),
            observed_urls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn resolve(&self, request_url: String) -> io::Result<OutboundProxyRoute> {
        self.observed_urls
            .lock()
            .expect("observed URL lock")
            .push(request_url.clone());
        self.routes
            .get(&request_url)
            .cloned()
            .ok_or_else(|| io::Error::other(format!("no route for {request_url}")))
    }

    fn observed_urls(&self) -> Vec<String> {
        self.observed_urls
            .lock()
            .expect("observed URL lock")
            .clone()
    }
}

async fn resolve_with(
    pool: &RouteAwareClientPool,
    resolver: &FakeRouteResolver,
    request_url: &str,
) -> Result<HttpClient, RouteAwareClientPoolError> {
    let resolver = resolver.clone();
    let (_, client) = pool
        .client_for_url_with_resolver(request_url, move |request_url| async move {
            resolver.resolve(request_url).await
        })
        .await?;
    Ok(client)
}

fn manual_redirect_pool() -> RouteAwareClientPool {
    RouteAwareClientPool::with_builder(
        HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        ClientRouteClass::Api,
        HttpClientBuilder::new(),
    )
}

fn spawn_response_server(
    responses: Vec<String>,
) -> (std::net::SocketAddr, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("response listener should bind");
    let address = listener
        .local_addr()
        .expect("response listener should have an address");
    listener
        .set_nonblocking(true)
        .expect("response listener should become nonblocking");
    let server = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let deadline = Instant::now() + Duration::from_secs(2);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "response server should receive the next request"
                        );
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("response server should accept: {error}"),
                }
            };
            stream
                .set_nonblocking(false)
                .expect("response stream should become blocking");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("response stream should get a read timeout");
            requests.push(read_http_message(&mut stream));
            stream
                .write_all(response.as_bytes())
                .expect("response server should write response");
        }
        requests
    });
    (address, server)
}

fn read_http_message(stream: &mut impl Read) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let bytes_read = stream.read(&mut chunk).expect("HTTP message should read");
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
        if let Some(header_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            let body_start = header_end + 4;
            let headers = String::from_utf8_lossy(&buffer[..body_start]);
            let content_length = headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find_map(|(name, value)| {
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if buffer.len() >= body_start + content_length {
                break;
            }
        }
    }
    String::from_utf8_lossy(&buffer).into_owned()
}

#[derive(Clone)]
struct TestLogWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

struct TestLogSink {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for TestLogWriter {
    type Writer = TestLogSink;

    fn make_writer(&'a self) -> Self::Writer {
        TestLogSink {
            buffer: Arc::clone(&self.buffer),
        }
    }
}

impl Write for TestLogSink {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut log_buffer = self
            .buffer
            .lock()
            .map_err(|_| io::Error::other("log buffer lock was poisoned"))?;
        log_buffer.extend(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
