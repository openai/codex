use super::*;
use crate::plugins::PluginInstallRequest;
use crate::plugins::test_support::load_plugins_config;
use crate::plugins::test_support::write_curated_plugin_sha;
use crate::plugins::test_support::write_file;
use crate::plugins::test_support::write_openai_curated_marketplace;
use crate::plugins::test_support::write_plugins_feature_config;
use crate::tools::discoverable::DiscoverablePluginInfo;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_returns_uninstalled_curated_plugins() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = crate::plugins::curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["sample", "slack"]);
    write_plugins_feature_config(codex_home.path());

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_tool_suggest_discoverable_plugins(&config)
        .unwrap()
        .into_iter()
        .map(DiscoverablePluginInfo::from)
        .collect::<Vec<_>>();

    assert_eq!(
        discoverable_plugins,
        vec![DiscoverablePluginInfo {
            id: "slack@openai-curated".to_string(),
            name: "slack".to_string(),
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
async fn list_tool_suggest_discoverable_plugins_returns_empty_when_plugins_feature_disabled() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = crate::plugins::curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        r#"[features]
plugins = false
"#,
    );

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_tool_suggest_discoverable_plugins(&config)
        .unwrap()
        .into_iter()
        .map(DiscoverablePluginInfo::from)
        .collect::<Vec<_>>();

    assert_eq!(discoverable_plugins, Vec::<DiscoverablePluginInfo>::new());
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_normalizes_description() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = crate::plugins::curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    write_plugins_feature_config(codex_home.path());
    write_file(
        &curated_root.join("plugins/slack/.codex-plugin/plugin.json"),
        r#"{
  "name": "slack",
  "description": "  Plugin\n   with   extra   spacing  "
}"#,
    );

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_tool_suggest_discoverable_plugins(&config)
        .unwrap()
        .into_iter()
        .map(DiscoverablePluginInfo::from)
        .collect::<Vec<_>>();

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
    let curated_root = crate::plugins::curated_plugins_repo_path(codex_home.path());
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
    let discoverable_plugins = list_tool_suggest_discoverable_plugins(&refreshed_config)
        .unwrap()
        .into_iter()
        .map(DiscoverablePluginInfo::from)
        .collect::<Vec<_>>();

    assert_eq!(discoverable_plugins, Vec::<DiscoverablePluginInfo>::new());
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_includes_configured_plugin_ids() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = crate::plugins::curated_plugins_repo_path(codex_home.path());
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
    let discoverable_plugins = list_tool_suggest_discoverable_plugins(&config)
        .unwrap()
        .into_iter()
        .map(DiscoverablePluginInfo::from)
        .collect::<Vec<_>>();

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
