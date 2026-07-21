//! Shared outbound proxy policy tests.

use super::*;
use crate::HttpClientBuilder;
use http::HeaderMap;
use http::HeaderValue;
use http::header::AUTHORIZATION;
use http::header::COOKIE;
use http::header::PROXY_AUTHORIZATION;
use pretty_assertions::assert_eq;
use std::io::Read;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;

#[path = "outbound_proxy_redirect_coverage_tests.rs"]
mod redirect_coverage_tests;

struct MapEnv {
    values: HashMap<String, String>,
}

fn spawn_proxy_listener() -> (std::net::SocketAddr, std::thread::JoinHandle<Vec<String>>) {
    spawn_http_listener(vec![
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string(),
    ])
}

fn spawn_redirect_listener(
    location: &str,
) -> (std::net::SocketAddr, std::thread::JoinHandle<Vec<String>>) {
    spawn_http_listener(vec![format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )])
}

fn spawn_http_listener(
    responses: Vec<String>,
) -> (std::net::SocketAddr, std::thread::JoinHandle<Vec<String>>) {
    let listener =
        std::net::TcpListener::bind(("127.0.0.1", 0)).expect("HTTP listener should bind");
    let address = listener
        .local_addr()
        .expect("HTTP listener should have an address");
    listener
        .set_nonblocking(true)
        .expect("HTTP listener should become nonblocking");
    let thread = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let deadline = Instant::now() + Duration::from_secs(10);
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "HTTP listener should receive the next request"
                        );
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("HTTP listener should accept: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .expect("HTTP stream should get a read timeout");
            requests.push(read_http_message(&mut stream));
            stream
                .write_all(response.as_bytes())
                .expect("HTTP listener should write response");
        }
        requests
    });
    (address, thread)
}

fn only_request(thread: std::thread::JoinHandle<Vec<String>>, source: &str) -> String {
    let requests = thread
        .join()
        .unwrap_or_else(|_| panic!("{source} thread should finish"));
    let [request]: [String; 1] = requests.try_into().unwrap_or_else(|requests: Vec<String>| {
        panic!(
            "{source} should receive one request, got {}",
            requests.len()
        )
    });
    request
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

#[test]
fn websocket_route_uses_http_equivalent_for_system_resolution() {
    let env = MapEnv {
        values: HashMap::new(),
    };
    let route = resolve_proxy_route(
        &env,
        "wss://api.openai.com/v1/responses",
        OutboundProxyPolicy::RespectSystemProxy,
        |request_url, origin| {
            assert_eq!(request_url, "https://api.openai.com/v1/responses");
            assert_eq!(origin.scheme, "https");
            assert_eq!(origin.host, "api.openai.com");
            assert_eq!(origin.port, 443);
            SystemProxyDecision::Proxy {
                url: "http://proxy.example:8080".to_string(),
            }
        },
    );

    assert_eq!(
        route,
        OutboundProxyRoute::Proxy {
            url: "http://proxy.example:8080".to_string(),
            no_proxy: None,
        }
    );
}

#[test]
fn reqwest_default_route_preserves_transport_proxy_behavior() {
    let env = MapEnv {
        values: HashMap::new(),
    };
    let route = resolve_proxy_route(
        &env,
        "wss://api.openai.com/v1/responses",
        OutboundProxyPolicy::ReqwestDefault,
        |_, _| panic!("default policy should not resolve system proxy settings"),
    );

    assert_eq!(route, OutboundProxyRoute::TransportDefault);
}

impl EnvSource for MapEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }
}

#[test]
fn proxy_env_value_matches_reqwest_casing_precedence() {
    let env = MapEnv {
        values: HashMap::from([
            ("HTTPS_PROXY".to_string(), "upper".to_string()),
            ("https_proxy".to_string(), "lower".to_string()),
            ("http_proxy".to_string(), "lower-only".to_string()),
            ("ALL_PROXY".to_string(), String::new()),
            ("all_proxy".to_string(), "masked".to_string()),
        ]),
    };

    assert_eq!(
        proxy_env_value(&env, "HTTPS_PROXY"),
        Some("upper".to_string())
    );
    assert_eq!(
        proxy_env_value(&env, "HTTP_PROXY"),
        Some("lower-only".to_string())
    );
    assert_eq!(proxy_env_value(&env, "ALL_PROXY"), None);
}

