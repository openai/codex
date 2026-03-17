use super::PluginManifestPaths;
use super::curated_plugins_repo_path;
use super::load_plugin_manifest;
use super::manifest::PluginManifestInterfaceSummary;
use super::marketplace::MarketplaceError;
use super::marketplace::MarketplaceInterfaceSummary;
use super::marketplace::MarketplacePluginAuthPolicy;
use super::marketplace::MarketplacePluginInstallPolicy;
use super::marketplace::MarketplacePluginSourceSummary;
use super::marketplace::ResolvedMarketplacePlugin;
use super::marketplace::list_marketplaces;
use super::marketplace::load_marketplace_summary;
use super::marketplace::resolve_marketplace_plugin;
use super::plugin_manifest_name;
use super::plugin_manifest_paths;
use super::read_curated_plugins_sha;
use super::remote::RemotePluginFetchError;
use super::remote::RemotePluginMutationError;
use super::remote::enable_remote_plugin;
use super::remote::fetch_remote_plugin_status;
use super::remote::uninstall_remote_plugin;
use super::store::DEFAULT_PLUGIN_VERSION;
use super::store::PluginId;
use super::store::PluginIdError;
use super::store::PluginInstallResult as StorePluginInstallResult;
use super::store::PluginStore;
use super::store::PluginStoreError;
use super::sync_openai_plugins_repo;
use crate::analytics_client::AnalyticsEventsClient;
use crate::auth::CodexAuth;
use crate::config::Config;
use crate::config::ConfigService;
use crate::config::ConfigServiceError;
use crate::config::ConfigToml;
use crate::config::edit::ConfigEdit;
use crate::config::edit::ConfigEditsBuilder;
use crate::config::profile::ConfigProfile;
use crate::config::types::McpServerConfig;
use crate::config::types::PluginConfig;
use crate::config_loader::ConfigLayerStack;
use crate::features::Feature;
use crate::features::FeatureOverrides;
use crate::features::Features;
use crate::skills::SkillMetadata;
use crate::skills::loader::SkillRoot;
use crate::skills::loader::load_skills_from_roots;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::MergeStrategy;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde_json::Map as JsonMap;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use toml_edit::value;
use tracing::info;
use tracing::warn;

