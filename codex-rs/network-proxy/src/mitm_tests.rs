use super::*;

use crate::config::NetworkProxySettings;
use crate::reasons::REASON_METHOD_NOT_ALLOWED;
use crate::reasons::REASON_MITM_HOOK_DENIED;
use crate::reasons::REASON_NOT_ALLOWED_LOCAL;
use crate::runtime::network_proxy_state_for_policy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use rama_http::Body;
use rama_http::HeaderMap;
use rama_http::HeaderValue;
use rama_http::Method;
use rama_http::Request;
use rama_http::StatusCode;
use rama_http::header::HeaderName;
use tempfile::NamedTempFile;

fn github_write_hook() -> crate::mitm_hook::MitmHookConfig {
    crate::mitm_hook::MitmHookConfig {
        host: "api.github.com".to_string(),
        matcher: crate::mitm_hook::MitmHookMatchConfig {
            methods: vec!["POST".to_string(), "PUT".to_string()],
            path_prefixes: vec!["/repos/openai/".to_string()],
            ..crate::mitm_hook::MitmHookMatchConfig::default()
        },
        actions: crate::mitm_hook::MitmHookActionsConfig {
            strip_request_headers: vec!["authorization".to_string()],
            inject_request_headers: vec![crate::mitm_hook::InjectedHeaderConfig {
                name: "authorization".to_string(),
                secret_env_var: Some("CODEX_GITHUB_TOKEN".to_string()),
                secret_file: None,
                prefix: Some("Bearer ".to_string()),
            }],
        },
    }
}

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
    let app_state = Arc::new(network_proxy_state_for_policy({
        let mut network = NetworkProxySettings::default();
        network.set_allowed_domains(vec!["example.com".to_string()]);
        network
    }));
    let ctx = policy_ctx(
        app_state.clone(),
        NetworkMode::Limited,
        "example.com",
        /*target_port*/ 443,
    );
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
    let app_state = Arc::new(network_proxy_state_for_policy({
        let mut network = NetworkProxySettings::default();
        network.set_allowed_domains(vec!["example.com".to_string()]);
        network
    }));
    let ctx = policy_ctx(
        app_state.clone(),
        NetworkMode::Full,
        "example.com",
        /*target_port*/ 443,
    );
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
    let app_state = Arc::new(network_proxy_state_for_policy({
        let mut network = NetworkProxySettings::default();
        network.set_allowed_domains(vec!["example.com".to_string()]);
        network.allow_local_binding = false;
        network
    }));
    let ctx = policy_ctx(
        app_state.clone(),
        NetworkMode::Full,
        "10.0.0.1",
        /*target_port*/ 443,
    );
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

#[tokio::test]
async fn mitm_policy_allows_matching_hooked_write_in_full_mode() {
    let secret_file = NamedTempFile::new().unwrap();
    std::fs::write(secret_file.path(), "ghp-secret\n").unwrap();
    let mut hook = github_write_hook();
    hook.actions.inject_request_headers[0].secret_env_var = None;
    hook.actions.inject_request_headers[0].secret_file =
        Some(secret_file.path().display().to_string());
    let mut network = NetworkProxySettings {
        mitm: true,
        mitm_hooks: vec![hook],
        mode: NetworkMode::Full,
        ..NetworkProxySettings::default()
    };
    network.set_allowed_domains(vec!["api.github.com".to_string()]);
    let app_state = Arc::new(network_proxy_state_for_policy(network));
    let ctx = policy_ctx(
        app_state.clone(),
        NetworkMode::Full,
        "api.github.com",
        /*target_port*/ 443,
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/repos/openai/codex/issues")
        .header(HOST, "api.github.com")
        .body(Body::empty())
        .unwrap();

    let response = mitm_blocking_response(&req, &ctx).await.unwrap();

    assert!(
        response.is_none(),
        "matching hook should bypass method clamp"
    );
    assert_eq!(app_state.blocked_snapshot().await.unwrap().len(), 0);
}

#[tokio::test]
async fn mitm_policy_blocks_hook_miss_for_hooked_host_and_records_telemetry_in_full_mode() {
    let secret_file = NamedTempFile::new().unwrap();
    std::fs::write(secret_file.path(), "ghp-secret\n").unwrap();
    let mut hook = github_write_hook();
    hook.actions.inject_request_headers[0].secret_env_var = None;
    hook.actions.inject_request_headers[0].secret_file =
        Some(secret_file.path().display().to_string());
    let mut network = NetworkProxySettings {
        mitm: true,
        mitm_hooks: vec![hook],
        mode: NetworkMode::Full,
        ..NetworkProxySettings::default()
    };
    network.set_allowed_domains(vec!["api.github.com".to_string()]);
    let app_state = Arc::new(network_proxy_state_for_policy(network));
    let ctx = policy_ctx(
        app_state.clone(),
        NetworkMode::Full,
        "api.github.com",
        /*target_port*/ 443,
    );
    let req = Request::builder()
        .method(Method::GET)
        .uri("/repos/openai/codex/issues?token=secret")
        .header(HOST, "api.github.com")
        .header("authorization", "Bearer user-supplied")
        .body(Body::empty())
        .unwrap();

    let response = mitm_blocking_response(&req, &ctx)
        .await
        .unwrap()
        .expect("hook miss should be blocked");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response.headers().get("x-proxy-error").unwrap(),
        "blocked-by-mitm-hook"
    );

    let blocked = app_state.drain_blocked().await.unwrap();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].reason, REASON_MITM_HOOK_DENIED);
    assert_eq!(blocked[0].method.as_deref(), Some("GET"));
    assert_eq!(blocked[0].host, "api.github.com");
    assert_eq!(blocked[0].port, Some(443));
}

#[test]
fn apply_mitm_hook_actions_replaces_authorization_header() {
    let mut headers = HeaderMap::new();
    headers.append(
        HeaderName::from_static("authorization"),
        HeaderValue::from_static("Bearer user-supplied"),
    );
    headers.append(
        HeaderName::from_static("x-request-id"),
        HeaderValue::from_static("req_123"),
    );

    let actions = crate::mitm_hook::MitmHookActions {
        strip_request_headers: vec![HeaderName::from_static("authorization")],
        inject_request_headers: vec![crate::mitm_hook::ResolvedInjectedHeader {
            name: HeaderName::from_static("authorization"),
            value: HeaderValue::from_static("Bearer secret-token"),
            source: crate::mitm_hook::SecretSource::File(
                AbsolutePathBuf::try_from("/tmp/github-token").unwrap(),
            ),
        }],
    };

    apply_mitm_hook_actions(&mut headers, Some(&actions));

    assert_eq!(
        headers.get("authorization"),
        Some(&HeaderValue::from_static("Bearer secret-token"))
    );
    assert_eq!(
        headers.get("x-request-id"),
        Some(&HeaderValue::from_static("req_123"))
    );
}