#[test]
fn environment_fallback_reads_injected_proxy_environment() {
    let env = MapEnv {
        values: HashMap::from([("HTTPS_PROXY".to_string(), "://invalid".to_string())]),
    };
    let route = resolve_env_proxy_route(&env, EnvProxyKind::Https);
    let result = configure_builder_for_resolved_route(
        reqwest::Client::builder(),
        ClientRouteClass::Auth,
        &route,
    );

    assert!(matches!(
        result,
        Err(BuildRouteAwareHttpClientError::InvalidProxyConfig {
            route_class: ClientRouteClass::Auth,
        })
    ));
}

#[test]
fn unavailable_system_route_resolves_environment_or_direct_explicitly() {
    let env = MapEnv {
        values: HashMap::from([
            (
                "HTTPS_PROXY".to_string(),
                "http://proxy.example:8080".to_string(),
            ),
            ("NO_PROXY".to_string(), "localhost,.internal".to_string()),
        ]),
    };

    assert_eq!(
        route_from_system_decision(
            &env,
            EnvProxyKind::Https,
            SystemProxyDecision::Unavailable {
                failure: RouteFailureClass::ProxyResolutionUnavailable,
            },
        ),
        OutboundProxyRoute::Proxy {
            url: "http://proxy.example:8080".to_string(),
            no_proxy: Some("localhost,.internal".to_string()),
        }
    );
    assert_eq!(
        route_from_system_decision(
            &MapEnv {
                values: HashMap::new(),
            },
            EnvProxyKind::Https,
            SystemProxyDecision::Unavailable {
                failure: RouteFailureClass::ProxyResolutionUnavailable,
            },
        ),
        OutboundProxyRoute::Direct
    );
}

#[test]
fn unavailable_system_route_preserves_wss_http_proxy_fallback() {
    let env = MapEnv {
        values: HashMap::from([(
            "HTTP_PROXY".to_string(),
            "http://proxy.example:8080".to_string(),
        )]),
    };

    let route = resolve_proxy_route(
        &env,
        "wss://api.openai.com/v1/responses",
        OutboundProxyPolicy::RespectSystemProxy,
        |_, _| SystemProxyDecision::Unavailable {
            failure: RouteFailureClass::ProxyResolutionUnavailable,
        },
    );

    assert_eq!(
        route,
        OutboundProxyRoute::Proxy {
            url: "http://proxy.example:8080".to_string(),
            no_proxy: None,
        }
    );
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
#[tokio::test]
async fn async_resolution_uses_cached_route_before_global_permit() {
    let request_url = "https://cached-fast-path.test/request";
    cache_system_proxy_decision(request_url, SystemProxyDecision::Direct);
    let factory = HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy);
    let permit = ASYNC_SYSTEM_PROXY_RESOLUTION_PERMIT
        .acquire()
        .await
        .expect("global proxy permit should stay open");

    let route = tokio::time::timeout(
        Duration::from_secs(2),
        factory.resolve_proxy_route_async(request_url.to_string()),
    )
    .await
    .expect("cached resolution should not wait for the global permit")
    .expect("cached route should resolve");
    drop(permit);

    assert_eq!(route, OutboundProxyRoute::Direct);
}

#[tokio::test]
async fn enabled_environment_proxy_routes_request_through_proxy() {
    let (proxy_addr, proxy_thread) = spawn_proxy_listener();
    let env = MapEnv {
        values: HashMap::from([("HTTP_PROXY".to_string(), format!("http://{proxy_addr}"))]),
    };
    let request_url = "http://enabled-proxy.test/proxy-check";
    let builder = configure_proxy_for_route(
        &env,
        reqwest::Client::builder().timeout(Duration::from_secs(2)),
        request_url,
        ClientRouteClass::Auth,
        OutboundProxyPolicy::RespectSystemProxy,
        |_, _| SystemProxyDecision::Unavailable {
            failure: RouteFailureClass::ProxyResolutionUnavailable,
        },
    )
    .expect("enabled proxy route should configure");

    let response = builder
        .build()
        .expect("proxy client should build")
        .get(request_url)
        .send()
        .await
        .expect("request should use local proxy");
    let proxy_request = only_request(proxy_thread, "proxy");

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        proxy_request.lines().next(),
        Some("GET http://enabled-proxy.test/proxy-check HTTP/1.1")
    );
}

