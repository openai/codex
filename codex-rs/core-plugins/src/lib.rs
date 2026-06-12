mod app_bundled_internal;
mod discoverable;
pub mod installed_marketplaces;
pub mod loader;
mod manager;
pub mod manifest;
pub mod marketplace;
pub mod marketplace_add;
pub mod marketplace_remove;
pub mod marketplace_upgrade;
mod plugin_bundle_archive;
mod provider;
pub mod remote;
pub mod remote_bundle;
pub mod remote_legacy;
pub mod startup_sync;
pub mod store;
#[cfg(test)]
mod test_support;
pub mod toggles;

#[cfg(test)]
mod app_bundled_internal_tests;

pub const OPENAI_CURATED_MARKETPLACE_NAME: &str = "openai-curated";
pub const OPENAI_BUNDLED_MARKETPLACE_NAME: &str = "openai-bundled";

pub type LoadedPlugin = codex_plugin::LoadedPlugin<codex_config::McpServerConfig>;
pub type PluginLoadOutcome = codex_plugin::PluginLoadOutcome<codex_config::McpServerConfig>;

pub use discoverable::ToolSuggestDiscoverablePlugin;
pub use discoverable::ToolSuggestPluginDiscoveryInput;
pub use loader::PluginHookLoadOutcome;
pub use manager::ConfiguredMarketplace;
pub use manager::ConfiguredMarketplaceListOutcome;
pub use manager::ConfiguredMarketplacePlugin;
pub use manager::PluginDetail;
pub use manager::PluginDetailsUnavailableReason;
pub use manager::PluginInstallError;
pub use manager::PluginInstallOutcome;
pub use manager::PluginInstallRequest;
pub use manager::PluginListBackgroundTaskOptions;
pub use manager::PluginReadOutcome;
pub use manager::PluginReadRequest;
pub use manager::PluginUninstallError;
pub use manager::PluginsConfigInput;
pub use manager::PluginsManager;
pub use marketplace_upgrade::ConfiguredMarketplaceUpgradeError as PluginMarketplaceUpgradeError;
pub use marketplace_upgrade::ConfiguredMarketplaceUpgradeOutcome as PluginMarketplaceUpgradeOutcome;
pub use provider::ExecutorPluginProvider;
pub use provider::ExecutorPluginProviderError;