const DEFAULT_SKILLS_DIR_NAME: &str = "skills";
const DEFAULT_MCP_CONFIG_FILE: &str = ".mcp.json";
const DEFAULT_APP_CONFIG_FILE: &str = ".app.json";
pub const OPENAI_CURATED_MARKETPLACE_NAME: &str = "openai-curated";
static CURATED_REPO_SYNC_STARTED: AtomicBool = AtomicBool::new(false);
const MAX_CAPABILITY_SUMMARY_DESCRIPTION_LEN: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppConnectorId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInstallRequest {
    pub plugin_name: String,
    pub marketplace_path: AbsolutePathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginReadRequest {
    pub plugin_name: String,
    pub marketplace_path: AbsolutePathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInstallOutcome {
    pub plugin_id: PluginId,
    pub plugin_version: String,
    pub installed_path: AbsolutePathBuf,
    pub auth_policy: MarketplacePluginAuthPolicy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginReadOutcome {
    pub marketplace_name: String,
    pub marketplace_path: AbsolutePathBuf,
    pub plugin: PluginDetailSummary,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginDetailSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source: MarketplacePluginSourceSummary,
    pub install_policy: MarketplacePluginInstallPolicy,
    pub auth_policy: MarketplacePluginAuthPolicy,
    pub interface: Option<PluginManifestInterfaceSummary>,
    pub installed: bool,
    pub enabled: bool,
    pub skills: Vec<SkillMetadata>,
    pub apps: Vec<AppConnectorId>,
    pub mcp_server_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredMarketplaceSummary {
    pub name: String,
    pub path: AbsolutePathBuf,
    pub interface: Option<MarketplaceInterfaceSummary>,
    pub plugins: Vec<ConfiguredMarketplacePluginSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredMarketplacePluginSummary {
    pub id: String,
    pub name: String,
    pub source: MarketplacePluginSourceSummary,
    pub install_policy: MarketplacePluginInstallPolicy,
    pub auth_policy: MarketplacePluginAuthPolicy,
    pub interface: Option<PluginManifestInterfaceSummary>,
    pub installed: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedPlugin {
    pub config_name: String,
    pub manifest_name: Option<String>,
    pub manifest_description: Option<String>,
    pub root: AbsolutePathBuf,
    pub enabled: bool,
    pub skill_roots: Vec<PathBuf>,
    pub mcp_servers: HashMap<String, McpServerConfig>,
    pub apps: Vec<AppConnectorId>,
    pub error: Option<String>,
}

impl LoadedPlugin {
    fn is_active(&self) -> bool {
        self.enabled && self.error.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginCapabilitySummary {
    pub config_name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub has_skills: bool,
    pub mcp_server_names: Vec<String>,
    pub app_connector_ids: Vec<AppConnectorId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginTelemetryMetadata {
    pub plugin_id: PluginId,
    pub capability_summary: Option<PluginCapabilitySummary>,
}

impl PluginTelemetryMetadata {
    pub fn from_plugin_id(plugin_id: &PluginId) -> Self {
        Self {
            plugin_id: plugin_id.clone(),
            capability_summary: None,
        }
    }
}

impl PluginCapabilitySummary {
    fn from_plugin(plugin: &LoadedPlugin) -> Option<Self> {
        if !plugin.is_active() {
            return None;
        }

        let mut mcp_server_names: Vec<String> = plugin.mcp_servers.keys().cloned().collect();
        mcp_server_names.sort_unstable();

        let summary = Self {
            config_name: plugin.config_name.clone(),
            display_name: plugin
                .manifest_name
                .clone()
                .unwrap_or_else(|| plugin.config_name.clone()),
            description: prompt_safe_plugin_description(plugin.manifest_description.as_deref()),
            has_skills: !plugin.skill_roots.is_empty(),
            mcp_server_names,
            app_connector_ids: plugin.apps.clone(),
        };

        (summary.has_skills
            || !summary.mcp_server_names.is_empty()
            || !summary.app_connector_ids.is_empty())
        .then_some(summary)
    }

    pub fn telemetry_metadata(&self) -> Option<PluginTelemetryMetadata> {
        PluginId::parse(&self.config_name)
            .ok()
            .map(|plugin_id| PluginTelemetryMetadata {
                plugin_id,
                capability_summary: Some(self.clone()),
            })
    }
}

fn prompt_safe_plugin_description(description: Option<&str>) -> Option<String> {
    let description = description?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if description.is_empty() {
        return None;
    }

    Some(
        description
            .chars()
            .take(MAX_CAPABILITY_SUMMARY_DESCRIPTION_LEN)
            .collect(),
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginLoadOutcome {
    plugins: Vec<LoadedPlugin>,
    capability_summaries: Vec<PluginCapabilitySummary>,
}

impl Default for PluginLoadOutcome {
    fn default() -> Self {
        Self::from_plugins(Vec::new())
    }
}

impl PluginLoadOutcome {
    fn from_plugins(plugins: Vec<LoadedPlugin>) -> Self {
        let capability_summaries = plugins
            .iter()
            .filter_map(PluginCapabilitySummary::from_plugin)
            .collect::<Vec<_>>();
        Self {
            plugins,
            capability_summaries,
        }
    }

    pub fn effective_skill_roots(&self) -> Vec<PathBuf> {
        let mut skill_roots: Vec<PathBuf> = self
            .plugins
            .iter()
            .filter(|plugin| plugin.is_active())
            .flat_map(|plugin| plugin.skill_roots.iter().cloned())
            .collect();
        skill_roots.sort_unstable();
        skill_roots.dedup();
        skill_roots
    }

    pub fn effective_mcp_servers(&self) -> HashMap<String, McpServerConfig> {
        let mut mcp_servers = HashMap::new();
        for plugin in self.plugins.iter().filter(|plugin| plugin.is_active()) {
            for (name, config) in &plugin.mcp_servers {
                mcp_servers
                    .entry(name.clone())
                    .or_insert_with(|| config.clone());
            }
        }
        mcp_servers
    }

    pub fn effective_apps(&self) -> Vec<AppConnectorId> {
        let mut apps = Vec::new();
        let mut seen_connector_ids = std::collections::HashSet::new();

        for plugin in self.plugins.iter().filter(|plugin| plugin.is_active()) {
            for connector_id in &plugin.apps {
                if seen_connector_ids.insert(connector_id.clone()) {
                    apps.push(connector_id.clone());
                }
            }
        }

        apps
    }

    pub fn capability_summaries(&self) -> &[PluginCapabilitySummary] {
        &self.capability_summaries
    }

    pub fn plugins(&self) -> &[LoadedPlugin] {
        &self.plugins
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemotePluginSyncResult {
    /// Plugin ids newly installed into the local plugin cache.
    pub installed_plugin_ids: Vec<String>,
    /// Plugin ids whose local config was changed to enabled.
    pub enabled_plugin_ids: Vec<String>,
    /// Plugin ids whose local config was changed to disabled.
    /// This is not populated by `sync_plugins_from_remote`.
    pub disabled_plugin_ids: Vec<String>,
    /// Plugin ids removed from local cache or plugin config.
    pub uninstalled_plugin_ids: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginRemoteSyncError {
    #[error("chatgpt authentication required to sync remote plugins")]
    AuthRequired,

    #[error(
        "chatgpt authentication required to sync remote plugins; api key auth is not supported"
    )]
    UnsupportedAuthMode,

    #[error("failed to read auth token for remote plugin sync: {0}")]
    AuthToken(#[source] std::io::Error),

    #[error("failed to send remote plugin sync request to {url}: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("remote plugin sync request to {url} failed with status {status}: {body}")]
    UnexpectedStatus {
        url: String,
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("failed to parse remote plugin sync response from {url}: {source}")]
    Decode {
        url: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("local curated marketplace is not available")]
    LocalMarketplaceNotFound,

    #[error("remote marketplace `{marketplace_name}` is not available locally")]
    UnknownRemoteMarketplace { marketplace_name: String },

    #[error("duplicate remote plugin `{plugin_name}` in sync response")]
    DuplicateRemotePlugin { plugin_name: String },

    #[error(
        "remote plugin `{plugin_name}` was not found in local marketplace `{marketplace_name}`"
    )]
    UnknownRemotePlugin {
        plugin_name: String,
        marketplace_name: String,
    },

    #[error("{0}")]
    InvalidPluginId(#[from] PluginIdError),

    #[error("{0}")]
    Marketplace(#[from] MarketplaceError),

    #[error("{0}")]
    Store(#[from] PluginStoreError),

    #[error("{0}")]
    Config(#[from] anyhow::Error),

    #[error("failed to join remote plugin sync task: {0}")]
    Join(#[from] tokio::task::JoinError),
}

impl PluginRemoteSyncError {
    fn join(source: tokio::task::JoinError) -> Self {
        Self::Join(source)
    }
}

impl From<RemotePluginFetchError> for PluginRemoteSyncError {
    fn from(value: RemotePluginFetchError) -> Self {
        match value {
            RemotePluginFetchError::AuthRequired => Self::AuthRequired,
            RemotePluginFetchError::UnsupportedAuthMode => Self::UnsupportedAuthMode,
            RemotePluginFetchError::AuthToken(source) => Self::AuthToken(source),
            RemotePluginFetchError::Request { url, source } => Self::Request { url, source },
            RemotePluginFetchError::UnexpectedStatus { url, status, body } => {
                Self::UnexpectedStatus { url, status, body }
            }
            RemotePluginFetchError::Decode { url, source } => Self::Decode { url, source },
        }
    }
}

pub struct PluginsManager {
    codex_home: PathBuf,
    store: PluginStore,
    cache_by_cwd: RwLock<HashMap<PathBuf, PluginLoadOutcome>>,
    analytics_events_client: RwLock<Option<AnalyticsEventsClient>>,
}

impl PluginsManager {
    pub fn new(codex_home: PathBuf) -> Self {
        Self {
            codex_home: codex_home.clone(),
            store: PluginStore::new(codex_home),
            cache_by_cwd: RwLock::new(HashMap::new()),
            analytics_events_client: RwLock::new(None),
        }
    }

    pub fn set_analytics_events_client(&self, analytics_events_client: AnalyticsEventsClient) {
        let mut stored_client = match self.analytics_events_client.write() {
            Ok(client_guard) => client_guard,
            Err(err) => err.into_inner(),
        };
        *stored_client = Some(analytics_events_client);
    }

    pub fn plugins_for_config(&self, config: &Config) -> PluginLoadOutcome {
        self.plugins_for_layer_stack(
            &config.cwd,
            &config.config_layer_stack,
            /*force_reload*/ false,
        )
    }

    pub fn plugins_for_layer_stack(
        &self,
        cwd: &Path,
        config_layer_stack: &ConfigLayerStack,
        force_reload: bool,
    ) -> PluginLoadOutcome {
        if !plugins_feature_enabled_from_stack(config_layer_stack) {
            return PluginLoadOutcome::default();
        }

        if !force_reload && let Some(outcome) = self.cached_outcome_for_cwd(cwd) {
            return outcome;
        }

        let outcome = load_plugins_from_layer_stack(config_layer_stack, &self.store);
        log_plugin_load_errors(&outcome);
        let mut cache = match self.cache_by_cwd.write() {
            Ok(cache) => cache,
            Err(err) => err.into_inner(),
        };
        cache.insert(cwd.to_path_buf(), outcome.clone());
        outcome
    }

    pub fn clear_cache(&self) {
        let mut cache_by_cwd = match self.cache_by_cwd.write() {
            Ok(cache) => cache,
            Err(err) => err.into_inner(),
        };
        cache_by_cwd.clear();
    }

    fn cached_outcome_for_cwd(&self, cwd: &Path) -> Option<PluginLoadOutcome> {
        match self.cache_by_cwd.read() {
            Ok(cache) => cache.get(cwd).cloned(),
            Err(err) => err.into_inner().get(cwd).cloned(),
        }
    }

    pub async fn install_plugin(
        &self,
        request: PluginInstallRequest,
    ) -> Result<PluginInstallOutcome, PluginInstallError> {
        let resolved = resolve_marketplace_plugin(&request.marketplace_path, &request.plugin_name)?;
        self.install_resolved_plugin(resolved).await
    }

    pub async fn install_plugin_with_remote_sync(
        &self,
        config: &Config,
        auth: Option<&CodexAuth>,
        request: PluginInstallRequest,
    ) -> Result<PluginInstallOutcome, PluginInstallError> {
        let resolved = resolve_marketplace_plugin(&request.marketplace_path, &request.plugin_name)?;
        let plugin_id = resolved.plugin_id.as_key();
        // This only forwards the backend mutation before the local install flow. We rely on
        // `plugin/list(forceRemoteSync=true)` to sync local state rather than doing an extra
        // reconcile pass here.
        enable_remote_plugin(config, auth, &plugin_id)
            .await
            .map_err(PluginInstallError::from)?;
        self.install_resolved_plugin(resolved).await
    }

    async fn install_resolved_plugin(
        &self,
        resolved: ResolvedMarketplacePlugin,
    ) -> Result<PluginInstallOutcome, PluginInstallError> {
        let auth_policy = resolved.auth_policy;
        let plugin_version =
            if resolved.plugin_id.marketplace_name == OPENAI_CURATED_MARKETPLACE_NAME {
                Some(
                    read_curated_plugins_sha(self.codex_home.as_path()).ok_or_else(|| {
                        PluginStoreError::Invalid(
                            "local curated marketplace sha is not available".to_string(),
                        )
                    })?,
                )
            } else {
                None
            };
        let store = self.store.clone();
        let result: StorePluginInstallResult = tokio::task::spawn_blocking(move || {
            if let Some(plugin_version) = plugin_version {
                store.install_with_version(resolved.source_path, resolved.plugin_id, plugin_version)
            } else {
                store.install(resolved.source_path, resolved.plugin_id)
            }
        })
        .await
        .map_err(PluginInstallError::join)??;

        ConfigService::new_with_defaults(self.codex_home.clone())
            .write_value(ConfigValueWriteParams {
                key_path: format!("plugins.{}", result.plugin_id.as_key()),
                value: json!({
                    "enabled": true,
                }),
                merge_strategy: MergeStrategy::Replace,
                file_path: None,
                expected_version: None,
            })
            .await
            .map(|_| ())
            .map_err(PluginInstallError::from)?;

        let analytics_events_client = match self.analytics_events_client.read() {
            Ok(client) => client.clone(),
            Err(err) => err.into_inner().clone(),
        };
        if let Some(analytics_events_client) = analytics_events_client {
            analytics_events_client.track_plugin_installed(plugin_telemetry_metadata_from_root(
                &result.plugin_id,
                result.installed_path.as_path(),
            ));
        }

        Ok(PluginInstallOutcome {
            plugin_id: result.plugin_id,
            plugin_version: result.plugin_version,
            installed_path: result.installed_path,
            auth_policy,
        })
    }

    pub async fn uninstall_plugin(&self, plugin_id: String) -> Result<(), PluginUninstallError> {
        let plugin_id = PluginId::parse(&plugin_id)?;
        self.uninstall_plugin_id(plugin_id).await
    }

    pub async fn uninstall_plugin_with_remote_sync(
        &self,
        config: &Config,
        auth: Option<&CodexAuth>,
        plugin_id: String,
    ) -> Result<(), PluginUninstallError> {
        let plugin_id = PluginId::parse(&plugin_id)?;
        let plugin_key = plugin_id.as_key();
        // This only forwards the backend mutation before the local uninstall flow. We rely on
        // `plugin/list(forceRemoteSync=true)` to sync local state rather than doing an extra
        // reconcile pass here.
        uninstall_remote_plugin(config, auth, &plugin_key)
            .await
            .map_err(PluginUninstallError::from)?;
        self.uninstall_plugin_id(plugin_id).await
    }

    async fn uninstall_plugin_id(&self, plugin_id: PluginId) -> Result<(), PluginUninstallError> {
        let plugin_telemetry = self
            .store
            .active_plugin_root(&plugin_id)
            .map(|_| installed_plugin_telemetry_metadata(self.codex_home.as_path(), &plugin_id));
        let store = self.store.clone();
        let plugin_id_for_store = plugin_id.clone();
        tokio::task::spawn_blocking(move || store.uninstall(&plugin_id_for_store))
            .await
            .map_err(PluginUninstallError::join)??;

        ConfigEditsBuilder::new(&self.codex_home)
            .with_edits([ConfigEdit::ClearPath {
                segments: vec!["plugins".to_string(), plugin_id.as_key()],
            }])
            .apply()
            .await?;

        let analytics_events_client = match self.analytics_events_client.read() {
            Ok(client) => client.clone(),
            Err(err) => err.into_inner().clone(),
        };
        if let Some(plugin_telemetry) = plugin_telemetry
            && let Some(analytics_events_client) = analytics_events_client
        {
            analytics_events_client.track_plugin_uninstalled(plugin_telemetry);
        }

        Ok(())
    }

    pub async fn sync_plugins_from_remote(
        &self,
        config: &Config,
        auth: Option<&CodexAuth>,
    ) -> Result<RemotePluginSyncResult, PluginRemoteSyncError> {
        info!("starting remote plugin sync");
        let remote_plugins = fetch_remote_plugin_status(config, auth)
            .await
            .map_err(PluginRemoteSyncError::from)?;
        let configured_plugins = configured_plugins_from_stack(&config.config_layer_stack);
        let curated_marketplace_root = curated_plugins_repo_path(self.codex_home.as_path());
        let curated_marketplace_path = AbsolutePathBuf::try_from(
            curated_marketplace_root.join(".agents/plugins/marketplace.json"),
        )
        .map_err(|_| PluginRemoteSyncError::LocalMarketplaceNotFound)?;
        let curated_marketplace = match load_marketplace_summary(&curated_marketplace_path) {
            Ok(marketplace) => marketplace,
            Err(MarketplaceError::MarketplaceNotFound { .. }) => {
                return Err(PluginRemoteSyncError::LocalMarketplaceNotFound);
            }
            Err(err) => return Err(err.into()),
        };

        let marketplace_name = curated_marketplace.name.clone();
        let curated_plugin_version = read_curated_plugins_sha(self.codex_home.as_path())
            .ok_or_else(|| {
                PluginStoreError::Invalid(
                    "local curated marketplace sha is not available".to_string(),
                )
            })?;
        let mut local_plugins = Vec::<(
            String,
            PluginId,
            AbsolutePathBuf,
            Option<bool>,
            Option<String>,
        )>::new();
        let mut local_plugin_names = HashSet::new();
        for plugin in curated_marketplace.plugins {
            let plugin_name = plugin.name;
            if !local_plugin_names.insert(plugin_name.clone()) {
                warn!(
                    plugin = plugin_name,
                    marketplace = %marketplace_name,
                    "ignoring duplicate local plugin entry during remote sync"
                );
                continue;
            }

            let plugin_id = PluginId::new(plugin_name.clone(), marketplace_name.clone())?;
            let plugin_key = plugin_id.as_key();
            let source_path = match plugin.source {
                MarketplacePluginSourceSummary::Local { path } => path,
            };
            let current_enabled = configured_plugins
                .get(&plugin_key)
                .map(|plugin| plugin.enabled);
            let installed_version = self.store.active_plugin_version(&plugin_id);
            local_plugins.push((
                plugin_name,
                plugin_id,
                source_path,
                current_enabled,
                installed_version,
            ));
        }

        let mut remote_installed_plugin_names = HashSet::<String>::new();
        for plugin in remote_plugins {
            if plugin.marketplace_name != marketplace_name {
                return Err(PluginRemoteSyncError::UnknownRemoteMarketplace {
                    marketplace_name: plugin.marketplace_name,
                });
            }
            if !local_plugin_names.contains(&plugin.name) {
                warn!(
                    plugin = plugin.name,
                    marketplace = %marketplace_name,
                    "ignoring remote plugin missing from local marketplace during sync"
                );
                continue;
            }
            // For now, sync treats remote `enabled = false` as uninstall rather than a distinct
            // disabled state.
            // TODO: Switch sync to `plugins/installed` so install and enable states stay distinct.
            if !plugin.enabled {
                continue;
            }
            if !remote_installed_plugin_names.insert(plugin.name.clone()) {
                return Err(PluginRemoteSyncError::DuplicateRemotePlugin {
                    plugin_name: plugin.name,
                });
            }
        }

        let mut config_edits = Vec::new();
        let mut installs = Vec::new();
        let mut uninstalls = Vec::new();
        let mut result = RemotePluginSyncResult::default();
        let remote_plugin_count = remote_installed_plugin_names.len();
        let local_plugin_count = local_plugins.len();

        for (plugin_name, plugin_id, source_path, current_enabled, installed_version) in
            local_plugins
        {
            let plugin_key = plugin_id.as_key();
            let is_installed = installed_version.is_some();
            if remote_installed_plugin_names.contains(&plugin_name) {
                if !is_installed {
                    installs.push((
                        source_path,
                        plugin_id.clone(),
                        curated_plugin_version.clone(),
                    ));
                }
                if !is_installed {
                    result.installed_plugin_ids.push(plugin_key.clone());
                }

                if current_enabled != Some(true) {
                    result.enabled_plugin_ids.push(plugin_key.clone());
                    config_edits.push(ConfigEdit::SetPath {
                        segments: vec!["plugins".to_string(), plugin_key, "enabled".to_string()],
                        value: value(true),
                    });
                }
            } else {
                if is_installed {
                    uninstalls.push(plugin_id);
                }
                if is_installed || current_enabled.is_some() {
                    result.uninstalled_plugin_ids.push(plugin_key.clone());
                }
                if current_enabled.is_some() {
                    config_edits.push(ConfigEdit::ClearPath {
                        segments: vec!["plugins".to_string(), plugin_key],
                    });
                }
            }
        }

        let store = self.store.clone();
        let store_result = tokio::task::spawn_blocking(move || {
            for (source_path, plugin_id, plugin_version) in installs {
                store.install_with_version(source_path, plugin_id, plugin_version)?;
            }
            for plugin_id in uninstalls {
                store.uninstall(&plugin_id)?;
            }
            Ok::<(), PluginStoreError>(())
        })
        .await
        .map_err(PluginRemoteSyncError::join)?;
        if let Err(err) = store_result {
            self.clear_cache();
            return Err(err.into());
        }

        let config_result = if config_edits.is_empty() {
            Ok(())
        } else {
            ConfigEditsBuilder::new(&self.codex_home)
                .with_edits(config_edits)
                .apply()
                .await
        };
        self.clear_cache();
        config_result?;

        info!(
            marketplace = %marketplace_name,
            remote_plugin_count,
            local_plugin_count,
            installed_plugin_ids = ?result.installed_plugin_ids,
            enabled_plugin_ids = ?result.enabled_plugin_ids,
            disabled_plugin_ids = ?result.disabled_plugin_ids,
            uninstalled_plugin_ids = ?result.uninstalled_plugin_ids,
            "completed remote plugin sync"
        );

        Ok(result)
    }

    pub fn list_marketplaces_for_config(
        &self,
        config: &Config,
        additional_roots: &[AbsolutePathBuf],
    ) -> Result<Vec<ConfiguredMarketplaceSummary>, MarketplaceError> {
        let (installed_plugins, configured_plugins) = self.configured_plugin_states(config);
        let marketplaces = list_marketplaces(&self.marketplace_roots(additional_roots))?;
        let mut seen_plugin_keys = HashSet::new();

        Ok(marketplaces
            .into_iter()
            .filter_map(|marketplace| {
                let marketplace_name = marketplace.name.clone();
                let plugins = marketplace
                    .plugins
                    .into_iter()
                    .filter_map(|plugin| {
                        let plugin_key = format!("{}@{marketplace_name}", plugin.name);
                        if !seen_plugin_keys.insert(plugin_key.clone()) {
                            return None;
                        }

                        Some(ConfiguredMarketplacePluginSummary {
                            // Enabled state is keyed by `<plugin>@<marketplace>`, so duplicate
                            // plugin entries from duplicate marketplace files intentionally
                            // resolve to the first discovered source.
                            id: plugin_key.clone(),
                            installed: installed_plugins.contains(&plugin_key),
                            enabled: configured_plugins
                                .get(&plugin_key)
                                .copied()
                                .unwrap_or(false),
                            name: plugin.name,
                            source: plugin.source,
                            install_policy: plugin.install_policy,
                            auth_policy: plugin.auth_policy,
                            interface: plugin.interface,
                        })
                    })
                    .collect::<Vec<_>>();

                (!plugins.is_empty()).then_some(ConfiguredMarketplaceSummary {
                    name: marketplace.name,
                    path: marketplace.path,
                    interface: marketplace.interface,
                    plugins,
                })
            })
            .collect())
    }

    pub fn read_plugin_for_config(
        &self,
        config: &Config,
        request: &PluginReadRequest,
    ) -> Result<PluginReadOutcome, MarketplaceError> {
        let marketplace = load_marketplace_summary(&request.marketplace_path)?;
        let marketplace_name = marketplace.name.clone();
        let plugin = marketplace
            .plugins
            .into_iter()
            .find(|plugin| plugin.name == request.plugin_name);
        let Some(plugin) = plugin else {
            return Err(MarketplaceError::PluginNotFound {
                plugin_name: request.plugin_name.clone(),
                marketplace_name,
            });
        };

        let plugin_id = PluginId::new(plugin.name.clone(), marketplace.name.clone()).map_err(
            |err| match err {
                PluginIdError::Invalid(message) => MarketplaceError::InvalidPlugin(message),
            },
        )?;
        let plugin_key = plugin_id.as_key();
        let (installed_plugins, configured_plugins) = self.configured_plugin_states(config);
        let source_path = match &plugin.source {
            MarketplacePluginSourceSummary::Local { path } => path.clone(),
        };
        let manifest = load_plugin_manifest(source_path.as_path()).ok_or_else(|| {
            MarketplaceError::InvalidPlugin(
                "missing or invalid .codex-plugin/plugin.json".to_string(),
            )
        })?;
        let description = manifest.description.clone();
        let manifest_paths = plugin_manifest_paths(&manifest, source_path.as_path());
        let skill_roots = plugin_skill_roots(source_path.as_path(), &manifest_paths);
        let skills = load_skills_from_roots(skill_roots.into_iter().map(|path| SkillRoot {
            path,
            scope: SkillScope::User,
        }))
        .skills;
        let apps = load_plugin_apps(source_path.as_path());
        let mcp_config_paths = plugin_mcp_config_paths(source_path.as_path(), &manifest_paths);
        let mut mcp_server_names = Vec::new();
        for mcp_config_path in mcp_config_paths {
            mcp_server_names.extend(
                load_mcp_servers_from_file(source_path.as_path(), &mcp_config_path)
                    .mcp_servers
                    .into_keys(),
            );
        }
        mcp_server_names.sort_unstable();
        mcp_server_names.dedup();

        Ok(PluginReadOutcome {
            marketplace_name: marketplace.name,
            marketplace_path: marketplace.path,
            plugin: PluginDetailSummary {
                id: plugin_key.clone(),
                name: plugin.name,
                description,
                source: plugin.source,
                install_policy: plugin.install_policy,
                auth_policy: plugin.auth_policy,
                interface: plugin.interface,
                installed: installed_plugins.contains(&plugin_key),
                enabled: configured_plugins
                    .get(&plugin_key)
                    .copied()
                    .unwrap_or(false),
                skills,
                apps,
                mcp_server_names,
            },
        })
    }

    pub fn maybe_start_curated_repo_sync_for_config(self: &Arc<Self>, config: &Config) {
        if plugins_feature_enabled_from_stack(&config.config_layer_stack) {
            let mut configured_curated_plugin_ids =
                configured_plugins_from_stack(&config.config_layer_stack)
                    .into_keys()
                    .filter_map(|plugin_key| match PluginId::parse(&plugin_key) {
                        Ok(plugin_id)
                            if plugin_id.marketplace_name == OPENAI_CURATED_MARKETPLACE_NAME =>
                        {
                            Some(plugin_id)
                        }
                        Ok(_) => None,
                        Err(err) => {
                            warn!(
                                plugin_key,
                                error = %err,
                                "ignoring invalid configured plugin key during curated sync setup"
                            );
                            None
                        }
                    })
                    .collect::<Vec<_>>();
            configured_curated_plugin_ids.sort_unstable_by_key(super::store::PluginId::as_key);
            self.start_curated_repo_sync(configured_curated_plugin_ids);
        }
    }

    fn start_curated_repo_sync(self: &Arc<Self>, configured_curated_plugin_ids: Vec<PluginId>) {
        if CURATED_REPO_SYNC_STARTED.swap(true, Ordering::SeqCst) {
            return;
        }
        let manager = Arc::clone(self);
        let codex_home = self.codex_home.clone();
        if let Err(err) = std::thread::Builder::new()
            .name("plugins-curated-repo-sync".to_string())
            .spawn(
                move || match sync_openai_plugins_repo(codex_home.as_path()) {
                    Ok(curated_plugin_version) => {
                        match refresh_curated_plugin_cache(
                            codex_home.as_path(),
                            &curated_plugin_version,
                            &configured_curated_plugin_ids,
                        ) {
                            Ok(cache_refreshed) => {
                                if cache_refreshed {
                                    manager.clear_cache();
                                }
                            }
                            Err(err) => {
                                manager.clear_cache();
                                CURATED_REPO_SYNC_STARTED.store(false, Ordering::SeqCst);
                                warn!("failed to refresh curated plugin cache after sync: {err}");
                            }
                        }
                    }
                    Err(err) => {
                        CURATED_REPO_SYNC_STARTED.store(false, Ordering::SeqCst);
                        warn!("failed to sync curated plugins repo: {err}");
                    }
                },
            )
        {
            CURATED_REPO_SYNC_STARTED.store(false, Ordering::SeqCst);
            warn!("failed to start curated plugins repo sync task: {err}");
        }
    }

    fn configured_plugin_states(
        &self,
        config: &Config,
    ) -> (HashSet<String>, HashMap<String, bool>) {
        let installed_plugins = configured_plugins_from_stack(&config.config_layer_stack)
            .into_keys()
            .filter(|plugin_key| {
                PluginId::parse(plugin_key)
                    .ok()
                    .is_some_and(|plugin_id| self.store.is_installed(&plugin_id))
            })
            .collect::<HashSet<_>>();
        let configured_plugins = self
            .plugins_for_config(config)
            .plugins()
            .iter()
            .map(|plugin| (plugin.config_name.clone(), plugin.enabled))
            .collect::<HashMap<String, bool>>();
        (installed_plugins, configured_plugins)
    }

    fn marketplace_roots(&self, additional_roots: &[AbsolutePathBuf]) -> Vec<AbsolutePathBuf> {
        // Treat the curated catalog as an extra marketplace root so plugin listing can surface it
        // without requiring every caller to know where it is stored.
        let mut roots = additional_roots.to_vec();
        let curated_repo_root = curated_plugins_repo_path(self.codex_home.as_path());
        if curated_repo_root.is_dir()
            && let Ok(curated_repo_root) = AbsolutePathBuf::try_from(curated_repo_root)
        {
            roots.push(curated_repo_root);
        }
        roots.sort_unstable_by(|left, right| left.as_path().cmp(right.as_path()));
        roots.dedup();
        roots
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PluginInstallError {
    #[error("{0}")]
    Marketplace(#[from] MarketplaceError),

    #[error("{0}")]
    Remote(#[from] RemotePluginMutationError),

    #[error("{0}")]
    Store(#[from] PluginStoreError),

    #[error("{0}")]
    Config(#[from] ConfigServiceError),

    #[error("failed to join plugin install task: {0}")]
    Join(#[from] tokio::task::JoinError),
}

impl PluginInstallError {
    fn join(source: tokio::task::JoinError) -> Self {
        Self::Join(source)
    }

    pub fn is_invalid_request(&self) -> bool {
        matches!(
            self,
            Self::Marketplace(
                MarketplaceError::MarketplaceNotFound { .. }
                    | MarketplaceError::InvalidMarketplaceFile { .. }
                    | MarketplaceError::PluginNotFound { .. }
                    | MarketplaceError::PluginNotAvailable { .. }
                    | MarketplaceError::InvalidPlugin(_)
            ) | Self::Store(PluginStoreError::Invalid(_))
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PluginUninstallError {
    #[error("{0}")]
    InvalidPluginId(#[from] PluginIdError),

    #[error("{0}")]
    Remote(#[from] RemotePluginMutationError),

    #[error("{0}")]
    Store(#[from] PluginStoreError),

    #[error("{0}")]
    Config(#[from] anyhow::Error),

    #[error("failed to join plugin uninstall task: {0}")]
    Join(#[from] tokio::task::JoinError),
}

impl PluginUninstallError {
    fn join(source: tokio::task::JoinError) -> Self {
        Self::Join(source)
    }

    pub fn is_invalid_request(&self) -> bool {
        matches!(self, Self::InvalidPluginId(_))
    }
}

fn plugins_feature_enabled_from_stack(config_layer_stack: &ConfigLayerStack) -> bool {
    // Plugins are intentionally opt-in from the persisted user config only. Project config
    // layers should not be able to enable plugin loading for a checkout.
    let Some(user_layer) = config_layer_stack.get_user_layer() else {
        return false;
    };
    let Ok(config_toml) = user_layer.config.clone().try_into::<ConfigToml>() else {
        warn!("failed to deserialize config when checking plugin feature flag");
        return false;
    };
    let config_profile = config_toml
        .get_config_profile(config_toml.profile.clone())
        .unwrap_or_else(|_| ConfigProfile::default());
    let features =
        Features::from_config(&config_toml, &config_profile, FeatureOverrides::default());
    features.enabled(Feature::Plugins)
}

fn log_plugin_load_errors(outcome: &PluginLoadOutcome) {
    for plugin in outcome
        .plugins
        .iter()
        .filter(|plugin| plugin.error.is_some())
    {
        if let Some(error) = plugin.error.as_deref() {
            warn!(
                plugin = plugin.config_name,
                path = %plugin.root.display(),
                "failed to load plugin: {error}"
            );
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginMcpFile {
    #[serde(default)]
    mcp_servers: HashMap<String, JsonValue>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginAppFile {
    #[serde(default)]
    apps: HashMap<String, PluginAppConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct PluginAppConfig {
    id: String,
}

pub(crate) fn load_plugins_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
    store: &PluginStore,
) -> PluginLoadOutcome {
    let mut configured_plugins: Vec<_> = configured_plugins_from_stack(config_layer_stack)
        .into_iter()
        .collect();
    configured_plugins.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));

    let mut plugins = Vec::with_capacity(configured_plugins.len());
    let mut seen_mcp_server_names = HashMap::<String, String>::new();
    for (configured_name, plugin) in configured_plugins {
        let loaded_plugin = load_plugin(configured_name.clone(), &plugin, store);
        for name in loaded_plugin.mcp_servers.keys() {
            if let Some(previous_plugin) =
                seen_mcp_server_names.insert(name.clone(), configured_name.clone())
            {
                warn!(
                    plugin = configured_name,
                    previous_plugin,
                    server = name,
                    "skipping duplicate plugin MCP server name"
                );
            }
        }
        plugins.push(loaded_plugin);
    }

    PluginLoadOutcome::from_plugins(plugins)
}

pub(crate) fn plugin_namespace_for_skill_path(path: &Path) -> Option<String> {
    for ancestor in path.ancestors() {
        if let Some(manifest) = load_plugin_manifest(ancestor) {
            return Some(plugin_manifest_name(&manifest, ancestor));
        }
    }

    None
}

fn refresh_curated_plugin_cache(
    codex_home: &Path,
    plugin_version: &str,
    configured_curated_plugin_ids: &[PluginId],
) -> Result<bool, String> {
    let store = PluginStore::new(codex_home.to_path_buf());
    let curated_marketplace_path = AbsolutePathBuf::try_from(
        curated_plugins_repo_path(codex_home).join(".agents/plugins/marketplace.json"),
    )
    .map_err(|_| "local curated marketplace is not available".to_string())?;
    let curated_marketplace = load_marketplace_summary(&curated_marketplace_path)
        .map_err(|err| format!("failed to load curated marketplace for cache refresh: {err}"))?;

    let mut plugin_sources = HashMap::<String, AbsolutePathBuf>::new();
    for plugin in curated_marketplace.plugins {
        let plugin_name = plugin.name;
        if plugin_sources.contains_key(&plugin_name) {
            warn!(
                plugin = plugin_name,
                marketplace = OPENAI_CURATED_MARKETPLACE_NAME,
                "ignoring duplicate curated plugin entry during cache refresh"
            );
            continue;
        }
        let source_path = match plugin.source {
            MarketplacePluginSourceSummary::Local { path } => path,
        };
        plugin_sources.insert(plugin_name, source_path);
    }

    let mut cache_refreshed = false;
    for plugin_id in configured_curated_plugin_ids {
        if store.active_plugin_version(plugin_id).as_deref() == Some(plugin_version) {
            continue;
        }

        let Some(source_path) = plugin_sources.get(&plugin_id.plugin_name).cloned() else {
            warn!(
                plugin = plugin_id.plugin_name,
                marketplace = OPENAI_CURATED_MARKETPLACE_NAME,
                "configured curated plugin no longer exists in curated marketplace during cache refresh"
            );
            continue;
        };

        store
            .install_with_version(source_path, plugin_id.clone(), plugin_version.to_string())
            .map_err(|err| {
                format!(
                    "failed to refresh curated plugin cache for {}: {err}",
                    plugin_id.as_key()
                )
            })?;
        cache_refreshed = true;
    }

    Ok(cache_refreshed)
}

fn configured_plugins_from_stack(
    config_layer_stack: &ConfigLayerStack,
) -> HashMap<String, PluginConfig> {
    // Keep plugin entries aligned with the same user-layer-only semantics as the feature gate.
    let Some(user_layer) = config_layer_stack.get_user_layer() else {
        return HashMap::new();
    };
    let Some(plugins_value) = user_layer.config.get("plugins") else {
        return HashMap::new();
    };
    match plugins_value.clone().try_into() {
        Ok(plugins) => plugins,
        Err(err) => {
            warn!("invalid plugins config: {err}");
            HashMap::new()
        }
    }
}

fn load_plugin(config_name: String, plugin: &PluginConfig, store: &PluginStore) -> LoadedPlugin {
    let plugin_root = PluginId::parse(&config_name).map(|plugin_id| {
        store
            .active_plugin_root(&plugin_id)
            .unwrap_or_else(|| store.plugin_root(&plugin_id, DEFAULT_PLUGIN_VERSION))
    });
    let root = match &plugin_root {
        Ok(plugin_root) => plugin_root.clone(),
        Err(_) => store.root().clone(),
    };
    let mut loaded_plugin = LoadedPlugin {
        config_name,
        manifest_name: None,
        manifest_description: None,
        root,
        enabled: plugin.enabled,
        skill_roots: Vec::new(),
        mcp_servers: HashMap::new(),
        apps: Vec::new(),
        error: None,
    };

    if !plugin.enabled {
        return loaded_plugin;
    }

    let plugin_root = match plugin_root {
        Ok(plugin_root) => plugin_root,
        Err(err) => {
            loaded_plugin.error = Some(err.to_string());
            return loaded_plugin;
        }
    };

    if !plugin_root.as_path().is_dir() {
        loaded_plugin.error = Some("path does not exist or is not a directory".to_string());
        return loaded_plugin;
    }

    let Some(manifest) = load_plugin_manifest(plugin_root.as_path()) else {
        loaded_plugin.error = Some("missing or invalid .codex-plugin/plugin.json".to_string());
        return loaded_plugin;
    };

    let manifest_paths = plugin_manifest_paths(&manifest, plugin_root.as_path());
    loaded_plugin.manifest_name = Some(plugin_manifest_name(&manifest, plugin_root.as_path()));
    loaded_plugin.manifest_description = manifest.description;
    loaded_plugin.skill_roots = plugin_skill_roots(plugin_root.as_path(), &manifest_paths);
    let mut mcp_servers = HashMap::new();
    for mcp_config_path in plugin_mcp_config_paths(plugin_root.as_path(), &manifest_paths) {
        let plugin_mcp = load_mcp_servers_from_file(plugin_root.as_path(), &mcp_config_path);
        for (name, config) in plugin_mcp.mcp_servers {
            if mcp_servers.insert(name.clone(), config).is_some() {
                warn!(
                    plugin = %plugin_root.display(),
                    path = %mcp_config_path.display(),
                    server = name,
                    "plugin MCP file overwrote an earlier server definition"
                );
            }
        }
    }
    loaded_plugin.mcp_servers = mcp_servers;
    loaded_plugin.apps = load_plugin_apps(plugin_root.as_path());
    loaded_plugin
}

fn plugin_skill_roots(plugin_root: &Path, manifest_paths: &PluginManifestPaths) -> Vec<PathBuf> {
    let mut paths = default_skill_roots(plugin_root);
    if let Some(path) = &manifest_paths.skills {
        paths.push(path.to_path_buf());
    }
    paths.sort_unstable();
    paths.dedup();
    paths
}

fn default_skill_roots(plugin_root: &Path) -> Vec<PathBuf> {
    let skills_dir = plugin_root.join(DEFAULT_SKILLS_DIR_NAME);
    if skills_dir.is_dir() {
        vec![skills_dir]
    } else {
        Vec::new()
    }
}

fn plugin_mcp_config_paths(
    plugin_root: &Path,
    manifest_paths: &PluginManifestPaths,
) -> Vec<AbsolutePathBuf> {
    if let Some(path) = &manifest_paths.mcp_servers {
        return vec![path.clone()];
    }
    default_mcp_config_paths(plugin_root)
}

fn default_mcp_config_paths(plugin_root: &Path) -> Vec<AbsolutePathBuf> {
    let mut paths = Vec::new();
    let default_path = plugin_root.join(DEFAULT_MCP_CONFIG_FILE);
    if default_path.is_file()
        && let Ok(default_path) = AbsolutePathBuf::try_from(default_path)
    {
        paths.push(default_path);
    }
    paths.sort_unstable_by(|left, right| left.as_path().cmp(right.as_path()));
    paths.dedup_by(|left, right| left.as_path() == right.as_path());
    paths
}

pub fn load_plugin_apps(plugin_root: &Path) -> Vec<AppConnectorId> {
    if let Some(manifest) = load_plugin_manifest(plugin_root) {
        let manifest_paths = plugin_manifest_paths(&manifest, plugin_root);
        return load_apps_from_paths(
            plugin_root,
            plugin_app_config_paths(plugin_root, &manifest_paths),
        );
    }
    load_apps_from_paths(plugin_root, default_app_config_paths(plugin_root))
}

fn plugin_app_config_paths(
    plugin_root: &Path,
    manifest_paths: &PluginManifestPaths,
) -> Vec<AbsolutePathBuf> {
    if let Some(path) = &manifest_paths.apps {
        return vec![path.clone()];
    }
    default_app_config_paths(plugin_root)
}

fn default_app_config_paths(plugin_root: &Path) -> Vec<AbsolutePathBuf> {
    let mut paths = Vec::new();
    let default_path = plugin_root.join(DEFAULT_APP_CONFIG_FILE);
    if default_path.is_file()
        && let Ok(default_path) = AbsolutePathBuf::try_from(default_path)
    {
        paths.push(default_path);
    }
    paths.sort_unstable_by(|left, right| left.as_path().cmp(right.as_path()));
    paths.dedup_by(|left, right| left.as_path() == right.as_path());
    paths
}

fn load_apps_from_paths(
    plugin_root: &Path,
    app_config_paths: Vec<AbsolutePathBuf>,
) -> Vec<AppConnectorId> {
    let mut connector_ids = Vec::new();
    for app_config_path in app_config_paths {
        let Ok(contents) = fs::read_to_string(app_config_path.as_path()) else {
            continue;
        };
        let parsed = match serde_json::from_str::<PluginAppFile>(&contents) {
            Ok(parsed) => parsed,
            Err(err) => {
                warn!(
                    path = %app_config_path.display(),
                    "failed to parse plugin app config: {err}"
                );
                continue;
            }
        };

        let mut apps: Vec<PluginAppConfig> = parsed.apps.into_values().collect();
        apps.sort_unstable_by(|left, right| left.id.cmp(&right.id));

        connector_ids.extend(apps.into_iter().filter_map(|app| {
            if app.id.trim().is_empty() {
                warn!(
                    plugin = %plugin_root.display(),
                    "plugin app config is missing an app id"
                );
                None
            } else {
                Some(AppConnectorId(app.id))
            }
        }));
    }
    connector_ids.dedup();
    connector_ids
}

pub fn plugin_telemetry_metadata_from_root(
    plugin_id: &PluginId,
    plugin_root: &Path,
) -> PluginTelemetryMetadata {
    let Some(manifest) = load_plugin_manifest(plugin_root) else {
        return PluginTelemetryMetadata::from_plugin_id(plugin_id);
    };

    let manifest_paths = plugin_manifest_paths(&manifest, plugin_root);
    let has_skills = !plugin_skill_roots(plugin_root, &manifest_paths).is_empty();
    let mut mcp_server_names = Vec::new();
    for path in plugin_mcp_config_paths(plugin_root, &manifest_paths) {
        mcp_server_names.extend(
            load_mcp_servers_from_file(plugin_root, &path)
                .mcp_servers
                .into_keys(),
        );
    }
    mcp_server_names.sort_unstable();
    mcp_server_names.dedup();

    PluginTelemetryMetadata {
        plugin_id: plugin_id.clone(),
        capability_summary: Some(PluginCapabilitySummary {
            config_name: plugin_id.as_key(),
            display_name: plugin_id.plugin_name.clone(),
            description: None,
            has_skills,
            mcp_server_names,
            app_connector_ids: load_plugin_apps(plugin_root),
        }),
    }
}

pub fn installed_plugin_telemetry_metadata(
    codex_home: &Path,
    plugin_id: &PluginId,
) -> PluginTelemetryMetadata {
    let store = PluginStore::new(codex_home.to_path_buf());
    let Some(plugin_root) = store.active_plugin_root(plugin_id) else {
        return PluginTelemetryMetadata::from_plugin_id(plugin_id);
    };

    plugin_telemetry_metadata_from_root(plugin_id, plugin_root.as_path())
}

fn load_mcp_servers_from_file(
    plugin_root: &Path,
    mcp_config_path: &AbsolutePathBuf,
) -> PluginMcpDiscovery {
    let Ok(contents) = fs::read_to_string(mcp_config_path.as_path()) else {
        return PluginMcpDiscovery::default();
    };
    let parsed = match serde_json::from_str::<PluginMcpFile>(&contents) {
        Ok(parsed) => parsed,
        Err(err) => {
            warn!(
                path = %mcp_config_path.display(),
                "failed to parse plugin MCP config: {err}"
            );
            return PluginMcpDiscovery::default();
        }
    };
    normalize_plugin_mcp_servers(
        plugin_root,
        parsed.mcp_servers,
        mcp_config_path.to_string_lossy().as_ref(),
    )
}

fn normalize_plugin_mcp_servers(
    plugin_root: &Path,
    plugin_mcp_servers: HashMap<String, JsonValue>,
    source: &str,
) -> PluginMcpDiscovery {
    let mut mcp_servers = HashMap::new();

    for (name, config_value) in plugin_mcp_servers {
        let normalized = normalize_plugin_mcp_server_value(plugin_root, config_value);
        match serde_json::from_value::<McpServerConfig>(JsonValue::Object(normalized)) {
            Ok(config) => {
                mcp_servers.insert(name, config);
            }
            Err(err) => {
                warn!(
                    plugin = %plugin_root.display(),
                    server = name,
                    "failed to parse plugin MCP server from {source}: {err}"
                );
            }
        }
    }

    PluginMcpDiscovery { mcp_servers }
}

fn normalize_plugin_mcp_server_value(
    plugin_root: &Path,
    value: JsonValue,
) -> JsonMap<String, JsonValue> {
    let mut object = match value {
        JsonValue::Object(object) => object,
        _ => return JsonMap::new(),
    };

    if let Some(JsonValue::String(transport_type)) = object.remove("type") {
        match transport_type.as_str() {
            "http" | "streamable_http" | "streamable-http" => {}
            "stdio" => {}
            other => {
                warn!(
                    plugin = %plugin_root.display(),
                    transport = other,
                    "plugin MCP server uses an unknown transport type"
                );
            }
        }
    }

    if let Some(JsonValue::Object(oauth)) = object.remove("oauth")
        && oauth.contains_key("callbackPort")
    {
        warn!(
            plugin = %plugin_root.display(),
            "plugin MCP server OAuth callbackPort is ignored; Codex uses global MCP OAuth callback settings"
        );
    }

    if let Some(JsonValue::String(cwd)) = object.get("cwd")
        && !Path::new(cwd).is_absolute()
    {
        object.insert(
            "cwd".to_string(),
            JsonValue::String(plugin_root.join(cwd).display().to_string()),
        );
    }

    object
}

#[derive(Debug, Default)]
struct PluginMcpDiscovery {
    mcp_servers: HashMap<String, McpServerConfig>,
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
