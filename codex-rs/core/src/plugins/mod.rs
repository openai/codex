use crate::config::types::McpServerConfig;

mod discoverable;
mod injection;
mod manager;
mod manifest;
mod marketplace;
mod mentions;
mod remote;
mod render;
mod startup_sync;
mod store;
#[cfg(test)]
pub(crate) mod test_support;
mod toggles;

pub use codex_plugin::AppConnectorId;
pub use codex_plugin::EffectiveSkillRoots;
pub use codex_plugin::PluginCapabilitySummary;
pub use codex_plugin::PluginId;
pub use codex_plugin::PluginIdError;
pub use codex_plugin::PluginTelemetryMetadata;

pub type LoadedPlugin = codex_plugin::LoadedPlugin<McpServerConfig>;
pub type PluginLoadOutcome = codex_plugin::PluginLoadOutcome<McpServerConfig>;

pub(crate) use discoverable::list_tool_suggest_discoverable_plugins;
pub(crate) use injection::build_plugin_injections;
pub use manager::ConfiguredMarketplace;
pub use manager::ConfiguredMarketplaceListOutcome;
pub use manager::ConfiguredMarketplacePlugin;
pub use manager::OPENAI_CURATED_MARKETPLACE_NAME;
pub use manager::PluginDetail;
pub use manager::PluginInstallError;
pub use manager::PluginInstallOutcome;
pub use manager::PluginInstallRequest;
pub use manager::PluginReadOutcome;
pub use manager::PluginReadRequest;
pub use manager::PluginRemoteSyncError;
pub use manager::PluginUninstallError;
pub use manager::PluginsManager;
pub use manager::RemotePluginSyncResult;
pub use manager::installed_plugin_telemetry_metadata;
pub use manager::load_plugin_apps;
pub use manager::load_plugin_mcp_servers;
pub use manager::plugin_telemetry_metadata_from_root;
pub use manifest::PluginManifestInterface;
pub(crate) use manifest::PluginManifestPaths;
pub(crate) use manifest::load_plugin_manifest;
pub use marketplace::MarketplaceError;
pub use marketplace::MarketplaceListError;
pub use marketplace::MarketplacePluginAuthPolicy;
pub use marketplace::MarketplacePluginInstallPolicy;
pub use marketplace::MarketplacePluginPolicy;
pub use marketplace::MarketplacePluginSource;
pub use remote::RemotePluginFetchError;
pub use remote::fetch_remote_featured_plugin_ids;
pub(crate) use render::render_explicit_plugin_instructions;
pub(crate) use render::render_plugins_section;
pub(crate) use startup_sync::curated_plugins_repo_path;
pub(crate) use startup_sync::read_curated_plugins_sha;
pub(crate) use startup_sync::sync_openai_plugins_repo;
pub use toggles::collect_plugin_enabled_candidates;

pub(crate) use mentions::build_connector_slug_counts;
pub(crate) use mentions::build_skill_name_counts;
pub(crate) use mentions::collect_explicit_app_ids;
pub(crate) use mentions::collect_explicit_plugin_mentions;
pub(crate) use mentions::collect_tool_mentions_from_messages;
