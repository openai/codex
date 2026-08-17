use crate::plugins::plugins_manager_for_config;
use crate::plugins::test_support::load_plugins_config;
use crate::plugins::test_support::write_file;
use crate::plugins::test_support::write_openai_api_curated_marketplace;
use codex_core_plugins::startup_sync::curated_plugins_repo_path;
use codex_login::test_support::auth_manager_from_optional_auth;
use codex_tools::DiscoverablePluginInfo;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

async fn list_discoverable_plugins(
    config: &crate::config::Config,
    loaded_plugin_app_connector_ids: &[String],
) -> anyhow::Result<Vec<DiscoverablePluginInfo>> {
    let plugins_manager =
        plugins_manager_for_config(config, auth_manager_from_optional_auth(/*auth*/ None));
    super::list_tool_suggest_discoverable_plugins(
        config,
        &plugins_manager,
        /*auth*/ None,
        loaded_plugin_app_connector_ids,
    )
    .await
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_returns_empty_when_plugins_feature_disabled() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_api_curated_marketplace(&curated_root, &["slack"]);
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
    write_openai_api_curated_marketplace(&curated_root, &["slack"]);
    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[tool_suggest]
disabled_tools = [
  { type = "plugin", id = "slack@openai-api-curated" }
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
    write_openai_api_curated_marketplace(&curated_root, &["sample"]);
    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[tool_suggest]
discoverables = [{ type = "plugin", id = "sample@openai-api-curated" }]
"#,
    );

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(
        discoverable_plugins,
        vec![DiscoverablePluginInfo {
            id: "sample@openai-api-curated".to_string(),
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
