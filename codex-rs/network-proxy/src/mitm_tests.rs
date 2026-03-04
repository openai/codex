use super::*;

use crate::config::NetworkProxySettings;
use crate::reasons::REASON_METHOD_NOT_ALLOWED;
use crate::reasons::REASON_NOT_ALLOWED_LOCAL;
use crate::runtime::network_proxy_state_for_policy;
use pretty_assertions::assert_eq;
use rama_http::Body;
use rama_http::Method;
use rama_http::Request;
use rama_http::StatusCode;

fn policy_ctx(
    app_state: Arc<NetworkProxyState>,
    mode: NetworkMode,
    target_host: &str,
    target_port: u16,
) -> MitmPolicyContext {
    MitmPolicyContext {
        target_host: target_host.to_string(),
        target_port,
        mode,
        app_state,
    }
}

#[tokio::test]
async fn mitm_policy_blocks_disallowed_method_and_records_telemetry() {
    let app_state = Arc::new(network_proxy_state_for_policy(NetworkProxySettings {
        allowed_domains: vec!["example.com".to_string()],
        ..NetworkProxySettings::default()
    }));
    let ctx = policy_ctx(app_state.clone(), NetworkMode::Limited, "example.com", 443);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/v1/responses?api_key=secret")
        .header(HOST, "example.com")
        .body(Body::empty())
        .unwrap();

    let response = mitm_blocking_response(&req, &ctx)
        .await
        .unwrap()
        .expect("POST should be blocked in limited mode");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response.headers().get("x-proxy-error").unwrap(),
        "blocked-by-method-policy"
    );

    let blocked = app_state.drain_blocked().await.unwrap();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].reason, REASON_METHOD_NOT_ALLOWED);
    assert_eq!(blocked[0].method.as_deref(), Some("POST"));
    assert_eq!(blocked[0].host, "example.com");
    assert_eq!(blocked[0].port, Some(443));
}

#[tokio::test]
async fn mitm_policy_rejects_host_mismatch() {
    let app_state = Arc::new(network_proxy_state_for_policy(NetworkProxySettings {
        allowed_domains: vec!["example.com".to_string()],
        ..NetworkProxySettings::default()
    }));
    let ctx = policy_ctx(app_state.clone(), NetworkMode::Full, "example.com", 443);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/")
        .header(HOST, "evil.example")
        .body(Body::empty())
        .unwrap();

    let response = mitm_blocking_response(&req, &ctx)
        .await
        .unwrap()
        .expect("mismatched host should be rejected");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(app_state.blocked_snapshot().await.unwrap().len(), 0);
}

#[tokio::test]
async fn mitm_policy_rechecks_local_private_target_after_connect() {
    let app_state = Arc::new(network_proxy_state_for_policy(NetworkProxySettings {
        allowed_domains: vec!["*".to_string()],
        allow_local_binding: false,
        ..NetworkProxySettings::default()
    }));
    let ctx = policy_ctx(app_state.clone(), NetworkMode::Full, "10.0.0.1", 443);
    let req = Request::builder()
        .method(Method::GET)
        .uri("/health?token=secret")
        .header(HOST, "10.0.0.1")
        .body(Body::empty())
        .unwrap();

    let response = mitm_blocking_response(&req, &ctx)
        .await
        .unwrap()
        .expect("local/private target should be blocked on inner request");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let blocked = app_state.drain_blocked().await.unwrap();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].reason, REASON_NOT_ALLOWED_LOCAL);
    assert_eq!(blocked[0].host, "10.0.0.1");
    assert_eq!(blocked[0].port, Some(443));
}