#[tokio::test]
async fn route_aware_builder_preserves_default_headers() {
    let (server_addr, server_thread) = spawn_proxy_listener();
    let request_url = format!("http://{server_addr}/builder-check");
    cache_system_proxy_decision(&request_url, SystemProxyDecision::Direct);
    let mut headers = HeaderMap::new();
    headers.insert("x-builder-test", HeaderValue::from_static("preserved"));
    let factory = HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy);
    let client = HttpClientBuilder::new()
        .default_headers(headers)
        .build_respecting_outbound_proxy_policy(&factory, &request_url, ClientRouteClass::Api)
        .expect("route-aware client should build");

    let response = client
        .get(&request_url)
        .send()
        .await
        .expect("request should use direct route");
    let request = only_request(server_thread, "server");

    assert!(response.status().is_success());
    assert!(
        request
            .lines()
            .any(|line| line.eq_ignore_ascii_case("x-builder-test: preserved"))
    );
}

#[tokio::test]
async fn route_aware_pool_uses_respect_system_proxy_route_for_exact_url() {
    let (proxy_addr, proxy_thread) = spawn_proxy_listener();
    let request_url = "http://route-aware-proxy.test/proxy-check?pac=exact";
    cache_system_proxy_decision(
        request_url,
        SystemProxyDecision::Proxy {
            url: format!("http://{proxy_addr}"),
        },
    );
    let pool = crate::RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        ClientRouteClass::Api,
    );

    let response = tokio::time::timeout(Duration::from_secs(2), pool.get(request_url).send())
        .await
        .expect("proxy request should finish")
        .expect("request should use local proxy");
    let proxy_request = only_request(proxy_thread, "proxy");

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        proxy_request.lines().next(),
        Some("GET http://route-aware-proxy.test/proxy-check?pac=exact HTTP/1.1")
    );
}

#[tokio::test]
async fn route_aware_pool_logs_only_the_final_redirect_outcome() {
    let (proxy_addr, proxy_thread) = spawn_proxy_listener();
    let redirected_url = "http://redirect-target.test/final?token=redirect-target-secret-value";
    let (redirect_addr, redirect_thread) = spawn_redirect_listener(redirected_url);
    let initial_url = format!("http://{redirect_addr}/start");
    cache_system_proxy_decision(&initial_url, SystemProxyDecision::Direct);
    cache_system_proxy_decision(
        redirected_url,
        SystemProxyDecision::Proxy {
            url: format!("http://{proxy_addr}"),
        },
    );
    let pool = crate::RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        ClientRouteClass::Api,
    );
    let log_buffer = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(TestLogWriter {
                buffer: Arc::clone(&log_buffer),
            })
            .with_filter(
                tracing_subscriber::filter::Targets::new()
                    .with_target("codex_http_client", tracing::Level::TRACE),
            ),
    );
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::debug!(target: "codex_http_client", "log capture sentinel");

    let response = tokio::time::timeout(Duration::from_secs(2), pool.get(&initial_url).send())
        .await
        .expect("redirected request should finish")
        .expect("redirected request should use selected routes");
    only_request(redirect_thread, "redirect");
    only_request(proxy_thread, "proxy");

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let logs = String::from_utf8(log_buffer.lock().expect("log buffer lock").clone())
        .expect("logs should be UTF-8");
    assert!(logs.contains("log capture sentinel"));
    assert!(logs.contains(&initial_url));
    assert_eq!(logs.matches("Request completed").count(), 1);
    for secret in ["redirect-target-secret-value", redirected_url, "location"] {
        assert!(
            !logs
                .to_ascii_lowercase()
                .contains(&secret.to_ascii_lowercase())
        );
    }
}

#[test]
fn parses_pac_proxy_tokens() {
    assert_eq!(
        parse_proxy_list("PROXY proxy.internal:8080; DIRECT", "https"),
        ParsedProxyListDecision::Proxy("http://proxy.internal:8080".to_string())
    );
    assert_eq!(
        parse_proxy_list("HTTPS proxy.internal:8443", "https"),
        ParsedProxyListDecision::Proxy("https://proxy.internal:8443".to_string())
    );
}

