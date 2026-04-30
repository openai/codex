#[cfg(test)]
mod discoverable_tests;
mod injection;
#[cfg(test)]
mod manager_tests;
mod mentions;
#[cfg(test)]
pub(crate) mod test_support;

pub use codex_core_plugins::AppConnectorId;
pub use codex_core_plugins::EffectiveSkillRoots;
pub use codex_core_plugins::LoadedPlugin;
pub use codex_core_plugins::PluginCapabilitySummary;
pub use codex_core_plugins::PluginId;
pub use codex_core_plugins::PluginIdError;
pub use codex_core_plugins::PluginLoadOutcome;
pub use codex_core_plugins::PluginTelemetryMetadata;
pub(crate) use codex_core_plugins::discoverable::list_tool_suggest_discoverable_plugins;
pub use codex_core_plugins::manager::ConfiguredMarketplace;
pub use codex_core_plugins::manager::ConfiguredMarketplaceListOutcome;
pub use codex_core_plugins::manager::ConfiguredMarketplacePlugin;
pub use codex_core_plugins::manager::PluginDetail;
pub use codex_core_plugins::manager::PluginDetailsUnavailableReason;
pub use codex_core_plugins::manager::PluginInstallError;
pub use codex_core_plugins::manager::PluginInstallOutcome;
pub use codex_core_plugins::manager::PluginInstallRequest;
pub use codex_core_plugins::manager::PluginReadOutcome;
pub use codex_core_plugins::manager::PluginReadRequest;
pub use codex_core_plugins::manager::PluginRemoteSyncError;
pub use codex_core_plugins::manager::PluginUninstallError;
pub use codex_core_plugins::manager::PluginsManager;
pub use codex_core_plugins::manager::RemotePluginSyncResult;
pub use codex_core_plugins::marketplace_upgrade::ConfiguredMarketplaceUpgradeError as PluginMarketplaceUpgradeError;
pub use codex_core_plugins::marketplace_upgrade::ConfiguredMarketplaceUpgradeOutcome as PluginMarketplaceUpgradeOutcome;
pub(crate) use codex_core_plugins::render::render_explicit_plugin_instructions;
pub use codex_core_plugins::validate_plugin_segment;
pub(crate) use injection::build_plugin_injections;

pub(crate) use mentions::build_connector_slug_counts;
pub(crate) use mentions::build_skill_name_counts;
pub(crate) use mentions::collect_explicit_app_ids;
pub(crate) use mentions::collect_explicit_plugin_mentions;
pub(crate) use mentions::collect_tool_mentions_from_messages;

impl codex_core_plugins::manager::PluginManagerConfig for crate::config::Config {
    fn codex_home(&self) -> &std::path::Path {
        self.codex_home.as_path()
    }

    fn chatgpt_base_url(&self) -> &str {
        self.chatgpt_base_url.as_str()
    }

    fn config_layer_stack(&self) -> &codex_config::ConfigLayerStack {
        &self.config_layer_stack
    }

    fn feature_enabled(&self, feature: codex_features::Feature) -> bool {
        self.features.enabled(feature)
    }

    fn tool_suggest(&self) -> &codex_config::types::ToolSuggestConfig {
        &self.tool_suggest
    }
}
