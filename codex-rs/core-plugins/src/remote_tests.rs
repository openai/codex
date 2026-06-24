use super::*;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

#[test]
fn build_remote_marketplace_preserves_directory_order_and_appends_installed_only_plugins() {
    let directory_plugins = vec![
        directory_plugin("plugin-z", "zulu"),
        directory_plugin("plugin-m", "mike"),
    ];
    let installed_plugins = vec![RemotePluginInstalledItem {
        plugin: directory_plugin("plugin-a", "alpha"),
        enabled: true,
        disabled_skill_names: Vec::new(),
    }];

    let marketplace = build_remote_marketplace(
        "marketplace",
        "Marketplace",
        directory_plugins,
        installed_plugins,
        /*include_installed_only*/ true,
    )
    .expect("marketplace should be valid")
    .expect("marketplace should not be empty");

    assert_eq!(
        marketplace
            .plugins
            .into_iter()
            .map(|plugin| plugin.remote_plugin_id)
            .collect::<Vec<_>>(),
        vec!["plugin-z", "plugin-m", "plugin-a"]
    );
}

#[tokio::test]
async fn fetch_remote_marketplaces_retries_transient_workspace_installed_failure() {
    let server = MockServer::start().await;
    let directory_body = remote_plugin_page_body(/*enabled*/ None);
    let installed_body = remote_plugin_page_body(/*enabled*/ Some(true));
    mount_workspace_directory(
        &server,
        ResponseTemplate::new(200).set_body_string(directory_body),
    )
    .await;
    mount_workspace_installed(&server, fail_once_then_succeed(installed_body)).await;

    let marketplaces = fetch_remote_marketplaces(
        &remote_plugin_service_config(&server),
        Some(&CodexAuth::create_dummy_chatgpt_auth_for_testing()),
        &[RemoteMarketplaceSource::WorkspaceDirectory],
        /*global_catalog_cache_path*/ None,
    )
    .await
    .expect("workspace marketplace should load after retry");

    assert_eq!(marketplaces.len(), 1);
    assert_eq!(marketplaces[0].plugins.len(), 1);
    assert_eq!(marketplaces[0].plugins[0].name, "workspace-linear");
    assert!(marketplaces[0].plugins[0].installed);
    assert!(marketplaces[0].plugins[0].enabled);
}

#[tokio::test]
async fn fetch_remote_marketplaces_retries_transient_workspace_directory_failure() {
    let server = MockServer::start().await;
    let directory_body = remote_plugin_page_body(/*enabled*/ None);
    mount_workspace_directory(&server, fail_once_then_succeed(directory_body)).await;
    mount_workspace_installed(
        &server,
        ResponseTemplate::new(200).set_body_string(empty_plugin_page_body()),
    )
    .await;

    let marketplaces = fetch_remote_marketplaces(
        &remote_plugin_service_config(&server),
        Some(&CodexAuth::create_dummy_chatgpt_auth_for_testing()),
        &[RemoteMarketplaceSource::WorkspaceDirectory],
        /*global_catalog_cache_path*/ None,
    )
    .await
    .expect("workspace marketplace should load after retry");

    assert_eq!(marketplaces.len(), 1);
    assert_eq!(marketplaces[0].plugins.len(), 1);
    assert_eq!(marketplaces[0].plugins[0].name, "workspace-linear");
    assert!(!marketplaces[0].plugins[0].installed);
    assert!(!marketplaces[0].plugins[0].enabled);
}

#[test]
fn remote_plugin_catalog_get_retry_delay_uses_short_retry_after() {
    let err = RemotePluginCatalogError::UnexpectedStatus {
        url: "https://chatgpt.example/backend-api/ps/plugins/list".to_string(),
        status: reqwest::StatusCode::TOO_MANY_REQUESTS,
        body: "throttled".to_string(),
    };

    assert_eq!(
        retry_delay_for_remote_plugin_catalog_get_error(
            &err,
            RetryAfterDelay::Delay(Duration::from_millis(100))
        ),
        Some(Duration::from_millis(100))
    );
}

