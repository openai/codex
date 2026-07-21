use super::*;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;

#[tokio::test]
async fn route_aware_pool_strips_credentials_on_cross_origin_redirect() {
    let (destination_addr, destination_thread) = spawn_proxy_listener();
    let destination_url = format!("http://{destination_addr}/final");
    let (redirect_addr, redirect_thread) = spawn_redirect_listener(&destination_url);
    let initial_url = format!("http://{redirect_addr}/start");
    cache_system_proxy_decision(&initial_url, SystemProxyDecision::Direct);
    cache_system_proxy_decision(&destination_url, SystemProxyDecision::Direct);
    let pool = crate::RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        ClientRouteClass::Api,
    );

    let response = tokio::time::timeout(
        Duration::from_secs(2),
        pool.get(&initial_url)
            .header(AUTHORIZATION, "Bearer origin-secret")
            .header(COOKIE, "session=origin-secret")
            .header(PROXY_AUTHORIZATION, "Basic proxy-secret")
            .send(),
    )
    .await
    .expect("redirected request should finish")
    .expect("cross-origin redirect should succeed");
    let initial_request = only_request(redirect_thread, "redirect");
    let destination_request = only_request(destination_thread, "destination");

    assert_eq!(response.url().as_str(), destination_url);
    assert_eq!(
        [
            credential_headers(&initial_request),
            credential_headers(&destination_request),
        ],
        [(true, true, true), (false, false, false)]
    );
}

#[tokio::test]
async fn route_aware_pool_retains_credentials_for_same_origin_and_route() {
    let (proxy_addr, proxy_thread) = spawn_http_listener(vec![
        "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_string(),
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string(),
    ]);
    let initial_url = "http://same-route.test/start";
    let redirected_url = "http://same-route.test/final";
    let route = SystemProxyDecision::Proxy {
        url: format!("http://{proxy_addr}"),
    };
    cache_system_proxy_decision(initial_url, route.clone());
    cache_system_proxy_decision(redirected_url, route);
    let pool = crate::RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        ClientRouteClass::Api,
    );

    let response = tokio::time::timeout(
        Duration::from_secs(2),
        pool.get(initial_url)
            .header(AUTHORIZATION, "Bearer origin-secret")
            .header(COOKIE, "session=origin-secret")
            .header(PROXY_AUTHORIZATION, "Basic proxy-secret")
            .send(),
    )
    .await
    .expect("redirected request should finish")
    .expect("same-route redirect should succeed");
    let requests = proxy_thread.join().expect("proxy thread should finish");

    assert_eq!(response.url().as_str(), redirected_url);
    assert_eq!(
        requests
            .iter()
            .map(|request| credential_headers(request))
            .collect::<Vec<_>>(),
        vec![(true, true, true), (true, true, true)]
    );
}

#[tokio::test]
async fn route_aware_pool_sanitizes_redirected_failure_logs() {
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

    let (enabled_failure_addr, enabled_failure_thread) = spawn_failing_listener();
    let enabled_target_url =
        format!("http://{enabled_failure_addr}/final?token=enabled-target-secret");
    let (enabled_redirect_addr, enabled_redirect_thread) =
        spawn_redirect_listener(&enabled_target_url);
    let enabled_initial_url = format!("http://{enabled_redirect_addr}/start");
    cache_system_proxy_decision(&enabled_initial_url, SystemProxyDecision::Direct);
    cache_system_proxy_decision(&enabled_target_url, SystemProxyDecision::Direct);
    let enabled_pool = crate::RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        ClientRouteClass::Api,
    );

    enabled_pool
        .get(&enabled_initial_url)
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .expect_err("redirect target should fail");
    only_request(enabled_redirect_thread, "enabled redirect");
    enabled_failure_thread
        .join()
        .expect("enabled failure thread should finish");

    let (disabled_failure_addr, disabled_failure_thread) = spawn_failing_listener();
    let disabled_target_url =
        format!("http://{disabled_failure_addr}/final?token=disabled-target-secret");
    let (disabled_redirect_addr, disabled_redirect_thread) =
        spawn_redirect_listener(&disabled_target_url);
    let disabled_initial_url = format!("http://{disabled_redirect_addr}/start");
    cache_system_proxy_decision(&disabled_initial_url, SystemProxyDecision::Direct);
    cache_system_proxy_decision(&disabled_target_url, SystemProxyDecision::Direct);
    let disabled_pool = crate::RouteAwareClientPool::new_without_request_logging(
        HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        ClientRouteClass::Api,
    );

    disabled_pool
        .get(&disabled_initial_url)
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .expect_err("redirect target should fail");
    only_request(disabled_redirect_thread, "disabled redirect");
    disabled_failure_thread
        .join()
        .expect("disabled failure thread should finish");

    let logs = String::from_utf8(log_buffer.lock().expect("log buffer lock").clone())
        .expect("logs should be UTF-8");
    assert!(logs.contains("log capture sentinel"));
    assert!(logs.contains(&enabled_initial_url));
    assert!(!logs.contains(&disabled_initial_url));
    assert_eq!(logs.matches("Request failed").count(), 1);
    assert!(logs.contains("is_timeout"));
    assert!(logs.contains("is_connect"));
    for secret in [
        "enabled-target-secret",
        "disabled-target-secret",
        enabled_target_url.as_str(),
        disabled_target_url.as_str(),
        "error=",
    ] {
        assert!(!logs.contains(secret), "logs exposed {secret}:\n{logs}");
    }
}

fn credential_headers(request: &str) -> (bool, bool, bool) {
    let names = request
        .lines()
        .filter_map(|line| {
            line.split_once(':')
                .map(|(name, _)| name.to_ascii_lowercase())
        })
        .collect::<Vec<_>>();
    (
        names.iter().any(|name| name == "authorization"),
        names.iter().any(|name| name == "cookie"),
        names.iter().any(|name| name == "proxy-authorization"),
    )
}

fn spawn_failing_listener() -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
    let listener =
        std::net::TcpListener::bind(("127.0.0.1", 0)).expect("failing listener should bind");
    let address = listener
        .local_addr()
        .expect("failing listener should have an address");
    listener
        .set_nonblocking(true)
        .expect("failing listener should become nonblocking");
    let thread = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "failing listener should receive a request"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("failing listener should accept: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("failing stream should get a read timeout");
        read_http_message(&mut stream);
    });
    (address, thread)
}