#[test]
fn unavailable_system_proxy_decision_is_cached() {
    let request_url = "https://unavailable-cache.test/oauth/token";
    let decision = SystemProxyDecision::Unavailable {
        failure: RouteFailureClass::ProxyResolutionUnavailable,
    };

    cache_system_proxy_decision(request_url, decision.clone());

    assert_eq!(cached_system_proxy_decision(request_url), Some(decision));
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

#[test]
fn system_proxy_resolution_is_single_flight() {
    let cache = Arc::new(Mutex::new(HashMap::new()));
    let request_url = "https://single-flight.test/models";
    let origin = RequestOrigin::parse(request_url).expect("valid request URL");
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let worker_cache = Arc::clone(&cache);
    let worker_origin = origin.clone();

    let worker = std::thread::spawn(move || {
        resolve_system_proxy_with(&worker_cache, request_url, &worker_origin, |_, _| {
            started_tx.send(()).expect("test should still be running");
            release_rx.recv().expect("test should release resolver");
            SystemProxyDecision::Direct
        })
    });

    started_rx.recv().expect("resolver should start");
    assert!(matches!(
        cache.try_lock(),
        Err(std::sync::TryLockError::WouldBlock)
    ));
    release_tx
        .send(())
        .expect("resolver should still be running");
    assert_eq!(
        worker.join().expect("resolver should finish"),
        SystemProxyDecision::Direct
    );
    assert_eq!(
        resolve_system_proxy_with(&cache, request_url, &origin, |_, _| {
            panic!("cached waiter should not resolve the platform proxy again")
        }),
        SystemProxyDecision::Direct
    );
}

#[test]
fn system_proxy_cache_is_bounded() {
    let mut cache = HashMap::new();
    let now = Instant::now();

    for index in 0..=SYSTEM_PROXY_CACHE_MAX_ENTRIES {
        insert_system_proxy_cache_entry(
            &mut cache,
            &format!("https://bounded-cache.test/{index}"),
            SystemProxyDecision::Direct,
            now,
        );
    }

    assert_eq!(cache.len(), SYSTEM_PROXY_CACHE_MAX_ENTRIES);
}

#[test]
fn parses_static_winhttp_proxy_entries_for_target_scheme() {
    assert_eq!(
        parse_proxy_list("http=web-proxy:8080;https=secure-proxy:8443", "https"),
        ParsedProxyListDecision::Proxy("http://secure-proxy:8443".to_string())
    );
    assert_eq!(
        parse_proxy_list("http=web-proxy:8080 https=secure-proxy:8443", "https"),
        ParsedProxyListDecision::Proxy("http://secure-proxy:8443".to_string())
    );
    assert_eq!(
        parse_proxy_list("http=web-proxy:8080", "https"),
        ParsedProxyListDecision::Unavailable
    );
    assert_eq!(
        parse_proxy_list("proxy.internal:8080", "https"),
        ParsedProxyListDecision::Proxy("http://proxy.internal:8080".to_string())
    );
}

#[test]
fn reports_direct_and_unsupported_proxy_tokens() {
    assert_eq!(
        parse_proxy_list("DIRECT; PROXY proxy.internal:8080", "https"),
        ParsedProxyListDecision::Direct
    );
    assert_eq!(
        parse_proxy_list("DIRECT", "https"),
        ParsedProxyListDecision::Direct
    );
    assert_eq!(
        parse_proxy_list("SOCKS proxy.internal:1080", "https"),
        ParsedProxyListDecision::UnsupportedScheme
    );
}

#[test]
fn no_proxy_matches_exact_suffix_wildcard_and_port() {
    let origin = RequestOrigin {
        scheme: "https".to_string(),
        host: "auth.openai.com".to_string(),
        port: 443,
    };
    assert!(no_proxy_matches_origin("auth.openai.com", &origin));
    assert!(!no_proxy_matches_origin("openai.com", &origin));
    assert!(no_proxy_matches_origin(".openai.com", &origin));
    assert!(no_proxy_matches_origin("*.openai.com", &origin));
    assert!(no_proxy_matches_origin("auth.openai.com:443", &origin));
    assert!(!no_proxy_matches_origin("auth.openai.com:8443", &origin));
}

#[test]
fn system_proxy_cache_key_preserves_url_specific_pac_decisions() {
    let request_url = "https://auth.openai.com/oauth/token?access_token=secret";
    let cache_key = system_proxy_cache_key(request_url);

    assert_ne!(
        cache_key,
        system_proxy_cache_key("https://auth.openai.com/oauth/revoke")
    );
    assert!(!cache_key.contains(request_url));
}