#[test]
fn remote_plugin_catalog_get_retry_delay_skips_long_retry_after() {
    let err = RemotePluginCatalogError::UnexpectedStatus {
        url: "https://chatgpt.example/backend-api/ps/plugins/list".to_string(),
        status: reqwest::StatusCode::TOO_MANY_REQUESTS,
        body: "throttled".to_string(),
    };

    assert_eq!(
        retry_delay_for_remote_plugin_catalog_get_error(
            &err,
            RetryAfterDelay::Delay(Duration::from_secs(1))
        ),
        None
    );
}

#[test]
fn remote_plugin_catalog_get_retry_delay_skips_unparsable_retry_after() {
    let err = RemotePluginCatalogError::UnexpectedStatus {
        url: "https://chatgpt.example/backend-api/ps/plugins/list".to_string(),
        status: reqwest::StatusCode::TOO_MANY_REQUESTS,
        body: "throttled".to_string(),
    };

    assert_eq!(
        retry_delay_for_remote_plugin_catalog_get_error(&err, RetryAfterDelay::Unparsable),
        None
    );
}

#[tokio::test]
async fn remote_plugin_catalog_get_skips_retry_when_body_fails_after_long_retry_after() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let url = format!("http://{}/catalog", listener.local_addr().unwrap());
    let attempts = Arc::new(AtomicUsize::new(0));
    let server_attempts = Arc::clone(&attempts);
    let server_task = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            server_attempts.fetch_add(1, Ordering::SeqCst);
            let response = concat!(
                "HTTP/1.1 503 Service Unavailable\r\n",
                "Retry-After: 60\r\n",
                "Content-Length: 100\r\n",
                "Connection: close\r\n",
                "\r\n",
                "partial body",
            );
            let _ = stream.write_all(response.as_bytes()).await;
        }
    });

    let request = reqwest::Client::new().get(&url);
    let err = send_and_decode_idempotent_get_with_retry::<serde_json::Value>(request, &url)
        .await
        .expect_err("truncated response should fail without retrying");
    server_task.abort();

    assert!(
        matches!(err, RemotePluginCatalogError::Request { .. }),
        "expected body read failure, got {err:?}"
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

fn directory_plugin(id: &str, name: &str) -> RemotePluginDirectoryItem {
    RemotePluginDirectoryItem {
        id: id.to_string(),
        name: name.to_string(),
        scope: RemotePluginScope::Global,
        discoverability: None,
        creator_account_user_id: None,
        creator_name: None,
        share_url: None,
        share_principals: None,
        installation_policy: PluginInstallPolicy::Available,
        authentication_policy: PluginAuthPolicy::OnUse,
        availability: PluginAvailability::Available,
        release: RemotePluginReleaseResponse {
            version: None,
            display_name: name.to_string(),
            description: String::new(),
            bundle_download_url: None,
            app_ids: Vec::new(),
            app_manifest: None,
            app_templates: Vec::new(),
            keywords: Vec::new(),
            interface: RemotePluginReleaseInterfaceResponse {
                short_description: None,
                long_description: None,
                developer_name: None,
                category: None,
                capabilities: Vec::new(),
                website_url: None,
                privacy_policy_url: None,
                terms_of_service_url: None,
                brand_color: None,
                default_prompt: None,
                default_prompts: None,
                composer_icon_url: None,
                logo_url: None,
                logo_url_dark: None,
                screenshot_urls: Vec::new(),
            },
            skills: Vec::new(),
            mcp_servers: Vec::new(),
        },
    }
}

fn remote_plugin_service_config(server: &MockServer) -> RemotePluginServiceConfig {
    RemotePluginServiceConfig {
        chatgpt_base_url: format!("{}/backend-api/", server.uri()),
    }
}

async fn mount_workspace_directory(
    server: &MockServer,
    response: impl wiremock::Respond + 'static,
) {
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/list"))
        .and(query_param("scope", "WORKSPACE"))
        .and(query_param("limit", "200"))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .respond_with(response)
        .expect(1..=2)
        .mount(server)
        .await;
}

async fn mount_workspace_installed(
    server: &MockServer,
    response: impl wiremock::Respond + 'static,
) {
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/installed"))
        .and(query_param("scope", "WORKSPACE"))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .respond_with(response)
        .expect(1..=2)
        .mount(server)
        .await;
}

fn fail_once_then_succeed(body: String) -> impl wiremock::Respond {
    let attempts = Arc::new(AtomicUsize::new(0));
    move |_request: &wiremock::Request| {
        if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
            ResponseTemplate::new(503).set_body_string("temporary failure")
        } else {
            ResponseTemplate::new(200).set_body_string(body.clone())
        }
    }
}

