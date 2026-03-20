use super::*;
use async_trait::async_trait;
use codex_app_server_protocol::ConfigLayerSource;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use tempfile::tempdir;
use toml::Value as TomlValue;

#[derive(Default)]
struct RecordingConfigPersister {
    edits: Mutex<Vec<PluginConfigEdit>>,
}

impl RecordingConfigPersister {
    fn edits(&self) -> Vec<PluginConfigEdit> {
        self.edits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[async_trait]
impl PluginConfigPersister for RecordingConfigPersister {
    async fn enable_plugin(&self, plugin_id: &str) -> anyhow::Result<()> {
        self.edits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(PluginConfigEdit::SetEnabled {
                plugin_id: plugin_id.to_string(),
                enabled: true,
            });
        Ok(())
    }

    async fn clear_plugin(&self, plugin_id: &str) -> anyhow::Result<()> {
        self.edits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(PluginConfigEdit::ClearPlugin {
                plugin_id: plugin_id.to_string(),
            });
        Ok(())
    }

    async fn apply_plugin_edits(&self, edits: &[PluginConfigEdit]) -> anyhow::Result<()> {
        self.edits
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend_from_slice(edits);
        Ok(())
    }
}

#[derive(Default)]
struct RecordingAnalyticsHook {
    installed: Mutex<Vec<PluginTelemetryMetadata>>,
    uninstalled: Mutex<Vec<PluginTelemetryMetadata>>,
}

impl RecordingAnalyticsHook {
    fn installed(&self) -> Vec<PluginTelemetryMetadata> {
        self.installed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn uninstalled(&self) -> Vec<PluginTelemetryMetadata> {
        self.uninstalled
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl PluginAnalyticsHook for RecordingAnalyticsHook {
    fn track_plugin_installed(&self, metadata: PluginTelemetryMetadata) {
        self.installed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(metadata);
    }

    fn track_plugin_uninstalled(&self, metadata: PluginTelemetryMetadata) {
        self.uninstalled
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(metadata);
    }
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent directories");
    }
    fs::write(path, contents).expect("write file");
}

fn write_marketplace_plugin(codex_home: &Path) -> AbsolutePathBuf {
    let marketplace_root = codex_home.join("marketplace");
    write_file(
        &marketplace_root.join("plugins/sample-plugin/.codex-plugin/plugin.json"),
        r#"{
  "name": "sample-plugin",
  "description": "Plugin that includes the sample MCP server and Skills"
}"#,
    );
    write_file(
        &marketplace_root.join("plugins/sample-plugin/skills/sample-search/SKILL.md"),
        "---\nname: sample-search\ndescription: search sample data\n---\n",
    );
    write_file(
        &marketplace_root.join("plugins/sample-plugin/.mcp.json"),
        r#"{
  "mcpServers": {
    "sample": {
      "type": "http",
      "url": "https://sample.example/mcp"
    }
  }
}"#,
    );
    write_file(
        &marketplace_root.join("plugins/sample-plugin/.app.json"),
        r#"{
  "apps": {
    "example": {
      "id": "connector_example"
    }
  }
}"#,
    );
    write_file(
        &marketplace_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample-plugin",
      "source": {
        "source": "local",
        "path": "./plugins/sample-plugin"
      }
    }
  ]
}"#,
    );
    AbsolutePathBuf::try_from(marketplace_root.join(".agents/plugins/marketplace.json"))
        .expect("marketplace path")
}

fn plugin_load_request(codex_home: &Path) -> PluginLoadRequest {
    let config_layer_stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: AbsolutePathBuf::from_absolute_path(codex_home.join("config.toml"))
                    .expect("absolute config path"),
            },
            toml::from_str::<TomlValue>(
                r#"
[plugins."sample-plugin@debug"]
enabled = true
"#,
            )
            .expect("parse config"),
        )],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("config layer stack");
    PluginLoadRequest {
        plugins_enabled: true,
        config_layer_stack,
    }
}

#[tokio::test]
async fn install_plugin_enables_config_tracks_analytics_and_loads_capabilities() {
    let codex_home = tempdir().expect("tempdir");
    let marketplace_path = write_marketplace_plugin(codex_home.path());
    let persister = RecordingConfigPersister::default();
    let analytics = Arc::new(RecordingAnalyticsHook::default());
    let manager = PluginsManager::new(codex_home.path().to_path_buf());
    manager.set_analytics_hook(analytics.clone());

    let outcome = manager
        .install_plugin(
            PluginInstallRequest {
                plugin_name: "sample-plugin".to_string(),
                marketplace_path,
            },
            &persister,
        )
        .await
        .expect("install plugin");

    assert_eq!(outcome.plugin_id.as_key(), "sample-plugin@debug");
    assert_eq!(
        persister.edits(),
        vec![PluginConfigEdit::SetEnabled {
            plugin_id: "sample-plugin@debug".to_string(),
            enabled: true,
        }]
    );
    assert_eq!(analytics.installed().len(), 1);

    let load_outcome = manager.plugins_for_request(&plugin_load_request(codex_home.path()));
    assert_eq!(
        load_outcome.capability_summaries(),
        &[PluginCapabilitySummary {
            config_name: "sample-plugin@debug".to_string(),
            display_name: "sample-plugin".to_string(),
            description: Some("Plugin that includes the sample MCP server and Skills".to_string(),),
            has_skills: true,
            mcp_server_names: vec!["sample".to_string()],
            app_connector_ids: vec![AppConnectorId("connector_example".to_string())],
        }]
    );
    assert_eq!(
        load_outcome.effective_skill_roots(),
        vec![
            codex_home
                .path()
                .join("plugins/cache/debug/sample-plugin/local/skills")
        ]
    );
}

#[tokio::test]
async fn uninstall_plugin_clears_config_tracks_analytics_and_removes_cache() {
    let codex_home = tempdir().expect("tempdir");
    let marketplace_path = write_marketplace_plugin(codex_home.path());
    let install_persister = RecordingConfigPersister::default();
    let uninstall_persister = RecordingConfigPersister::default();
    let analytics = Arc::new(RecordingAnalyticsHook::default());
    let manager = PluginsManager::new(codex_home.path().to_path_buf());
    manager.set_analytics_hook(analytics.clone());

    manager
        .install_plugin(
            PluginInstallRequest {
                plugin_name: "sample-plugin".to_string(),
                marketplace_path,
            },
            &install_persister,
        )
        .await
        .expect("install plugin");

    manager
        .uninstall_plugin("sample-plugin@debug".to_string(), &uninstall_persister)
        .await
        .expect("uninstall plugin");

    assert_eq!(
        uninstall_persister.edits(),
        vec![PluginConfigEdit::ClearPlugin {
            plugin_id: "sample-plugin@debug".to_string(),
        }]
    );
    assert_eq!(analytics.uninstalled().len(), 1);
    assert!(
        !codex_home
            .path()
            .join("plugins/cache/debug/sample-plugin/local")
            .exists()
    );
}
