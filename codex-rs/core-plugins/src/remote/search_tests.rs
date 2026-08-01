use super::*;
use crate::remote::REMOTE_CREATED_BY_ME_MARKETPLACE_NAME;
use crate::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use crate::remote::REMOTE_WORKSPACE_MARKETPLACE_NAME;
use crate::remote::REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME;
use crate::remote::RemotePluginShareDiscoverability;
use crate::test_support::recorded_http_client_urls;
use crate::test_support::recording_remote_plugin_service_config;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginAvailability;
use codex_app_server_protocol::PluginDisabledReason;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginInstallPolicySource;
use codex_app_server_protocol::PluginInterface;
use http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::header_exists;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;
use wiremock::matchers::query_param_is_missing;

fn remote_plugin_json(remote_plugin_id: &str, plugin_name: &str, scope: &str) -> serde_json::Value {
    let discoverability = (scope == "WORKSPACE").then_some("LISTED");
    json!({
        "id": remote_plugin_id,
        "name": plugin_name,
        "scope": scope,
        "discoverability": discoverability,
        "installation_policy": "AVAILABLE",
        "authentication_policy": "ON_USE",
        "release": {
            "display_name": plugin_name,
            "description": format!("{plugin_name} description"),
            "interface": {},
        },
    })
}

#[tokio::test]
async fn search_remote_plugins_forwards_parameters_and_converts_results() {
    let server = MockServer::start().await;
    let remote_plugin = json!({
        "id": "plugins~Plugin_linear",
        "name": "linear",
        "scope": "GLOBAL",
        "installation_policy": "NOT_AVAILABLE",
        "installation_policy_source": "WORKSPACE_SETTING",
        "must_show_installation_interstitial": true,
        "authentication_policy": "ON_INSTALL",
        "status": "DISABLED_BY_ADMIN",
        "disabled_reason": "plan_not_eligible",
        "eligible_plan_types": ["pro"],
        "release": {
            "version": "1.2.3",
            "display_name": "Linear",
            "description": "Track issues",
            "keywords": ["issues"],
            "interface": {
                "short_description": "Issue tracking",
                "category": "Productivity",
                "capabilities": ["Create issues"],
                "logo_url": "https://example.com/linear.png",
            },
        },
    });
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/search"))
        .and(query_param("q", "linear & docs/+"))
        .and(query_param("scope", "GLOBAL"))
        .and(query_param("limit", "16"))
        .and(query_param("pageToken", "next page/+"))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .and(header("oai-product-sku", "codex"))
        .and(header_exists("user-agent"))
        .and(header_exists("originator"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [remote_plugin],
            "pagination": {"next_page_token": "later page/+"},
        })))
        .expect(1)
        .mount(&server)
        .await;
    let (config, selected_urls) =
        recording_remote_plugin_service_config(format!("{}/backend-api/", server.uri()));
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

    let result = search_remote_plugins(
        &config,
        Some(&auth),
        RemotePluginSearchRequest {
            query: "linear & docs/+",
            scope: Some(RemotePluginScope::Global),
            limit: 16,
            page_token: Some("next page/+"),
        },
    )
    .await
    .expect("plugin search should succeed");

    assert_eq!(
        result,
        RemotePluginSearchPage {
            plugins: vec![RemotePluginSummary {
                id: format!("linear@{REMOTE_GLOBAL_MARKETPLACE_NAME}"),
                remote_plugin_id: "plugins~Plugin_linear".to_string(),
                version: Some("1.2.3".to_string()),
                local_version: None,
                name: "linear".to_string(),
                share_context: None,
                installed: false,
                installed_at: None,
                enabled: false,
                install_policy: PluginInstallPolicy::NotAvailable,
                install_policy_source: Some(PluginInstallPolicySource::WorkspaceSetting),
                must_show_installation_interstitial: Some(true),
                auth_policy: PluginAuthPolicy::OnInstall,
                availability: PluginAvailability::DisabledByAdmin,
                disabled_reason: Some(PluginDisabledReason::PlanNotEligible),
                eligible_plan_types: Some(vec!["pro".to_string()]),
                interface: Some(PluginInterface {
                    display_name: Some("Linear".to_string()),
                    short_description: Some("Issue tracking".to_string()),
                    long_description: None,
                    developer_name: None,
                    category: Some("Productivity".to_string()),
                    capabilities: vec!["Create issues".to_string()],
                    website_url: None,
                    privacy_policy_url: None,
                    terms_of_service_url: None,
                    default_prompt: None,
                    brand_color: None,
                    composer_icon: None,
                    composer_icon_url: None,
                    logo: None,
                    logo_dark: None,
                    logo_url: Some("https://example.com/linear.png".to_string()),
                    logo_url_dark: None,
                    screenshots: Vec::new(),
                    screenshot_urls: Vec::new(),
                }),
                keywords: vec!["issues".to_string()],
            }],
            next_page_token: Some("later page/+".to_string()),
        }
    );
    assert_eq!(
        recorded_http_client_urls(&selected_urls),
        vec![format!(
            "{}/backend-api/ps/plugins/search?q=linear+%26+docs%2F%2B&scope=GLOBAL&limit=16&pageToken=next+page%2F%2B",
            server.uri()
        )]
    );
}

