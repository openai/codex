use anyhow::Result;
use async_trait::async_trait;
use codex_config::ConfigLayerStack;

use super::PluginTelemetryMetadata;

#[derive(Debug, Clone)]
pub struct PluginLoadRequest {
    pub plugins_enabled: bool,
    pub config_layer_stack: ConfigLayerStack,
}

#[derive(Debug, Clone)]
pub struct PluginRemoteRequest {
    pub plugins_enabled: bool,
    pub config_layer_stack: ConfigLayerStack,
    pub chatgpt_base_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginConfigEdit {
    SetEnabled { plugin_id: String, enabled: bool },
    ClearPlugin { plugin_id: String },
}

#[async_trait]
pub trait PluginConfigPersister: Send + Sync {
    async fn enable_plugin(&self, plugin_id: &str) -> Result<()>;

    async fn clear_plugin(&self, plugin_id: &str) -> Result<()>;

    async fn apply_plugin_edits(&self, edits: &[PluginConfigEdit]) -> Result<()>;
}

pub trait PluginAnalyticsHook: Send + Sync {
    fn track_plugin_installed(&self, metadata: PluginTelemetryMetadata);

    fn track_plugin_uninstalled(&self, metadata: PluginTelemetryMetadata);
}
