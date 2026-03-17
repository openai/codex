use super::*;
use crate::config::CONFIG_TOML_FILE;
use crate::config::ConfigBuilder;
use crate::plugins::PluginInstallRequest;
use crate::tools::discoverable::DiscoverablePluginInfo;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn write_file(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("file should have a parent")).unwrap();
    fs::write(path, contents).unwrap();
}

fn write_curated_plugin(root: &Path, plugin_name: &str) {
    let plugin_root = root.join("plugins").join(plugin_name);
    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        &format!(
            r#"{{
  "name": "{plugin_name}",
  "description": "Plugin that includes skills, MCP servers, and app connectors"
}}"#
        ),
    );
    write_file(
        &plugin_root.join("skills/SKILL.md"),
        "---\nname: sample\ndescription: sample\n---\n",
    );
    write_file(
        &plugin_root.join(".mcp.json"),
        r#"{
  "mcpServers": {
    "sample-docs": {
      "type": "http",
      "url": "https://sample.example/mcp"
    }
  }
}"#,
    );
    write_file(
        &plugin_root.join(".app.json"),
        r#"{
  "apps": {
    "calendar": {
      "id": "connector_calendar"
    }
  }
}"#,
    );
}

fn write_openai_curated_marketplace(root: &Path, plugin_names: &[&str]) {
    let plugins = plugin_names
        .iter()
        .map(|plugin_name| {
            format!(
                r#"{{
      "name": "{plugin_name}",
      "source": {{
        "source": "local",
        "path": "./plugins/{plugin_name}"
      }}
    }}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    write_file(
        &root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "{OPENAI_CURATED_MARKETPLACE_NAME}",
  "plugins": [
{plugins}
  ]
}}"#
        ),
    );
    for plugin_name in plugin_names {
        write_curated_plugin(root, plugin_name);
    }
}

fn write_curated_plugin_sha(codex_home: &Path) {
    write_file(
        &codex_home.join(".tmp/plugins.sha"),
        "0123456789abcdef0123456789abcdef01234567\n",
    );
}

fn write_plugins_feature_config(codex_home: &Path) {
    write_file(
        &codex_home.join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true
"#,
    );
}

async fn load_plugins_config(codex_home: &Path) -> crate::config::Config {
    ConfigBuilder::default()
        .codex_home(codex_home.to_path_buf())
        .fallback_cwd(Some(codex_home.to_path_buf()))
        .build()
        .await
        .expect("config should load")
}

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