#[tokio::test]
async fn search_remote_plugins_omits_optional_scope_and_page_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/search"))
        .and(query_param("q", "calendar"))
        .and(query_param("limit", "25"))
        .and(query_param_is_missing("scope"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [],
            "pagination": {"next_page_token": null},
        })))
        .expect(2)
        .mount(&server)
        .await;
    let (config, _) =
        recording_remote_plugin_service_config(format!("{}/backend-api", server.uri()));
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

    for _ in 0..2 {
        let result = search_remote_plugins(
            &config,
            Some(&auth),
            RemotePluginSearchRequest {
                query: "calendar",
                scope: None,
                limit: 25,
                page_token: None,
            },
        )
        .await
        .expect("unscoped plugin search should succeed");

        assert_eq!(
            result,
            RemotePluginSearchPage {
                plugins: Vec::new(),
                next_page_token: None,
            }
        );
    }
}

#[tokio::test]
async fn search_remote_plugins_forwards_each_supported_scope() {
    let server = MockServer::start().await;
    let (config, _) =
        recording_remote_plugin_service_config(format!("{}/backend-api", server.uri()));
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

    for (scope, expected_scope) in [
        (RemotePluginScope::Global, "GLOBAL"),
        (RemotePluginScope::User, "USER"),
        (RemotePluginScope::Workspace, "WORKSPACE"),
    ] {
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/search"))
            .and(query_param("scope", expected_scope))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "plugins": [],
                "pagination": {"next_page_token": null},
            })))
            .expect(1)
            .mount(&server)
            .await;

        search_remote_plugins(
            &config,
            Some(&auth),
            RemotePluginSearchRequest {
                query: "calendar",
                scope: Some(scope),
                limit: 16,
                page_token: None,
            },
        )
        .await
        .expect("scoped plugin search should succeed");
    }
}

