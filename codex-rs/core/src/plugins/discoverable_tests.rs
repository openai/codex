use crate::plugins::test_support::load_plugins_config;
use crate::plugins::test_support::write_curated_plugin;
use crate::plugins::test_support::write_curated_plugin_sha;
use crate::plugins::test_support::write_file;
use crate::plugins::test_support::write_openai_curated_marketplace;
use crate::plugins::test_support::write_plugins_feature_config;
use codex_core_plugins::OPENAI_BUNDLED_MARKETPLACE_NAME;
use codex_core_plugins::PluginInstallRequest;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::remote::RemotePluginScope;
use codex_core_plugins::remote::RemotePluginServiceConfig;
use codex_core_plugins::remote::fetch_and_cache_global_remote_plugin_catalog;
use codex_core_plugins::startup_sync::curated_plugins_repo_path;
use codex_tools::DiscoverablePluginInfo;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::tempdir;
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_test::internal::MockWriter;

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
    let plugins_manager = PluginsManager::new(config.codex_home.to_path_buf());
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
    )
    .await
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_returns_fallback_plugins_without_installed_apps() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["sample", "slack", "openai-developers"]);
    write_plugins_feature_config(codex_home.path());

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(
        discoverable_plugins
            .into_iter()
            .map(|plugin| plugin.id)
            .collect::<Vec<_>>(),
        vec![
            "openai-developers@openai-curated".to_string(),
            "slack@openai-curated".to_string(),
        ]
    );
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_filters_non_fallback_by_installed_apps() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["sample", "slack", "hubspot"]);
    write_plugin_app(&curated_root, "sample", "sample", "connector_sample");
    write_plugins_feature_config(codex_home.path());
    install_marketplace_plugin(codex_home.path(), curated_root.as_path(), "slack").await;

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(
        discoverable_plugins
            .into_iter()
            .map(|plugin| plugin.id)
            .collect::<Vec<_>>(),
        vec!["hubspot@openai-curated".to_string()]
    );
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_filters_by_loaded_plugin_apps() {
    let hubspot_app_id = "asdk_app_697acb8e53d88191bf7a79e62012ae14";
    let granola_app_id = "asdk_app_697761cab6f48191b5ed345919a3ce8b";
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["hubspot", "granola"]);
    write_plugin_app(&curated_root, "hubspot", "hubspot", hubspot_app_id);
    write_plugin_app(&curated_root, "granola", "granola", granola_app_id);
    write_plugins_feature_config(codex_home.path());

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[hubspot_app_id.to_string()])
        .await
        .unwrap();

    assert_eq!(
        discoverable_plugins
            .into_iter()
            .map(|plugin| plugin.id)
            .collect::<Vec<_>>(),
        vec!["hubspot@openai-curated".to_string()]
    );
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_filters_microsoft_by_installed_apps() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(
        &curated_root,
        &["teams", "sharepoint", "outlook-email", "outlook-calendar"],
    );
    write_plugins_feature_config(codex_home.path());
    install_marketplace_plugin(codex_home.path(), curated_root.as_path(), "teams").await;

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(
        discoverable_plugins
            .into_iter()
            .map(|plugin| plugin.id)
            .collect::<Vec<_>>(),
        vec![
            "outlook-calendar@openai-curated".to_string(),
            "outlook-email@openai-curated".to_string(),
            "sharepoint@openai-curated".to_string(),
        ]
    );
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
    let remote_plugin_service_config =
        RemotePluginServiceConfig::new(config.chatgpt_base_url.clone());
    fetch_and_cache_global_remote_plugin_catalog(
        codex_home.path(),
        &remote_plugin_service_config,
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

    for scope in ["GLOBAL", "WORKSPACE"] {
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
            &[RemotePluginScope::Global],
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
async fn list_tool_suggest_discoverable_plugins_filters_sales_apps_by_marketplace() {
    let hubspot_app_id = "asdk_app_697acb8e53d88191bf7a79e62012ae14";
    let granola_app_id = "asdk_app_697761cab6f48191b5ed345919a3ce8b";
    let test_app_id = "asdk_app_test_source";
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["hubspot", "granola", "test-source"]);
    write_plugin_app(&curated_root, "hubspot", "hubspot", hubspot_app_id);
    write_plugin_app(&curated_root, "granola", "granola", granola_app_id);
    write_plugin_app(&curated_root, "test-source", "test_source", test_app_id);

    let sales_marketplace_name = "oai-maintained-plugins";
    let sales_marketplace_root = codex_home
        .path()
        .join(format!(".tmp/marketplaces/{sales_marketplace_name}"));
    write_file(
        &sales_marketplace_root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "{sales_marketplace_name}",
  "plugins": [
    {{"name": "sales", "source": {{"source": "local", "path": "./plugins/sales"}}}}
  ]
}}
"#
        ),
    );
    write_curated_plugin(&sales_marketplace_root, "sales");
    write_file(
        &sales_marketplace_root.join("plugins/sales/.app.json"),
        &format!(
            r#"{{
  "apps": {{
    "hubspot": {{
      "id": "{hubspot_app_id}"
    }},
    "granola": {{
      "id": "{granola_app_id}"
    }}
  }}
}}
"#
        ),
    );
    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        &format!(
            r#"[features]
plugins = true

[marketplaces.{sales_marketplace_name}]
source_type = "git"
source = "/tmp/{sales_marketplace_name}"
"#
        ),
    );
    install_marketplace_plugin(codex_home.path(), sales_marketplace_root.as_path(), "sales").await;

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(
        discoverable_plugins
            .into_iter()
            .map(|plugin| plugin.id)
            .collect::<Vec<_>>(),
        vec![
            "granola@openai-curated".to_string(),
            "hubspot@openai-curated".to_string(),
        ]
    );
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_omits_openai_curated_when_remote_enabled() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);

    let bundled_marketplace_name = OPENAI_BUNDLED_MARKETPLACE_NAME;
    let bundled_marketplace_root = codex_home
        .path()
        .join(format!(".tmp/marketplaces/{bundled_marketplace_name}"));
    write_file(
        &bundled_marketplace_root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "{bundled_marketplace_name}",
  "plugins": [
    {{"name": "chrome", "source": {{"source": "local", "path": "./plugins/chrome"}}}}
  ]
}}
"#
        ),
    );
    write_curated_plugin(&bundled_marketplace_root, "chrome");
    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        &format!(
            r#"[features]
plugins = true
remote_plugin = true

[marketplaces.{bundled_marketplace_name}]
source_type = "git"
source = "/tmp/{bundled_marketplace_name}"
"#
        ),
    );

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(
        discoverable_plugins
            .into_iter()
            .map(|plugin| plugin.id)
            .collect::<Vec<_>>(),
        vec!["chrome@openai-bundled".to_string()]
    );
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_deduplicates_configured_marketplace_plugin() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let plugin_name = "sample";
    let marketplace_name = OPENAI_BUNDLED_MARKETPLACE_NAME;
    let plugin_id = format!("{plugin_name}@{marketplace_name}");
    let marketplace_root = codex_home
        .path()
        .join(format!(".tmp/marketplaces/{marketplace_name}"));
    write_file(
        &marketplace_root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "{marketplace_name}",
  "plugins": [
    {{"name": "{plugin_name}", "source": {{"source": "local", "path": "./plugins/{plugin_name}"}}}}
  ]
}}
"#
        ),
    );
    write_curated_plugin(&marketplace_root, plugin_name);
    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        &format!(
            r#"[features]
plugins = true

[marketplaces.{marketplace_name}]
source_type = "git"
source = "/tmp/{marketplace_name}"

[tool_suggest]
discoverables = [{{ type = "plugin", id = "{plugin_id}" }}]
"#
        ),
    );

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(discoverable_plugins.len(), 1);
    assert_eq!(discoverable_plugins[0].id, plugin_id.as_str());
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_ignores_missing_marketplace_plugin() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["installed", "slack"]);
    let marketplace_name = OPENAI_BUNDLED_MARKETPLACE_NAME;
    let marketplace_root = codex_home
        .path()
        .join(format!(".tmp/marketplaces/{marketplace_name}"));
    write_file(
        &marketplace_root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "{marketplace_name}",
  "plugins": [
    {{"name": "sample", "source": {{"source": "local", "path": "./plugins/sample"}}}}
  ]
}}
"#
        ),
    );
    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        &format!(
            r#"[features]
plugins = true

[marketplaces.{marketplace_name}]
source_type = "git"
source = "/tmp/{marketplace_name}"
"#
        ),
    );
    install_marketplace_plugin(codex_home.path(), curated_root.as_path(), "installed").await;

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(discoverable_plugins.len(), 1);
    assert_eq!(discoverable_plugins[0].id, "slack@openai-curated");
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
async fn list_tool_suggest_discoverable_plugins_normalizes_description() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["installed", "slack"]);
    write_plugins_feature_config(codex_home.path());
    write_file(
        &curated_root.join("plugins/slack/.codex-plugin/plugin.json"),
        r#"{
  "name": "slack",
  "description": "  Plugin\n   with   extra   spacing  "
}"#,
    );
    install_marketplace_plugin(codex_home.path(), curated_root.as_path(), "installed").await;

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(
        discoverable_plugins,
        vec![DiscoverablePluginInfo {
            id: "slack@openai-curated".to_string(),
            name: "slack".to_string(),
            description: Some("Plugin with extra spacing".to_string()),
            has_skills: true,
            mcp_server_names: vec!["sample-docs".to_string()],
            app_connector_ids: vec!["connector_calendar".to_string()],
        }]
    );
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_omits_installed_curated_plugins() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    write_curated_plugin_sha(codex_home.path());
    write_plugins_feature_config(codex_home.path());

    PluginsManager::new(codex_home.path().to_path_buf())
        .install_plugin(PluginInstallRequest {
            plugin_name: "slack".to_string(),
            marketplace_path: AbsolutePathBuf::try_from(
                curated_root.join(".agents/plugins/marketplace.json"),
            )
            .expect("marketplace path"),
        })
        .await
        .expect("plugin should install");

    let refreshed_config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&refreshed_config, &[])
        .await
        .unwrap();

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
async fn list_tool_suggest_discoverable_plugins_omits_not_available_curated_plugins() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_file(
        &curated_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "openai-curated",
  "plugins": [
    {
      "name": "installed",
      "source": {
        "source": "local",
        "path": "./plugins/installed"
      }
    },
    {
      "name": "slack",
      "source": {
        "source": "local",
        "path": "./plugins/slack"
      }
    },
    {
      "name": "gmail",
      "source": {
        "source": "local",
        "path": "./plugins/gmail"
      },
      "policy": {
        "installation": "NOT_AVAILABLE"
      }
    }
  ]
}
"#,
    );
    write_curated_plugin(&curated_root, "installed");
    write_curated_plugin(&curated_root, "slack");
    write_curated_plugin(&curated_root, "gmail");
    write_plugins_feature_config(codex_home.path());
    install_marketplace_plugin(codex_home.path(), curated_root.as_path(), "installed").await;

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(
        discoverable_plugins
            .into_iter()
            .map(|plugin| plugin.id)
            .collect::<Vec<_>>(),
        vec!["slack@openai-curated".to_string()]
    );
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

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_does_not_reload_marketplace_per_plugin() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack", "gmail", "openai-developers"]);
    write_plugins_feature_config(codex_home.path());
    install_marketplace_plugin(codex_home.path(), curated_root.as_path(), "slack").await;

    let too_long_prompt = "x".repeat(129);
    for plugin_name in ["gmail", "openai-developers"] {
        write_file(
            &curated_root.join(format!("plugins/{plugin_name}/.codex-plugin/plugin.json")),
            &format!(
                r#"{{
  "name": "{plugin_name}",
  "description": "Plugin that includes skills, MCP servers, and app connectors",
  "interface": {{
    "defaultPrompt": "{too_long_prompt}"
  }}
}}"#
            ),
        );
    }

    let config = load_plugins_config(codex_home.path()).await;
    let buffer: &'static std::sync::Mutex<Vec<u8>> =
        Box::leak(Box::new(std::sync::Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .with_level(true)
        .with_ansi(false)
        .with_max_level(Level::WARN)
        .with_span_events(FmtSpan::NONE)
        .with_writer(MockWriter::new(buffer))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(
        discoverable_plugins
            .iter()
            .map(|plugin| plugin.id.as_str())
            .collect::<Vec<_>>(),
        vec!["gmail@openai-curated", "openai-developers@openai-curated"]
    );

    let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone())
        .expect("utf8 logs")
        .replace('\\', "/");
    assert_eq!(logs.matches("ignoring interface.defaultPrompt").count(), 8);
    let normalized_logs = logs.replace('\\', "/");
    assert_eq!(
        normalized_logs
            .matches("gmail/.codex-plugin/plugin.json")
            .count(),
        4
    );
    assert_eq!(
        normalized_logs
            .matches("openai-developers/.codex-plugin/plugin.json")
            .count(),
        4
    );
}

async fn install_marketplace_plugin(codex_home: &Path, marketplace_root: &Path, plugin_name: &str) {
    write_curated_plugin_sha(codex_home);
    PluginsManager::new(codex_home.to_path_buf())
        .install_plugin(PluginInstallRequest {
            plugin_name: plugin_name.to_string(),
            marketplace_path: AbsolutePathBuf::try_from(
                marketplace_root.join(".agents/plugins/marketplace.json"),
            )
            .expect("marketplace path"),
        })
        .await
        .expect("plugin should install");
}

fn write_plugin_app(root: &Path, plugin_name: &str, app_name: &str, app_id: &str) {
    write_file(
        &root.join(format!("plugins/{plugin_name}/.app.json")),
        &format!(
            r#"{{
  "apps": {{
    "{app_name}": {{
      "id": "{app_id}"
    }}
  }}
}}
"#
        ),
    );
}
