use super::*;
use pretty_assertions::assert_eq;
use std::io::Read;
use std::io::Write;

struct MapEnv {
    values: HashMap<String, String>,
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
    let origin = RequestOrigin::parse("https://auth.openai.com/oauth/token").expect("valid URL");
    let result = configure_env_proxy_handling(
        &env,
        reqwest::Client::builder(),
        Some(&origin),
        ClientRouteClass::Auth,
    );

    assert!(matches!(
        result,
        Err(BuildRouteAwareHttpClientError::InvalidProxyConfig {
            route_class: ClientRouteClass::Auth,
        })
    ));
}

#[tokio::test]
async fn enabled_environment_proxy_routes_request_through_proxy() {
    let listener =
        std::net::TcpListener::bind(("127.0.0.1", 0)).expect("local proxy listener should bind");
    let proxy_addr = listener
        .local_addr()
        .expect("local proxy listener should have an address");
    let proxy_thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("proxy should accept a request");
        let mut buffer = [0_u8; 4096];
        let size = stream.read(&mut buffer).expect("proxy should read request");
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .expect("proxy should write response");
        String::from_utf8_lossy(&buffer[..size]).into_owned()
    });
    let env = MapEnv {
        values: HashMap::from([("HTTP_PROXY".to_string(), format!("http://{proxy_addr}"))]),
    };
    let request_url = "http://enabled-proxy.test/proxy-check";
    let config = OutboundProxyConfig::respect_system_proxy();
    let builder = configure_proxy_for_route(
        &env,
        reqwest::Client::builder().timeout(Duration::from_secs(2)),
        request_url,
        ClientRouteClass::Auth,
        Some(&config),
    )
    .expect("enabled proxy route should configure");

    let response = builder
        .build()
        .expect("proxy client should build")
        .get(request_url)
        .send()
        .await
        .expect("request should use local proxy");
    let proxy_request = proxy_thread.join().expect("proxy thread should finish");

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        proxy_request.lines().next(),
        Some("GET http://enabled-proxy.test/proxy-check HTTP/1.1")
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
