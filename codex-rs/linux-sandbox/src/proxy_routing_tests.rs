//! Verify proxy route planning, attribution handling, and URL rewriting.

use super::PROXY_ATTRIBUTION_TOKEN_ENV_KEY;
use super::ProxyRouteEntry;
use super::ProxyRouteSpec;
use super::default_proxy_port;
use super::extract_attribution_token_and_plan;
use super::is_proxy_env_key;
use super::parse_loopback_proxy_endpoint;
use super::plan_proxy_routes;
use super::rewrite_proxy_env_value;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::net::SocketAddr;

#[test]
fn recognizes_proxy_env_keys_case_insensitively() {
    assert_eq!(is_proxy_env_key("HTTP_PROXY"), true);
    assert_eq!(is_proxy_env_key("http_proxy"), true);
    assert_eq!(is_proxy_env_key("WS_PROXY"), true);
    assert_eq!(is_proxy_env_key("wss_proxy"), true);
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
