use super::*;

use crate::CredentialProviderConfig;
use crate::config::NetworkProxyConfig;
use crate::reasons::REASON_METHOD_NOT_ALLOWED;
use crate::reasons::REASON_MITM_HOOK_DENIED;
use crate::reasons::REASON_NOT_ALLOWED_LOCAL;
use crate::runtime::network_proxy_state_for_policy;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use rama_http::Body;
use rama_http::HeaderMap;
use rama_http::HeaderValue;
use rama_http::Method;
use rama_http::Request;
use rama_http::StatusCode;
use rama_http::header::HeaderName;
use std::collections::HashMap;
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
        scheme: Scheme::HTTPS,
        mode,
        app_state,
    }
}

#[tokio::test]
async fn mitm_policy_blocks_disallowed_method_and_records_telemetry() {
    let app_state = Arc::new(network_proxy_state_for_policy({
        let mut network = NetworkProxyConfig::default();
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
        let mut network = NetworkProxyConfig::default();
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
        let mut network = NetworkProxyConfig::default();
        network.set_allowed_domains(vec!["example.com".to_string()]);
        network.allow_local_binding = Some(false);
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
async fn mitm_policy_rechecks_dns_for_approved_unallowlisted_host() {
    let app_state = Arc::new(network_proxy_state_for_policy(NetworkProxyConfig::default()));
    let host = "does-not-resolve.invalid";
    // The context represents an approved CONNECT, without a persisted allowlist entry.
    let ctx = policy_ctx(
        Arc::clone(&app_state),
        NetworkMode::Full,
        host,
        /*target_port*/ 443,
    );
    let req = Request::builder()
        .method(Method::GET)
        .uri("/")
        .header(HOST, host)
        .body(Body::empty())
        .unwrap();
    let response = mitm_blocking_response(&req, &ctx)
        .await
        .unwrap()
        .expect("approved CONNECT must still validate DNS on inner requests");
    let blocked = app_state.drain_blocked().await.unwrap();
    assert_eq!(
        (
            response.status(),
            blocked
                .iter()
                .map(|request| (request.host.as_str(), request.reason.as_str(), request.port))
                .collect::<Vec<_>>()
        ),
        (
            StatusCode::FORBIDDEN,
            vec![(host, REASON_NOT_ALLOWED_LOCAL, Some(443))]
        ),
    );
}

#[tokio::test]
async fn mitm_policy_allows_matching_hooked_write_in_full_mode() {
    let secret_file = NamedTempFile::new().unwrap();
    std::fs::write(secret_file.path(), "ghp-secret\n").unwrap();
    let mut hook = github_write_hook();
    hook.actions.inject_request_headers[0].secret_env_var = None;
    hook.actions.inject_request_headers[0].secret_file =
        Some(secret_file.path().display().to_string());
    let mut network = NetworkProxyConfig {
        mitm: true,
        mitm_hooks: vec![hook],
        mode: NetworkMode::Full,
        ..NetworkProxyConfig::default()
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
async fn mitm_policy_enforces_brokered_aliases_without_blocking_unrelated_credentials() {
    for hooked_hosts in [
        vec!["api.github.com"],
        vec!["api.github.com", "github.com"],
        vec![],
    ] {
        let network = NetworkProxyConfig {
            credential_broker: true,
            mitm: true,
            allow_local_binding: Some(true),
            mitm_hooks: hooked_hosts
                .iter()
                .map(|host| {
                    let mut hook = github_write_hook();
                    hook.host = (*host).to_string();
                    hook.matcher.methods = vec!["GET".to_string()];
                    hook.actions.inject_request_headers.clear();
                    hook
                })
                .collect(),
            ..NetworkProxyConfig::default()
        };
        let app_state = Arc::new(network_proxy_state_for_policy(network));
        let mut env = HashMap::from([
            ("GH_TOKEN".to_string(), "ghp-real".to_string()),
            ("OPENAI_API_KEY".to_string(), "sk-real".to_string()),
            ("GH_HOST".to_string(), "enterprise.example".to_string()),
            (
                "GH_ENTERPRISE_TOKEN".to_string(),
                "ghp-enterprise-real".to_string(),
            ),
        ]);
        app_state.virtualize_child_credentials(&mut env);
        let github = format!("Bearer {}", env["GH_TOKEN"]);
        let token = format!("token {}", env["GH_TOKEN"]);
        let basic = format!(
            "Basic {}",
            STANDARD.encode(format!("x-access-token:{}", env["GH_TOKEN"]))
        );
        let openai = format!("Bearer {}", env["OPENAI_API_KEY"]);
        let enterprise = format!("Bearer {}", env["GH_ENTERPRISE_TOKEN"]);
        for (host, authorization, uses_cloud_credential) in [
            ("github.com", Some(github.as_str()), true),
            ("api.astemu.ghe.com", Some(github.as_str()), true),
            ("github.com", Some(token.as_str()), true),
            ("github.com", Some(basic.as_str()), true),
            ("github.com", None, false),
            ("github.com", Some("Bearer unrelated-token"), false),
            ("github.com", Some(openai.as_str()), false),
            ("enterprise.example", Some(enterprise.as_str()), false),
        ] {
            let ctx = policy_ctx(
                app_state.clone(),
                NetworkMode::Full,
                host,
                /*target_port*/ 443,
            );
            let mut req = Request::builder()
                .method(Method::GET)
                .uri("/repos/openai/codex/issues")
                .header(HOST, host)
                .body(Body::empty())
                .unwrap();
            if let Some(value) = authorization {
                req.headers_mut()
                    .insert("authorization", HeaderValue::from_str(value).unwrap());
            }
            let denied =
                uses_cloud_credential && !hooked_hosts.is_empty() && !hooked_hosts.contains(&host);
            let response = mitm_blocking_response(&req, &ctx).await.unwrap();
            assert_eq!(
                response.as_ref().map(Response::status),
                denied.then_some(StatusCode::FORBIDDEN),
                "host: {host}, hooks: {hooked_hosts:?}"
            );
            if let Some(response) = response {
                assert_eq!(response.headers()["x-proxy-error"], "blocked-by-mitm-hook");
            }
        }
    }
}

#[tokio::test]
async fn mitm_policy_checks_configured_credential_aliases_at_the_request_destination() {
    let mut hook = github_write_hook();
    hook.host = "api.hooked.example".to_string();
    hook.actions.inject_request_headers.clear();
    let mut network = NetworkProxyConfig {
        credential_broker: true,
        mitm: true,
        allow_local_binding: Some(true),
        mitm_hooks: vec![hook],
        ..NetworkProxyConfig::default()
    };
    network.credential_providers.insert(
        "custom".to_string(),
        CredentialProviderConfig {
            env: vec!["PROVIDER_TOKEN".to_string()],
            patterns: vec!["provider_[a-z]{24}".to_string()],
            // The hooked host need not use the alias's port or path prefix.
            url_prefixes: vec![
                "https://*.hooked.example/v1".to_string(),
                "https://alias.example:8443/v2".to_string(),
            ],
            ..CredentialProviderConfig::default()
        },
    );
    let app_state = Arc::new(network_proxy_state_for_policy(network));
    let mut env = HashMap::from([(
        "PROVIDER_TOKEN".to_string(),
        "provider_abcdefghijklmnopqrstuvwx".to_string(),
    )]);
    app_state.virtualize_child_credentials(&mut env);
    let ctx = policy_ctx(
        app_state,
        NetworkMode::Full,
        "alias.example",
        /*target_port*/ 8443,
    );
    for (path, denied) in [
        ("/v2/models", true),
        ("/v20/models", false),
        ("/v2/../v2/models", false),
    ] {
        let req = Request::builder()
            .uri(path)
            .header(HOST, "alias.example:8443")
            .header("authorization", format!("Bearer {}", env["PROVIDER_TOKEN"]))
            .body(Body::empty())
            .unwrap();
        let response = mitm_blocking_response(&req, &ctx).await.unwrap();
        assert_eq!(
            response.as_ref().map(Response::status),
            denied.then_some(StatusCode::FORBIDDEN),
            "path: {path}"
        );
        if let Some(response) = response {
            assert_eq!(response.headers()["x-proxy-error"], "blocked-by-mitm-hook");
        }
    }
}

#[tokio::test]
async fn mitm_policy_blocks_encoded_path_traversal_for_repository_allowlist() {
    let mut hook = github_write_hook();
    hook.host = "github.com".to_string();
    hook.matcher.methods = vec!["GET".to_string()];
    hook.matcher.path_prefixes = vec!["pattern:/openai/openai/**".to_string()];
    hook.actions.inject_request_headers.clear();
    let mut network = NetworkProxyConfig {
        mitm: true,
        mitm_hooks: vec![hook],
        mode: NetworkMode::Full,
        ..NetworkProxyConfig::default()
    };
    network.set_allowed_domains(vec!["github.com".to_string()]);
    let app_state = Arc::new(network_proxy_state_for_policy(network));
    let ctx = policy_ctx(
        app_state.clone(),
        NetworkMode::Full,
        "github.com",
        /*target_port*/ 443,
    );
    let paths = [
        "/openai/openai/issues",
        "/openai/codex",
        "/openai/openai/%2e%2e/codex",
        "/openai/openai/%2e%2e/%2e%2e/microsoft/vscode",
    ];
    let mut actual = Vec::with_capacity(paths.len());
    for path in paths {
        let req = Request::builder()
            .method(Method::GET)
            .uri(path)
            .header(HOST, "github.com")
            .body(Body::empty())
            .unwrap();
        let response = mitm_blocking_response(&req, &ctx).await.unwrap();
        actual.push(response.map(|response| {
            (
                response.status(),
                response.headers().get("x-proxy-error").cloned(),
            )
        }));
    }

    assert_eq!(
        actual,
        vec![
            None,
            Some((
                StatusCode::FORBIDDEN,
                Some(HeaderValue::from_static("blocked-by-mitm-hook")),
            )),
            Some((
                StatusCode::FORBIDDEN,
                Some(HeaderValue::from_static("blocked-by-mitm-hook")),
            )),
            Some((
                StatusCode::FORBIDDEN,
                Some(HeaderValue::from_static("blocked-by-mitm-hook")),
            )),
        ]
    );
    let blocked = app_state.drain_blocked().await.unwrap();
    assert_eq!(blocked.len(), 3);
    assert!(
        blocked
            .iter()
            .all(|request| request.reason == REASON_MITM_HOOK_DENIED)
    );
}

#[tokio::test]
async fn mitm_policy_blocks_matching_hooked_write_in_limited_mode() {
    let mut hook = github_write_hook();
    hook.actions.inject_request_headers.clear();
    let mut network = NetworkProxyConfig {
        mitm: true,
        mitm_hooks: vec![hook],
        mode: NetworkMode::Limited,
        ..NetworkProxyConfig::default()
    };
    network.set_allowed_domains(vec!["api.github.com".to_string()]);
    let app_state = Arc::new(network_proxy_state_for_policy(network));
    let ctx = policy_ctx(
        app_state.clone(),
        NetworkMode::Limited,
        "api.github.com",
        /*target_port*/ 443,
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/repos/openai/codex/issues")
        .header(HOST, "api.github.com")
        .body(Body::empty())
        .unwrap();

    let response = mitm_blocking_response(&req, &ctx)
        .await
        .unwrap()
        .expect("matching POST hook should still be blocked in limited mode");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response.headers().get("x-proxy-error").unwrap(),
        "blocked-by-method-policy"
    );

    let blocked = app_state.drain_blocked().await.unwrap();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].reason, REASON_METHOD_NOT_ALLOWED);
    assert_eq!(blocked[0].method.as_deref(), Some("POST"));
    assert_eq!(blocked[0].host, "api.github.com");
    assert_eq!(blocked[0].port, Some(443));
}

#[tokio::test]
async fn mitm_policy_blocks_hook_miss_for_hooked_host_and_records_telemetry_in_full_mode() {
    let secret_file = NamedTempFile::new().unwrap();
    std::fs::write(secret_file.path(), "ghp-secret\n").unwrap();
    let mut hook = github_write_hook();
    hook.actions.inject_request_headers[0].secret_env_var = None;
    hook.actions.inject_request_headers[0].secret_file =
        Some(secret_file.path().display().to_string());
    let mut network = NetworkProxyConfig {
        mitm: true,
        mitm_hooks: vec![hook],
        mode: NetworkMode::Full,
        ..NetworkProxyConfig::default()
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