fn empty_plugin_page_body() -> String {
    serde_json::json!({
        "plugins": [],
        "pagination": {
            "limit": 50,
            "next_page_token": null,
        },
    })
    .to_string()
}

fn remote_plugin_page_body(enabled: Option<bool>) -> String {
    let mut plugin = serde_json::json!({
        "id": "plugins~Plugin_11111111111111111111111111111111",
        "name": "workspace-linear",
        "scope": "WORKSPACE",
        "discoverability": "LISTED",
        "installation_policy": "AVAILABLE",
        "authentication_policy": "ON_USE",
        "status": "AVAILABLE",
        "release": {
            "display_name": "Workspace Linear",
            "description": "Track workspace work",
            "app_ids": [],
            "interface": {},
            "skills": [],
        },
    });
    if let Some(enabled) = enabled {
        plugin["enabled"] = serde_json::json!(enabled);
        plugin["disabled_skill_names"] = serde_json::json!([]);
    }

    serde_json::json!({
        "plugins": [plugin],
        "pagination": {
            "limit": 50,
            "next_page_token": null,
        },
    })
    .to_string()
}

#[test]
fn remote_plugin_interface_maps_dark_logo_url() {
    let mut plugin = directory_plugin("plugin-linear", "linear");
    plugin.release.interface.logo_url_dark =
        Some("https://example.com/linear/logo-dark.png".to_string());

    assert_eq!(
        remote_plugin_interface_to_info(&plugin)
            .expect("plugin interface")
            .logo_url_dark,
        Some("https://example.com/linear/logo-dark.png".to_string())
    );
}
fn item(name: &str, display_name: &str) -> RecommendedPluginItem {
    RecommendedPluginItem {
        id: format!("plugin_{name}"),
        name: name.to_string(),
        status: None,
        installation_policy: None,
        release: RecommendedPluginRelease {
            display_name: display_name.to_string(),
            app_ids: Vec::new(),
        },
    }
}

#[test]
fn recommended_plugins_enabled_flag_selects_endpoint_or_legacy_mode() {
    let disabled: RecommendedPluginsResponse = serde_json::from_value(serde_json::json!({
        "enabled": false,
        "plugins": [{"id": "plugin_github", "name": "github", "release": {"display_name": "GitHub"}}]
    }))
    .expect("response should deserialize");
    assert_eq!(
        recommended_plugins_mode(disabled),
        RecommendedPluginsMode::Legacy
    );

    for response in [
        serde_json::json!({"plugins": []}),
        serde_json::json!({"enabled": null, "plugins": []}),
    ] {
        let response: RecommendedPluginsResponse =
            serde_json::from_value(response).expect("response should deserialize");
        assert_eq!(
            recommended_plugins_mode(response),
            RecommendedPluginsMode::Legacy
        );
    }

    let enabled: RecommendedPluginsResponse = serde_json::from_value(serde_json::json!({
        "enabled": true,
        "plugins": []
    }))
    .expect("response should deserialize");
    assert_eq!(
        recommended_plugins_mode(enabled),
        RecommendedPluginsMode::Endpoint {
            plugins: Vec::new()
        }
    );
}

#[test]
fn recommended_plugins_require_remote_install_identity() {
    let response = serde_json::from_value::<RecommendedPluginsResponse>(serde_json::json!({
        "enabled": true,
        "plugins": [{
            "name": "github",
            "release": {"display_name": "GitHub"}
        }]
    }));

    assert!(response.is_err());
}

