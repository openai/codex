mod app_mcp_routing;
mod command_migration;
mod discoverable;
mod http_client_selector;
pub mod installed_marketplaces;
pub mod loader;
mod manager;
pub mod manifest;
pub mod marketplace;
pub mod marketplace_add;
mod marketplace_policy;
pub mod marketplace_remove;
pub mod marketplace_upgrade;
mod npm_source;
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
mod tool_suggest_metadata;

pub const OPENAI_CURATED_MARKETPLACE_NAME: &str = "openai-curated";
pub const OPENAI_API_CURATED_MARKETPLACE_NAME: &str = "openai-api-curated";
pub const OPENAI_BUNDLED_MARKETPLACE_NAME: &str = "openai-bundled";
pub(crate) const OPENAI_BUNDLED_ALPHA_MARKETPLACE_NAME: &str = "openai-bundled-alpha";
pub(crate) const OPENAI_PRIMARY_RUNTIME_MARKETPLACE_NAME: &str = "openai-primary-runtime";

pub fn is_openai_curated_marketplace_name(marketplace_name: &str) -> bool {
    marketplace_name == OPENAI_CURATED_MARKETPLACE_NAME
        || marketplace_name == OPENAI_API_CURATED_MARKETPLACE_NAME
}

pub type LoadedPlugin = codex_plugin::LoadedPlugin<codex_config::McpServerConfig>;
pub type PluginLoadOutcome = codex_plugin::PluginLoadOutcome<codex_config::McpServerConfig>;

pub use app_mcp_routing::apps_route_available;
pub use command_migration::CommandDescriptionMode;
pub use command_migration::CommandMigrationProfile;
pub use command_migration::RewriteProfile as CommandRewriteProfile;
pub use command_migration::count_missing_commands_with_profile;
pub use command_migration::import_commands_with_profile;
pub use command_migration::missing_command_names_with_profile;
pub use discoverable::ToolSuggestDiscoverablePlugin;
pub use discoverable::ToolSuggestPluginDiscoveryInput;
pub use loader::PluginHookLoadOutcome;
pub use manager::ConfiguredMarketplace;
pub use manager::ConfiguredMarketplaceListOutcome;
pub use manager::ConfiguredMarketplacePlugin;
pub use manager::EffectivePluginsChange;
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
pub use manager::RecommendedPluginCandidatesInput;
pub use marketplace_policy::allowed_configured_marketplace_names;
pub use marketplace_upgrade::ConfiguredMarketplaceUpgradeError as PluginMarketplaceUpgradeError;
pub use marketplace_upgrade::ConfiguredMarketplaceUpgradeOutcome as PluginMarketplaceUpgradeOutcome;
pub use provider::ExecutorPluginProvider;
pub use provider::ExecutorPluginProviderError;
pub use provider::ResolvedExecutorPlugin;
pub use remote::RecommendedPlugin;
pub use remote::RecommendedPluginsMode;