#[tokio::test]
async fn search_remote_plugins_preserves_order_and_canonical_marketplaces() {
    let server = MockServer::start().await;
    let mut shared_plugin = remote_plugin_json("plugin-shared", "shared", "WORKSPACE");
    shared_plugin["discoverability"] = json!("PRIVATE");
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [
                remote_plugin_json("plugin-user", "personal", "USER"),
                remote_plugin_json("plugin-global", "global", "GLOBAL"),
                remote_plugin_json("plugin-workspace", "workspace", "WORKSPACE"),
                shared_plugin,
            ],
            "pagination": {"next_page_token": null},
        })))
        .expect(1)
        .mount(&server)
        .await;
    let (config, _) =
        recording_remote_plugin_service_config(format!("{}/backend-api", server.uri()));
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

    let result = search_remote_plugins(
        &config,
        Some(&auth),
        RemotePluginSearchRequest {
            query: "plugin",
            scope: None,
            limit: 16,
            page_token: None,
        },
    )
    .await
    .expect("mixed-scope plugin search should succeed");

    let identities = result
        .plugins
        .into_iter()
        .map(|plugin| {
            (
                plugin.id,
                plugin.remote_plugin_id,
                plugin.share_context.map(|context| context.discoverability),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        identities,
        vec![
            (
                format!("personal@{REMOTE_CREATED_BY_ME_MARKETPLACE_NAME}"),
                "plugin-user".to_string(),
                None,
            ),
            (
                format!("global@{REMOTE_GLOBAL_MARKETPLACE_NAME}"),
                "plugin-global".to_string(),
                None,
            ),
            (
                format!("workspace@{REMOTE_WORKSPACE_MARKETPLACE_NAME}"),
                "plugin-workspace".to_string(),
                Some(RemotePluginShareDiscoverability::Listed),
            ),
            (
                format!("shared@{REMOTE_WORKSPACE_SHARED_WITH_ME_MARKETPLACE_NAME}"),
                "plugin-shared".to_string(),
                Some(RemotePluginShareDiscoverability::Private),
            ),
        ]
    );
}

#[tokio::test]
async fn search_remote_plugins_requires_chatgpt_authentication() {
    let (config, selected_urls) =
        recording_remote_plugin_service_config("https://chatgpt.example/backend-api".to_string());

    let result = search_remote_plugins(
        &config,
        /*auth*/ None,
        RemotePluginSearchRequest {
            query: "calendar",
            scope: None,
            limit: 16,
            page_token: None,
        },
    )
    .await;

    assert!(matches!(
        result,
        Err(RemotePluginCatalogError::AuthRequired)
    ));
    assert_eq!(
        recorded_http_client_urls(&selected_urls),
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn search_remote_plugins_rejects_api_key_authentication() {
    let (config, selected_urls) =
        recording_remote_plugin_service_config("https://chatgpt.example/backend-api".to_string());
    let auth = CodexAuth::from_api_key("test-api-key");

    let result = search_remote_plugins(
        &config,
        Some(&auth),
        RemotePluginSearchRequest {
            query: "calendar",
            scope: Some(RemotePluginScope::Global),
            limit: 16,
            page_token: None,
        },
    )
    .await;

    assert!(matches!(
        result,
        Err(RemotePluginCatalogError::UnsupportedAuthMode)
    ));
    assert_eq!(
        recorded_http_client_urls(&selected_urls),
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn search_remote_plugins_redacts_sensitive_parameters_from_transport_errors() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("test listener should bind to a local port");
    let address = listener
        .local_addr()
        .expect("test listener should have a local address");
    let connection = std::thread::spawn(move || {
        let (stream, _) = listener
            .accept()
            .expect("test listener should accept the plugin search request");
        drop(stream);
    });
    let (config, _) =
        recording_remote_plugin_service_config(format!("http://{address}/backend-api"));
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

    let error = search_remote_plugins(
        &config,
        Some(&auth),
        RemotePluginSearchRequest {
            query: "sensitive search term",
            scope: Some(RemotePluginScope::Global),
            limit: 16,
            page_token: Some("sensitive pagination token"),
        },
    )
    .await
    .expect_err("closed connection should fail the plugin search request");
    connection
        .join()
        .expect("test listener should close the accepted connection");

    let error_message = error.to_string();
    assert!(!error_message.contains("sensitive search term"));
    assert!(!error_message.contains("sensitive pagination token"));
    let RemotePluginCatalogError::Request { url, source } = error else {
        panic!("expected transport request error");
    };
    assert_eq!(
        url,
        format!("http://{address}/backend-api/ps/plugins/search")
    );
    assert!(!source.to_string().contains("sensitive"));
}

#[tokio::test]
async fn search_remote_plugins_preserves_upstream_http_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/search"))
        .respond_with(ResponseTemplate::new(503).set_body_string("plugin search unavailable"))
        .expect(1)
        .mount(&server)
        .await;
    let (config, _) =
        recording_remote_plugin_service_config(format!("{}/backend-api", server.uri()));
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

    let result = search_remote_plugins(
        &config,
        Some(&auth),
        RemotePluginSearchRequest {
            query: "sensitive search term",
            scope: Some(RemotePluginScope::Global),
            limit: 16,
            page_token: Some("sensitive pagination token"),
        },
    )
    .await;

    let error = result.expect_err("upstream HTTP status should fail");
    let error_message = error.to_string();
    assert!(!error_message.contains("sensitive search term"));
    assert!(!error_message.contains("sensitive pagination token"));
    let RemotePluginCatalogError::UnexpectedStatus { url, status, body } = error else {
        panic!("expected upstream HTTP status error");
    };
    assert_eq!(
        (url, status, body),
        (
            format!("{}/backend-api/ps/plugins/search", server.uri()),
            StatusCode::SERVICE_UNAVAILABLE,
            "plugin search unavailable".to_string(),
        )
    );
}

#[tokio::test]
async fn search_remote_plugins_preserves_response_decode_errors() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not-json"))
        .expect(1)
        .mount(&server)
        .await;
    let (config, _) =
        recording_remote_plugin_service_config(format!("{}/backend-api", server.uri()));
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

    let result = search_remote_plugins(
        &config,
        Some(&auth),
        RemotePluginSearchRequest {
            query: "calendar",
            scope: None,
            limit: 16,
            page_token: None,
        },
    )
    .await;

    assert!(matches!(
        result,
        Err(RemotePluginCatalogError::Decode { .. })
    ));
}
