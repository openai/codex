use codex_config::types::McpServerConfig;

mod discoverable;
mod injection;
mod installed_marketplaces;
mod manager;
mod marketplace_add;
mod mentions;
mod render;
mod startup_sync;
#[cfg(test)]
pub(crate) mod test_support;

pub use codex_core_plugins::marketplace_upgrade::ConfiguredMarketplaceUpgradeError as PluginMarketplaceUpgradeError;
pub use codex_core_plugins::marketplace_upgrade::ConfiguredMarketplaceUpgradeOutcome as PluginMarketplaceUpgradeOutcome;
pub use codex_plugin::AppConnectorId;
pub use codex_plugin::EffectiveSkillRoots;
pub use codex_plugin::PluginCapabilitySummary;
pub use codex_plugin::PluginId;
pub use codex_plugin::PluginIdError;
pub use codex_plugin::PluginTelemetryMetadata;
pub use codex_plugin::validate_plugin_segment;

pub type LoadedPlugin = codex_plugin::LoadedPlugin<McpServerConfig>;
pub type PluginLoadOutcome = codex_plugin::PluginLoadOutcome<McpServerConfig>;

pub(crate) use codex_core_plugins::marketplace::find_marketplace_manifest_path;
pub(crate) use discoverable::list_tool_suggest_discoverable_plugins;
pub(crate) use injection::build_plugin_injections;
pub use installed_marketplaces::INSTALLED_MARKETPLACES_DIR;
pub use installed_marketplaces::marketplace_install_root;
pub use manager::ConfiguredMarketplace;
pub use manager::ConfiguredMarketplaceListOutcome;
pub use manager::ConfiguredMarketplacePlugin;
pub use manager::OPENAI_BUNDLED_MARKETPLACE_NAME;
pub use manager::OPENAI_CURATED_MARKETPLACE_NAME;
pub use manager::PluginDetail;
pub use manager::PluginDetailsUnavailableReason;
pub use manager::PluginInstallError;
pub use manager::PluginInstallOutcome;
pub use manager::PluginInstallRequest;
pub use manager::PluginReadOutcome;
pub use manager::PluginReadRequest;
pub use manager::PluginRemoteSyncError;
pub use manager::PluginUninstallError;
pub use manager::PluginsManager;
pub use manager::RemotePluginSyncResult;
pub(crate) use manager::configured_plugins_from_stack;
pub use marketplace_add::MarketplaceAddError;
pub use marketplace_add::MarketplaceAddOutcome;
pub use marketplace_add::MarketplaceAddRequest;
pub use marketplace_add::add_marketplace;
pub(crate) use marketplace_add::is_local_marketplace_source;
pub(crate) use marketplace_add::parse_marketplace_source;
pub(crate) use render::render_explicit_plugin_instructions;
pub(crate) use render::render_plugins_section;
pub(crate) use startup_sync::curated_plugins_repo_path;
pub(crate) use startup_sync::read_curated_plugins_sha;
pub(crate) use startup_sync::sync_openai_plugins_repo;

pub(crate) use mentions::build_connector_slug_counts;
pub(crate) use mentions::build_skill_name_counts;
pub(crate) use mentions::collect_explicit_app_ids;
pub(crate) use mentions::collect_explicit_plugin_mentions;
pub(crate) use mentions::collect_tool_mentions_from_messages;
