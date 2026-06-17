use crate::plugins::test_support::load_plugins_config;
use crate::plugins::test_support::write_file;
use crate::plugins::test_support::write_openai_curated_marketplace;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use codex_core_plugins::remote::RemotePluginServiceConfig;
use codex_core_plugins::remote::fetch_and_cache_global_remote_plugin_catalog;
use codex_core_plugins::startup_sync::curated_plugins_repo_path;
use codex_protocol::protocol::Product;
use codex_tools::DiscoverablePluginInfo;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

async fn list_discoverable_plugins(
    config: &crate::config::Config,
    loaded_plugin_app_connector_ids: &[String],
) -> anyhow::Result<Vec<DiscoverablePluginInfo>> {
    list_discoverable_plugins_with_auth(config, /*auth*/ None, loaded_plugin_app_connector_ids)
        .await
}

async fn list_discoverable_plugins_with_auth(
    config: &crate::config::Config,
    auth: Option<&codex_login::CodexAuth>,
    loaded_plugin_app_connector_ids: &[String],
) -> anyhow::Result<Vec<DiscoverablePluginInfo>> {
    let plugins_manager = PluginsManager::new_with_options(
        config.codex_home.to_path_buf(),
        Some(Product::Codex),
        auth.map(codex_login::CodexAuth::api_auth_mode),
    );
    list_discoverable_plugins_with_manager_and_auth(
        config,
        &plugins_manager,
        auth,
        loaded_plugin_app_connector_ids,
    )
    .await
}

async fn list_discoverable_plugins_with_manager_and_auth(
    config: &crate::config::Config,
    plugins_manager: &PluginsManager,
    auth: Option<&codex_login::CodexAuth>,
    loaded_plugin_app_connector_ids: &[String],
) -> anyhow::Result<Vec<DiscoverablePluginInfo>> {
    super::list_tool_suggest_discoverable_plugins(
        config,
        plugins_manager,
        auth,
        loaded_plugin_app_connector_ids,
        codex_core_plugins::ToolSuggestPluginCatalog::RebuildAll,
    )
    .await
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_includes_cached_remote_global_plugins() {
    use codex_login::CodexAuth;
    use serde_json::json;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;
    use wiremock::matchers::query_param;

    let codex_home = tempdir().expect("tempdir should succeed");
    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        r#"[features]
plugins = true
remote_plugin = true
"#,
    );

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/list"))
        .and(query_param("scope", "GLOBAL"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [
                {
                    "id": "plugins~Plugin_remote_github",
                    "name": "github",
                    "scope": "GLOBAL",
                    "installation_policy": "AVAILABLE",
                    "authentication_policy": "ON_USE",
                    "status": "AVAILABLE",
                    "release": {
                        "display_name": "Remote GitHub",
                        "description": "Remote GitHub long",
                        "app_ids": ["github"],
                        "interface": {
                            "short_description": "Remote GitHub short",
                            "long_description": null,
                            "developer_name": null,
                            "category": null,
                            "capabilities": [],
                            "website_url": null,
                            "privacy_policy_url": null,
                            "terms_of_service_url": null,
                            "brand_color": null,
                            "default_prompt": null,
                            "composer_icon_url": null,
                            "logo_url": null,
                            "screenshot_urls": []
                        },
                        "skills": [
                            {
                                "name": "github",
                                "description": "Use GitHub",
                                "interface": null
                            }
                        ]
                    }
                },
                {
                    "id": "plugins~Plugin_remote_unlisted",
                    "name": "remote-unlisted",
                    "scope": "GLOBAL",
                    "installation_policy": "AVAILABLE",
                    "authentication_policy": "ON_USE",
                    "status": "AVAILABLE",
                    "release": {
                        "display_name": "Remote Unlisted",
                        "description": "Remote Unlisted long",
                        "app_ids": [],
                        "interface": {
                            "short_description": "Remote Unlisted short",
                            "long_description": null,
                            "developer_name": null,
                            "category": null,
                            "capabilities": [],
                            "website_url": null,
                            "privacy_policy_url": null,
                            "terms_of_service_url": null,
                            "brand_color": null,
                            "default_prompt": null,
                            "composer_icon_url": null,
                            "logo_url": null,
                            "screenshot_urls": []
                        },
                        "skills": [
                            {
                                "name": "remote-unlisted",
                                "description": "Use unlisted remote plugin",
                                "interface": null
                            }
                        ]
                    }
                },
                {
                    "id": "plugins~Plugin_remote_slack_not_available",
                    "name": "slack",
                    "scope": "GLOBAL",
                    "installation_policy": "NOT_AVAILABLE",
                    "authentication_policy": "ON_USE",
                    "status": "AVAILABLE",
                    "release": {
                        "display_name": "Remote Slack",
                        "description": "Remote Slack long",
                        "app_ids": [],
                        "interface": {
                            "short_description": "Remote Slack short",
                            "long_description": null,
                            "developer_name": null,
                            "category": null,
                            "capabilities": [],
                            "website_url": null,
                            "privacy_policy_url": null,
                            "terms_of_service_url": null,
                            "brand_color": null,
                            "default_prompt": null,
                            "composer_icon_url": null,
                            "logo_url": null,
                            "screenshot_urls": []
                        },
                        "skills": []
                    }
                },
                {
                    "id": "plugins~Plugin_remote_figma_admin_disabled",
                    "name": "figma",
                    "scope": "GLOBAL",
                    "installation_policy": "AVAILABLE",
                    "authentication_policy": "ON_USE",
                    "status": "DISABLED_BY_ADMIN",
                    "release": {
                        "display_name": "Remote Figma",
                        "description": "Remote Figma long",
                        "app_ids": [],
                        "interface": {
                            "short_description": "Remote Figma short",
                            "long_description": null,
                            "developer_name": null,
                            "category": null,
                            "capabilities": [],
                            "website_url": null,
                            "privacy_policy_url": null,
                            "terms_of_service_url": null,
                            "brand_color": null,
                            "default_prompt": null,
                            "composer_icon_url": null,
                            "logo_url": null,
                            "screenshot_urls": []
                        },
                        "skills": []
                    }
                }
            ],
            "pagination": {
                "next_page_token": null
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let mut config = load_plugins_config(codex_home.path()).await;
    config.chatgpt_base_url = format!("{}/backend-api", server.uri());
    let plugins_manager = PluginsManager::new(config.codex_home.to_path_buf());
    fetch_and_cache_global_remote_plugin_catalog(
        codex_home.path(),
        &RemotePluginServiceConfig {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
        },
        Some(&auth),
    )
    .await
    .expect("remote plugin catalog cache should write");

    let discoverable_plugins = list_discoverable_plugins_with_manager_and_auth(
        &config,
        &plugins_manager,
        Some(&auth),
        &[],
    )
    .await
    .unwrap();
    assert!(
        discoverable_plugins
            .iter()
            .all(|plugin| plugin.id != "github@openai-curated-remote")
    );

    for scope in ["GLOBAL", "USER", "WORKSPACE"] {
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/installed"))
            .and(query_param("scope", scope))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "plugins": [],
                "pagination": {
                    "next_page_token": null
                }
            })))
            .expect(1)
            .mount(&server)
            .await;
    }
    plugins_manager
        .build_and_cache_remote_installed_plugin_marketplaces(
            &config.plugins_config_input(),
            Some(&auth),
            &[REMOTE_GLOBAL_MARKETPLACE_NAME],
            /*on_effective_plugins_changed*/ None,
        )
        .await
        .expect("remote installed plugin cache should write");

    let discoverable_plugins = list_discoverable_plugins_with_manager_and_auth(
        &config,
        &plugins_manager,
        Some(&auth),
        &[],
    )
    .await
    .unwrap();
    assert_eq!(
        discoverable_plugins
            .iter()
            .filter(|plugin| plugin.id.ends_with("@openai-curated-remote"))
            .map(|plugin| plugin.id.as_str())
            .collect::<Vec<_>>(),
        vec!["github@openai-curated-remote"]
    );
    let remote_plugins = discoverable_plugins
        .into_iter()
        .filter(|plugin| plugin.id == "github@openai-curated-remote")
        .collect::<Vec<_>>();

    assert_eq!(
        remote_plugins,
        vec![DiscoverablePluginInfo {
            id: "github@openai-curated-remote".to_string(),
            remote_plugin_id: Some("plugins~Plugin_remote_github".to_string()),
            name: "Remote GitHub".to_string(),
            description: Some("Remote GitHub short".to_string()),
            has_skills: true,
            mcp_server_names: Vec::new(),
            app_connector_ids: vec!["github".to_string()],
        }]
    );

    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        r#"[features]