#[test]
fn recommended_plugins_are_validated_deduplicated_sorted_and_capped() {
    let mut plugins = (0..=52)
        .rev()
        .map(|index| item(&format!("plugin-{index:02}"), &format!("Plugin {index:02}")))
        .collect::<Vec<_>>();
    plugins.push(item("plugin-00", "Duplicate"));
    plugins.push(item("not/a/plugin", "Invalid"));
    plugins.push(RecommendedPluginItem {
        id: "plugin_disabled".to_string(),
        name: "disabled".to_string(),
        status: Some(PluginAvailability::DisabledByAdmin),
        installation_policy: Some(PluginInstallPolicy::Available),
        release: RecommendedPluginRelease {
            display_name: "Disabled".to_string(),
            app_ids: Vec::new(),
        },
    });
    plugins.push(RecommendedPluginItem {
        id: "plugin_not_available".to_string(),
        name: "not-available".to_string(),
        status: Some(PluginAvailability::Available),
        installation_policy: Some(PluginInstallPolicy::NotAvailable),
        release: RecommendedPluginRelease {
            display_name: "Not Available".to_string(),
            app_ids: Vec::new(),
        },
    });

    let mode = recommended_plugins_mode(RecommendedPluginsResponse {
        enabled: Some(true),
        plugins,
    });
    let RecommendedPluginsMode::Endpoint { plugins } = mode else {
        panic!("expected endpoint mode");
    };

    assert_eq!(plugins.len(), MAX_RECOMMENDED_PLUGINS);
    assert_eq!(
        plugins.first(),
        Some(&RecommendedPlugin {
            config_id: "plugin-00@openai-curated-remote".to_string(),
            remote_plugin_id: "plugin_plugin-00".to_string(),
            display_name: "Plugin 00".to_string(),
            app_connector_ids: Vec::new(),
        })
    );
    assert_eq!(
        plugins.last(),
        Some(&RecommendedPlugin {
            config_id: "plugin-49@openai-curated-remote".to_string(),
            remote_plugin_id: "plugin_plugin-49".to_string(),
            display_name: "Plugin 49".to_string(),
            app_connector_ids: Vec::new(),
        })
    );
}

#[test]
fn recommended_plugins_bound_model_visible_fields() {
    let overlong_name = "n".repeat(MAX_RECOMMENDED_PLUGIN_NAME_LEN + 1);
    let overlong_display_name = "D".repeat(MAX_RECOMMENDED_PLUGIN_DISPLAY_NAME_LEN + 1);
    let mode = recommended_plugins_mode(RecommendedPluginsResponse {
        enabled: Some(true),
        plugins: vec![
            item(&overlong_name, "Ignored"),
            item("bounded", &overlong_display_name),
        ],
    });

    assert_eq!(
        mode,
        RecommendedPluginsMode::Endpoint {
            plugins: vec![RecommendedPlugin {
                config_id: "bounded@openai-curated-remote".to_string(),
                remote_plugin_id: "plugin_bounded".to_string(),
                display_name: "D".repeat(MAX_RECOMMENDED_PLUGIN_DISPLAY_NAME_LEN),
                app_connector_ids: Vec::new(),
            }],
        }
    );
}

#[test]
fn recommended_plugins_preserve_install_identity_and_normalize_app_ids() {
    let mode = recommended_plugins_mode(RecommendedPluginsResponse {
        enabled: Some(true),
        plugins: vec![RecommendedPluginItem {
            id: "plugin_connector_sample".to_string(),
            name: "sample".to_string(),
            status: Some(PluginAvailability::Available),
            installation_policy: Some(PluginInstallPolicy::Available),
            release: RecommendedPluginRelease {
                display_name: "Sample".to_string(),
                app_ids: vec![
                    "connector_one".to_string(),
                    String::new(),
                    "connector_two".to_string(),
                    "connector_one".to_string(),
                ],
            },
        }],
    });

    assert_eq!(
        mode,
        RecommendedPluginsMode::Endpoint {
            plugins: vec![RecommendedPlugin {
                config_id: "sample@openai-curated-remote".to_string(),
                remote_plugin_id: "plugin_connector_sample".to_string(),
                display_name: "Sample".to_string(),
                app_connector_ids: vec!["connector_one".to_string(), "connector_two".to_string(),],
            }],
        }
    );
}

#[test]
fn recommended_plugins_ignore_invalid_remote_plugin_ids() {
    let mode = recommended_plugins_mode(RecommendedPluginsResponse {
        enabled: Some(true),
        plugins: vec![RecommendedPluginItem {
            id: "not/a/plugin".to_string(),
            name: "sample".to_string(),
            status: None,
            installation_policy: None,
            release: RecommendedPluginRelease {
                display_name: "Sample".to_string(),
                app_ids: Vec::new(),
            },
        }],
    });

    assert_eq!(
        mode,
        RecommendedPluginsMode::Endpoint {
            plugins: Vec::new(),
        }
    );
}
