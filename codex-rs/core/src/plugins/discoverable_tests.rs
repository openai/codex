use super::*;
use crate::plugins::PluginInstallRequest;
use crate::plugins::test_support::load_plugins_config;
use crate::plugins::test_support::write_curated_plugin;
use crate::plugins::test_support::write_curated_plugin_sha;
use crate::plugins::test_support::write_file;
use crate::plugins::test_support::write_openai_curated_marketplace;
use crate::plugins::test_support::write_plugins_feature_config;
use codex_tools::DiscoverablePluginInfo;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_test::internal::MockWriter;

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_returns_uninstalled_curated_plugins() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = crate::plugins::curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["sample", "slack"]);
    write_plugins_feature_config(codex_home.path());

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_tool_suggest_discoverable_plugins(&config)
        .await
        .unwrap();

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
async fn list_tool_suggest_discoverable_plugins_deduplicates_allowlisted_configured_plugin() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let plugin_id = TOOL_SUGGEST_DISCOVERABLE_PLUGIN_ALLOWLIST
        .iter()
        .copied()
        .find(|plugin_id| {
            plugin_id
                .rsplit_once('@')
                .is_some_and(|(_plugin_name, marketplace_name)| {
                    marketplace_name == OPENAI_BUNDLED_MARKETPLACE_NAME
                })
        })
        .expect("allowlist should include a bundled plugin");
    let (plugin_name, marketplace_name) = plugin_id
        .rsplit_once('@')
        .expect("plugin id should include a marketplace");
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
    let discoverable_plugins = list_tool_suggest_discoverable_plugins(&config)
        .await
        .unwrap();

    assert_eq!(discoverable_plugins.len(), 1);
    assert_eq!(discoverable_plugins[0].id, plugin_id);
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_ignores_missing_allowlisted_plugin() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = crate::plugins::curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    let marketplace_name = TOOL_SUGGEST_DISCOVERABLE_PLUGIN_ALLOWLIST
        .iter()
        .copied()
        .filter_map(|plugin_id| plugin_id.rsplit_once('@'))
        .find(|(_plugin_name, marketplace_name)| {
            *marketplace_name == OPENAI_BUNDLED_MARKETPLACE_NAME
        })
        .map(|(_plugin_name, marketplace_name)| marketplace_name)
        .expect("allowlist should include a bundled plugin");
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

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_tool_suggest_discoverable_plugins(&config)
        .await
        .unwrap();

    assert_eq!(discoverable_plugins.len(), 1);
    assert_eq!(discoverable_plugins[0].id, "slack@openai-curated");
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
        .await
        .unwrap();

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
        .await
        .unwrap();

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
        .await
        .unwrap();

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
        .await
        .unwrap();

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
    let curated_root = crate::plugins::curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(
        &curated_root,
        &["slack", "build-ios-apps", "life-science-research"],
    );
    write_plugins_feature_config(codex_home.path());

    let too_long_prompt = "x".repeat(129);
    for plugin_name in ["build-ios-apps", "life-science-research"] {
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

    let discoverable_plugins = list_tool_suggest_discoverable_plugins(&config)
        .await
        .unwrap();

    assert_eq!(discoverable_plugins.len(), 1);
    assert_eq!(discoverable_plugins[0].id, "slack@openai-curated");

    let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone())
        .expect("utf8 logs")
        .replace('\\', "/");
    assert_eq!(logs.matches("ignoring interface.defaultPrompt").count(), 2);
    let normalized_logs = logs.replace('\\', "/");
    assert_eq!(
        normalized_logs
            .matches("build-ios-apps/.codex-plugin/plugin.json")
            .count(),
        1
    );
    assert_eq!(
        normalized_logs
            .matches("life-science-research/.codex-plugin/plugin.json")
            .count(),
        1
    );
}