plugins = true
remote_plugin = true

[tool_suggest]
disabled_tools = [
  { type = "plugin", id = "github@openai-curated-remote" }
]
"#,
    );
    let mut config_with_disabled_remote_plugin = load_plugins_config(codex_home.path()).await;
    config_with_disabled_remote_plugin.chatgpt_base_url = config.chatgpt_base_url.clone();
    let discoverable_plugins = list_discoverable_plugins_with_manager_and_auth(
        &config_with_disabled_remote_plugin,
        &plugins_manager,
        Some(&auth),
        &[],
    )
    .await
    .unwrap();
    assert!(
        discoverable_plugins
            .iter()
            .all(|plugin| plugin.id != "github@openai-curated-remote")
    );
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_returns_empty_when_plugins_feature_disabled() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        r#"[features]
plugins = false
"#,
    );

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(discoverable_plugins, Vec::<DiscoverablePluginInfo>::new());
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_omits_disabled_tool_suggestions() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[tool_suggest]
disabled_tools = [
  { type = "plugin", id = "slack@openai-curated" }
]
"#,
    );

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(discoverable_plugins, Vec::<DiscoverablePluginInfo>::new());
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_includes_configured_plugin_ids() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["sample"]);
    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[tool_suggest]
discoverables = [{ type = "plugin", id = "sample@openai-curated" }]
"#,
    );

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(
        discoverable_plugins,
        vec![DiscoverablePluginInfo {
            id: "sample@openai-curated".to_string(),
            remote_plugin_id: None,
            name: "sample".to_string(),
            description: Some(
                "Plugin that includes skills, MCP servers, and app connectors".to_string(),
            ),
            has_skills: true,
            mcp_server_names: vec!["sample-docs".to_string()],
            app_connector_ids: vec!["connector_calendar".to_string()],
        }]
    );
}
