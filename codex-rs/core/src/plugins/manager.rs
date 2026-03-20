use crate::AuthManager;
use crate::CodexAuth;
use crate::analytics_client::AnalyticsEventsClient;
use crate::config::Config;
use crate::config::ConfigService;
use crate::config::edit::ConfigEdit;
use crate::config::edit::ConfigEditsBuilder;
use anyhow::Result;
use async_trait::async_trait;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::MergeStrategy;
use codex_capabilities::plugins::PluginAnalyticsHook;
use codex_capabilities::plugins::PluginConfigEdit;
use codex_capabilities::plugins::PluginConfigPersister;
use codex_capabilities::plugins::PluginLoadRequest;
use codex_capabilities::plugins::PluginRemoteRequest;
use codex_features::Feature;
use codex_protocol::protocol::Product;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use toml_edit::value;

pub use codex_capabilities::plugins::AppConnectorId;
pub use codex_capabilities::plugins::ConfiguredMarketplace;
pub use codex_capabilities::plugins::ConfiguredMarketplacePlugin;
pub use codex_capabilities::plugins::LoadedPlugin;
pub use codex_capabilities::plugins::OPENAI_CURATED_MARKETPLACE_NAME;
pub use codex_capabilities::plugins::PluginCapabilitySummary;
pub use codex_capabilities::plugins::PluginDetail;
pub use codex_capabilities::plugins::PluginInstallError;
pub use codex_capabilities::plugins::PluginInstallOutcome;
pub use codex_capabilities::plugins::PluginInstallRequest;
pub use codex_capabilities::plugins::PluginLoadOutcome;
pub use codex_capabilities::plugins::PluginReadOutcome;
pub use codex_capabilities::plugins::PluginReadRequest;
pub use codex_capabilities::plugins::PluginRemoteSyncError;
pub use codex_capabilities::plugins::PluginTelemetryMetadata;
pub use codex_capabilities::plugins::PluginUninstallError;
pub use codex_capabilities::plugins::RemotePluginSyncResult;
pub use codex_capabilities::plugins::installed_plugin_telemetry_metadata;
pub use codex_capabilities::plugins::load_plugin_apps;
pub use codex_capabilities::plugins::load_plugin_mcp_servers;
pub use codex_capabilities::plugins::plugin_telemetry_metadata_from_root;

pub struct PluginsManager {
    codex_home: PathBuf,
    inner: Arc<codex_capabilities::plugins::PluginsManager>,
}

impl PluginsManager {
    pub fn new(codex_home: PathBuf) -> Self {
        Self::new_with_restriction_product(codex_home, Some(Product::Codex))
    }

    pub fn new_with_restriction_product(
        codex_home: PathBuf,
        restriction_product: Option<Product>,
    ) -> Self {
        let inner = Arc::new(
            codex_capabilities::plugins::PluginsManager::new_with_restriction_product(
                codex_home.clone(),
                restriction_product,
            ),
        );
        Self { codex_home, inner }
    }

    pub fn set_analytics_events_client(&self, analytics_events_client: AnalyticsEventsClient) {
        self.inner
            .set_analytics_hook(Arc::new(analytics_events_client));
    }

    pub fn plugins_for_config(&self, config: &Config) -> PluginLoadOutcome {
        self.inner.plugins_for_request(&self.load_request(config))
    }

    pub fn plugins_for_config_with_force_reload(
        &self,
        config: &Config,
        force_reload: bool,
    ) -> PluginLoadOutcome {
        self.inner
            .plugins_for_request_with_force_reload(&self.load_request(config), force_reload)
    }

    pub fn clear_cache(&self) {
        self.inner.clear_cache();
    }

    pub async fn featured_plugin_ids_for_config(
        &self,
        config: &Config,
        auth: Option<&CodexAuth>,
    ) -> Result<Vec<String>, codex_capabilities::plugins::RemotePluginFetchError> {
        self.inner
            .featured_plugin_ids(&self.remote_request(config), auth)
            .await
    }

    pub async fn install_plugin(
        &self,
        request: PluginInstallRequest,
    ) -> Result<PluginInstallOutcome, PluginInstallError> {
        self.inner
            .install_plugin(
                request,
                &CorePluginConfigPersister::new(self.codex_home.clone()),
            )
            .await
    }

    pub async fn install_plugin_with_remote_sync(
        &self,
        config: &Config,
        auth: Option<&CodexAuth>,
        request: PluginInstallRequest,
    ) -> Result<PluginInstallOutcome, PluginInstallError> {
        self.inner
            .install_plugin_with_remote_sync(
                &self.remote_request(config),
                auth,
                request,
                &CorePluginConfigPersister::new(self.codex_home.clone()),
            )
            .await
    }

    pub async fn uninstall_plugin(&self, plugin_id: String) -> Result<(), PluginUninstallError> {
        self.inner
            .uninstall_plugin(
                plugin_id,
                &CorePluginConfigPersister::new(self.codex_home.clone()),
            )
            .await
    }

