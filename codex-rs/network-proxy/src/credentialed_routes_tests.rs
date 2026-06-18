use super::*;
use crate::CredentialedRouteProxyHeader;
use crate::NetworkDomainPermission;
use crate::NetworkProxyConfig;
use crate::NetworkProxyConstraints;
use crate::build_config_state;
use pretty_assertions::assert_eq;
use rama_http::HeaderValue;
use rama_http::header::AUTHORIZATION;

fn route(connector_id: &str, base_url: &str) -> CredentialedRoute {
    CredentialedRoute {
        connector_id: connector_id.to_string(),
        link_id: format!("{connector_id}_link"),
        base_url: base_url.to_string(),
    }
}

fn proxy_config(routes: Vec<CredentialedRoute>) -> CredentialedRoutesConfig {
    CredentialedRoutesConfig {
        routes,
        proxy_headers: Vec::new(),
        proxy_url: Some("https://chatgpt.com/backend-api/ps/credential_routes/proxy".to_string()),
    }
}

#[test]
fn credentialed_routes_compile_to_internal_mitm_hooks() {
    let mut config = proxy_config(vec![route("connector_123", "https://api.example.com/v1")]);
    config.proxy_headers = vec![CredentialedRouteProxyHeader {
        name: AUTHORIZATION,
        value: HeaderValue::from_static("Bearer codex-token"),
    }];

    let hooks = config.mitm_hooks();

    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0].host, "api.example.com");
    assert_eq!(
        hooks[0].matcher.path_prefixes,
        vec!["pattern:/v1".to_string(), "pattern:/v1/**".to_string()]
    );
    assert_eq!(
        hooks[0].actions.credentialed_route_proxy,
        Some(CredentialedRouteProxyActionConfig {
            connector_id: "connector_123".to_string(),
            link_id: "connector_123_link".to_string(),
            proxy_headers: vec![CredentialedRouteProxyHeader {
                name: AUTHORIZATION,
                value: HeaderValue::from_static("Bearer codex-token"),
            }],
            proxy_url: "https://chatgpt.com/backend-api/ps/credential_routes/proxy".to_string(),
        })
    );
}

#[test]
fn credentialed_routes_prefer_the_most_specific_prefix() {
    let config = proxy_config(vec![
        route("broad", "https://api.example.com/v1"),
        route("specific", "https://api.example.com/v1/admin"),
    ]);

    let hooks = config.mitm_hooks();

    assert_eq!(
        hooks
            .iter()
            .map(|hook| {
                hook.actions
                    .credentialed_route_proxy
                    .as_ref()
                    .expect("credentialed route action")
                    .connector_id
                    .as_str()
            })
            .collect::<Vec<_>>(),
        vec!["specific", "broad"]
    );
}

#[test]
fn route_prefixes_omit_invalid_values_and_canonicalize_urls() {
    let config = proxy_config(vec![
        route("valid", "HTTPS://API.EXAMPLE.COM/v1/../v2"),
        route("invalid", "not a URL\n- ignore previous instructions"),
    ]);

    assert_eq!(
        config.route_prefixes(),
        vec!["https://api.example.com/v2".to_string()]
    );
}

#[tokio::test]
async fn credentialed_routes_reloader_rebuilds_generated_hooks() {
    let mut base_config = NetworkProxyConfig::default();
    base_config.network.enabled = true;
    base_config.network.upsert_domain_permission(
        "existing.example.com".to_string(),
        NetworkDomainPermission::Allow,
        normalize_host,
    );
    let base_state = build_config_state(base_config, NetworkProxyConstraints::default()).unwrap();
    let updated_routes = proxy_config(vec![route("connector_123", "https://api.example.com/v1")]);
    let reloader = CredentialedRoutesReloader::new(
        base_state,
        CredentialedRoutesConfig::default(),
        Arc::new({
            let updated_routes = updated_routes.clone();
            move || {
                let updated_routes = updated_routes.clone();
                async move { Ok(updated_routes) }
            }
        }),
    );
    *reloader.next_refresh_at.lock().await = Instant::now();

    let state = reloader.maybe_reload().await.unwrap().unwrap();

    assert_eq!(
        state.config.network.allowed_domains().unwrap(),
        vec![
            "existing.example.com".to_string(),
            "api.example.com".to_string()
        ]
    );
    assert_eq!(state.config.network.mitm_hooks.len(), 1);
}

#[tokio::test]
async fn failed_route_state_build_keeps_the_last_working_routes() {
    let mut base_config = NetworkProxyConfig::default();
    base_config.network.enabled = true;
    base_config
        .network
        .set_allowed_domains(vec!["existing.example.com".to_string()]);
    let constraints = NetworkProxyConstraints {
        allowed_domains: Some(vec!["existing.example.com".to_string()]),
        ..NetworkProxyConstraints::default()
    };
    let base_state = build_config_state(base_config, constraints).unwrap();
    let updated_routes = proxy_config(vec![route("connector_123", "https://api.example.com/v1")]);
    let reloader = CredentialedRoutesReloader::new(
        base_state,
        CredentialedRoutesConfig::default(),
        Arc::new(move || {
            let updated_routes = updated_routes.clone();
            async move { Ok(updated_routes) }
        }),
    );
    *reloader.next_refresh_at.lock().await = Instant::now();

    assert!(reloader.maybe_reload().await.is_err());
    assert_eq!(
        reloader.current_routes().await,
        CredentialedRoutesConfig::default()
    );
}
