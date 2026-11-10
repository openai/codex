//! Verify proxy route planning, attribution handling, URL rewriting, and bridge cleanup.

use super::ConnectionSlots;
use super::PROXY_ATTRIBUTION_TOKEN_ENV_KEY;
use super::ProxyRouteEntry;
use super::ProxyRouteSpec;
use super::default_proxy_port;
use super::extract_attribution_token_and_plan;
use super::is_proxy_env_key;
use super::parse_loopback_proxy_endpoint;
use super::plan_proxy_routes;
use super::proxy_bidirectional;
use super::rewrite_proxy_env_value;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::net::Ipv4Addr;
use std::net::Shutdown;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

#[tokio::test]
async fn a_reset_on_either_side_releases_the_bridge_permit() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let connect = || {
        let peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        (peer, listener.accept().unwrap().0)
    };
    for reset_upstream in [false, true] {
        let (reset_peer, reset_socket) = connect();
        let (idle_peer, idle_socket) = connect();
        let slots = ConnectionSlots(Arc::new(Semaphore::new(1)));
        let (upstream, client) = if reset_upstream {
            (reset_socket, idle_socket)
        } else {
            (idle_socket, reset_socket)
        };
        let (done, result) = tokio::sync::oneshot::channel();
        slots.spawn(move || {
            let _ = done.send(proxy_bidirectional(upstream, client));
        });
        rustix::net::sockopt::set_socket_linger(&reset_peer, Some(Duration::ZERO)).unwrap();
        drop(reset_peer);

        let _permit = tokio::time::timeout(Duration::from_secs(5), slots.0.acquire())
            .await
            .expect("bridge must release its permit while the other peer is still open")
            .unwrap();
        assert!(result.await.unwrap().is_err());
        drop(idle_peer);
    }
}

#[test]
fn upload_failure_drains_the_pending_response() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let mut server = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let upstream = listener.accept().unwrap().0;
    let mut peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let client = listener.accept().unwrap().0;

    server.write_all(b"response").unwrap();
    assert!(upstream.peek(&mut [0]).unwrap() > 0);
    rustix::net::sockopt::set_socket_linger(&server, Some(Duration::ZERO)).unwrap();
    drop(server);
    peer.write_all(b"upload").unwrap();
    peer.shutdown(Shutdown::Write).unwrap();
    assert!(proxy_bidirectional(upstream, client).is_err());
    let mut response = Vec::new();
    peer.read_to_end(&mut response).unwrap();
    assert_eq!(response, b"response");
}

#[test]
fn recognizes_only_proxy_url_keys_written_by_managed_setup() {
    assert_eq!(is_proxy_env_key("HTTP_PROXY"), true);
    assert_eq!(is_proxy_env_key("http_proxy"), true);
    assert_eq!(is_proxy_env_key("WS_PROXY"), true);
    assert_eq!(is_proxy_env_key("wss_proxy"), true);
    assert_eq!(is_proxy_env_key("HtTp_PrOxY"), false);
    assert_eq!(is_proxy_env_key("pip_proxy"), false);
    assert_eq!(is_proxy_env_key("PATH"), false);
}

#[test]
fn parses_loopback_proxy_endpoint() {
    let endpoint = parse_loopback_proxy_endpoint("http://127.0.0.1:43128");
    assert_eq!(
        endpoint,
        Some(
            "127.0.0.1:43128"
                .parse::<SocketAddr>()
                .expect("valid socket")
        )
    );
}

#[test]
fn ignores_non_loopback_proxy_endpoint() {
    assert_eq!(
        parse_loopback_proxy_endpoint("http://example.com:3128"),
        None
    );
}

#[test]
fn plan_proxy_routes_only_includes_valid_loopback_endpoints() {
    let mut env = HashMap::new();
    env.insert(
        "HTTP_PROXY".to_string(),
        "http://127.0.0.1:43128".to_string(),
    );
    env.insert(
        "HTTPS_PROXY".to_string(),
        "http://example.com:3128".to_string(),
    );
    env.insert(
        "HtTp_PrOxY".to_string(),
        "http://127.0.0.1:32102".to_string(),
    );
    env.insert("PATH".to_string(), "/usr/bin".to_string());

    let plan = plan_proxy_routes(&env);
    assert_eq!(plan.has_proxy_config, true);
    assert_eq!(plan.routes.len(), 1);
    assert_eq!(plan.routes[0].env_key, "HTTP_PROXY");
    assert_eq!(
        plan.routes[0].endpoint,
        "127.0.0.1:43128"
            .parse::<SocketAddr>()
            .expect("valid socket")
    );
}

#[test]
fn attribution_token_is_extracted_before_proxy_route_planning() {
    let mut env = HashMap::new();
    env.insert(
        "HTTP_PROXY".to_string(),
        "http://127.0.0.1:43128".to_string(),
    );
    env.insert(
        PROXY_ATTRIBUTION_TOKEN_ENV_KEY.to_string(),
        "exec-token".to_string(),
    );

    let (attribution_token, plan) = extract_attribution_token_and_plan(env);

    assert_eq!(attribution_token.as_deref(), Some("exec-token"));
    assert_eq!(
        plan,
        super::ProxyRoutePlan {
            routes: vec![super::PlannedProxyRoute {
                env_key: "HTTP_PROXY".to_string(),
                endpoint: "127.0.0.1:43128"
                    .parse::<SocketAddr>()
                    .expect("valid socket"),
            }],
            has_proxy_config: true,
        }
    );
}

#[test]
fn rewrites_proxy_url_to_local_loopback_port() {
    let rewritten = rewrite_proxy_env_value("socks5h://127.0.0.1:8081", /*local_port*/ 43210)
        .expect("rewritten value");
    assert_eq!(rewritten, "socks5h://127.0.0.1:43210");
}

#[test]
fn default_proxy_ports_match_expected_schemes() {
    assert_eq!(default_proxy_port("http"), 80);
    assert_eq!(default_proxy_port("https"), 443);
    assert_eq!(default_proxy_port("socks5h"), 1080);
}

#[test]
fn proxy_route_spec_serialization_omits_proxy_urls() {
    let spec = ProxyRouteSpec {
        routes: vec![ProxyRouteEntry {
            env_key: "HTTP_PROXY".to_string(),
            control_fd: 3,
        }],
    };
    let serialized = serde_json::to_string(&spec).expect("proxy route spec should serialize");

    assert_eq!(
        serialized,
        r#"{"routes":[{"env_key":"HTTP_PROXY","control_fd":3}]}"#
    );
}