    pub async fn uninstall_plugin_with_remote_sync(
        &self,
        config: &Config,
        auth: Option<&CodexAuth>,
        plugin_id: String,
    ) -> Result<(), PluginUninstallError> {
        self.inner
            .uninstall_plugin_with_remote_sync(
                &self.remote_request(config),
                auth,
                plugin_id,
                &CorePluginConfigPersister::new(self.codex_home.clone()),
            )
            .await
    }

    pub async fn sync_plugins_from_remote(
        &self,
        config: &Config,
        auth: Option<&CodexAuth>,
        additive_only: bool,
    ) -> Result<RemotePluginSyncResult, PluginRemoteSyncError> {
        self.inner
            .sync_plugins_from_remote(
                &self.remote_request(config),
                auth,
                additive_only,
                &CorePluginConfigPersister::new(self.codex_home.clone()),
            )
            .await
    }

    pub fn list_marketplaces_for_config(
        &self,
        config: &Config,
        additional_roots: &[AbsolutePathBuf],
    ) -> Result<Vec<ConfiguredMarketplace>, codex_capabilities::plugins::MarketplaceError> {
        self.inner
            .list_marketplaces(&self.load_request(config), additional_roots)
    }

    pub fn read_plugin_for_config(
        &self,
        config: &Config,
        request: &PluginReadRequest,
    ) -> Result<PluginReadOutcome, codex_capabilities::plugins::MarketplaceError> {
        self.inner.read_plugin(&self.load_request(config), request)
    }

    pub fn maybe_start_plugin_startup_tasks_for_config(
        self: &Arc<Self>,
        config: &Config,
        auth_manager: Arc<AuthManager>,
    ) {
        self.inner.maybe_start_plugin_startup_tasks(
            self.remote_request(config),
            auth_manager,
            Arc::new(CorePluginConfigPersister::new(self.codex_home.clone())),
        );
    }

    pub(crate) fn inner(&self) -> &codex_capabilities::plugins::PluginsManager {
        self.inner.as_ref()
    }

    pub(crate) fn load_request(&self, config: &Config) -> PluginLoadRequest {
        PluginLoadRequest {
            plugins_enabled: config.features.enabled(Feature::Plugins),
            config_layer_stack: config.config_layer_stack.clone(),
        }
    }

    fn remote_request(&self, config: &Config) -> PluginRemoteRequest {
        PluginRemoteRequest {
            plugins_enabled: config.features.enabled(Feature::Plugins),
            config_layer_stack: config.config_layer_stack.clone(),
            chatgpt_base_url: config.chatgpt_base_url.clone(),
        }
    }
}

impl PluginAnalyticsHook for AnalyticsEventsClient {
    fn track_plugin_installed(&self, metadata: PluginTelemetryMetadata) {
        AnalyticsEventsClient::track_plugin_installed(self, metadata);
    }

    fn track_plugin_uninstalled(&self, metadata: PluginTelemetryMetadata) {
        AnalyticsEventsClient::track_plugin_uninstalled(self, metadata);
    }
}

struct CorePluginConfigPersister {
    codex_home: PathBuf,
}

impl CorePluginConfigPersister {
    fn new(codex_home: PathBuf) -> Self {
        Self { codex_home }
    }
}

#[async_trait]
impl PluginConfigPersister for CorePluginConfigPersister {
    async fn enable_plugin(&self, plugin_id: &str) -> Result<()> {
        ConfigService::new_with_defaults(self.codex_home.clone())
            .write_value(ConfigValueWriteParams {
                key_path: format!("plugins.{plugin_id}"),
                value: json!({
                    "enabled": true,
                }),
                merge_strategy: MergeStrategy::Replace,
                file_path: None,
                expected_version: None,
            })
            .await
            .map(|_| ())
            .map_err(Into::into)
    }

    async fn clear_plugin(&self, plugin_id: &str) -> Result<()> {
        ConfigEditsBuilder::new(&self.codex_home)
            .with_edits([ConfigEdit::ClearPath {
                segments: vec!["plugins".to_string(), plugin_id.to_string()],
            }])
            .apply()
            .await
            .map_err(Into::into)
    }

    async fn apply_plugin_edits(&self, edits: &[PluginConfigEdit]) -> Result<()> {
        if edits.is_empty() {
            return Ok(());
        }

        let config_edits = edits
            .iter()
            .map(|edit| match edit {
                PluginConfigEdit::SetEnabled { plugin_id, enabled } => ConfigEdit::SetPath {
                    segments: vec![
                        "plugins".to_string(),
                        plugin_id.clone(),
                        "enabled".to_string(),
                    ],
                    value: value(*enabled),
                },
                PluginConfigEdit::ClearPlugin { plugin_id } => ConfigEdit::ClearPath {
                    segments: vec!["plugins".to_string(), plugin_id.clone()],
                },
            })
            .collect::<Vec<_>>();

        ConfigEditsBuilder::new(&self.codex_home)
            .with_edits(config_edits)
            .apply()
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
