use crate::PluginGitMode;
use crate::app_mcp_routing::apply_app_mcp_routing_policy;
use crate::app_mcp_routing::apps_route_available;
use crate::is_openai_curated_marketplace_name;
use crate::manifest::PluginManifest;
use crate::manifest::PluginManifestFormat;
use crate::manifest::PluginManifestHooks;
use crate::manifest::PluginManifestMcpServers;
use crate::manifest::PluginManifestPaths;
use crate::manifest::load_plugin_manifest_with_format;
use crate::marketplace::MarketplacePluginSource;
use crate::marketplace::find_marketplace_plugin;
use crate::marketplace::list_marketplaces_with_home;
use crate::marketplace::load_marketplace;
use crate::marketplace_policy::configured_plugins_from_stack;
use crate::npm_source::materialize_npm_plugin_source;
use crate::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use crate::remote::RemoteInstalledPlugin;
use crate::remote_plugin_id_resolver::RemoteInstalledPluginsSnapshot;
use crate::remote_plugin_id_resolver::RemotePluginIdResolver;
use crate::store::PluginStore;
use crate::store::plugin_version_for_source;
use crate::store::plugin_version_for_source_with_fallback_manifest;
use codex_config::ConfigLayerStack;
use codex_config::HooksFile;
use codex_config::SkillConfigRules;
use codex_config::skill_config_rules_from_stack;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_config::types::PluginConfig;
use codex_config::types::PluginMcpServerConfig;
use codex_connectors::parse_plugin_app_config;
use codex_connectors::parse_plugin_app_config_value;
use codex_mcp::parse_agent_plugin_mcp_config;
use codex_mcp::parse_plugin_mcp_config;
use codex_plugin::AppDeclaration;
use codex_plugin::LoadedPlugin;
use codex_plugin::PluginCapabilitySummary;
use codex_plugin::PluginHookSource;
use codex_plugin::PluginId;
use codex_plugin::PluginIdError;
use codex_plugin::app_connector_ids_from_declarations;
use codex_protocol::auth::AuthMode;
use codex_protocol::protocol::Product;
use codex_skills::SkillMetadata;
use codex_skills::SkillRootLoadRequest;
use codex_skills::SkillRootLoader;
use codex_skills::SkillRootSnapshots;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::PluginIdentity;
use codex_utils_plugins::PluginSkillRoot;
use codex_utils_plugins::SkillDiscoveryMode;
use codex_utils_plugins::find_plugin_manifest_path;
use codex_utils_plugins::migrated_command_skills_root;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use tracing::instrument;
use tracing::warn;

#[path = "agent_plugin_mcp_overlay.rs"]
mod agent_plugin_mcp_overlay;

const DEFAULT_SKILLS_DIR_NAME: &str = "skills";
const DEFAULT_HOOKS_CONFIG_FILE: &str = "hooks/hooks.json";
const DEFAULT_MCP_CONFIG_FILE: &str = ".mcp.json";
const DEFAULT_APP_CONFIG_FILE: &str = ".app.json";
const CONFIG_TOML_FILE: &str = "config.toml";
const CURATED_PLUGIN_CACHE_VERSION_SHA_PREFIX_LEN: usize = 8;

/// Hook declarations and warnings resolved without loading other plugin capabilities.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginHookLoadOutcome {
    pub hook_sources: Vec<PluginHookSource>,
    pub hook_load_warnings: Vec<String>,
}

/// The built-in curated marketplace selection for the current runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetCuratedMarketplace {
    OpenAi,
    OpenAiWithRemote,
    OpenAiApi,
}

enum PluginLoadScope<'a> {
    AllCapabilities {
        restriction_product: Option<Product>,
        skill_config_rules: &'a SkillConfigRules,
        plugin_skill_snapshots: Option<&'a SkillRootSnapshots<PluginSkillRoot>>,
        remote_plugin_id_resolver: &'a RemotePluginIdResolver,
        skill_root_loader: &'a dyn SkillRootLoader<PluginSkillRoot>,
    },
    HooksOnly,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NonCuratedCacheRefreshMode {
    IfVersionChanged,
    ForceReinstall,
}

#[derive(Debug)]
pub(crate) struct NonCuratedCacheRefreshOutcome {
    pub(crate) cache_refreshed: bool,
    pub(crate) errors: Vec<NonCuratedCacheRefreshError>,
}

#[derive(Debug)]
pub(crate) struct NonCuratedCacheRefreshError {
    pub(crate) marketplace_name: String,
    pub(crate) message: String,
}

pub(crate) fn log_plugin_load_errors(plugins: &[LoadedPlugin<McpServerConfig>]) {
    for plugin in plugins.iter().filter(|plugin| plugin.error.is_some()) {
        if let Some(error) = plugin.error.as_deref() {
            warn!(
                plugin = plugin.config_name,
                path = %plugin.root.display(),
                "failed to load plugin: {error}"
            );
        }
    }
}

/// Load configured plugins without applying auth-dependent runtime policies.
#[instrument(level = "trace", skip_all)]
pub(crate) async fn load_plugins_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
    remote_installed_plugins_snapshot: RemoteInstalledPluginsSnapshot,
    store: &PluginStore,
    plugin_skill_snapshots: Option<&SkillRootSnapshots<PluginSkillRoot>>,
    restriction_product: Option<Product>,
    remote_global_catalog_active: bool,
    skill_root_loader: &dyn SkillRootLoader<PluginSkillRoot>,
) -> Vec<LoadedPlugin<McpServerConfig>> {
    let skill_config_rules = skill_config_rules_from_stack(config_layer_stack);
    let RemoteInstalledPluginsSnapshot {
        configs: extra_plugins,
        remote_plugin_id_resolver,
    } = remote_installed_plugins_snapshot;
    load_plugins_from_layer_stack_with_scope(
        config_layer_stack,
        extra_plugins,
        store,
        remote_global_catalog_active,
        PluginLoadScope::AllCapabilities {
            restriction_product,
            skill_config_rules: &skill_config_rules,
            plugin_skill_snapshots,
            remote_plugin_id_resolver: &remote_plugin_id_resolver,
            skill_root_loader,
        },
    )
    .await
}

async fn load_plugins_from_layer_stack_with_scope(
    config_layer_stack: &ConfigLayerStack,
    extra_plugins: HashMap<String, PluginConfig>,
    store: &PluginStore,
    remote_global_catalog_active: bool,
    scope: PluginLoadScope<'_>,
) -> Vec<LoadedPlugin<McpServerConfig>> {
    let configured_plugins = merge_configured_plugins_with_remote_installed(
        configured_plugins_from_stack(config_layer_stack, store.codex_home().as_path()),
        extra_plugins,
        store,
        remote_global_catalog_active,
    );
    let mut configured_plugins: Vec<_> = configured_plugins.into_iter().collect();
    configured_plugins.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));

    let mut plugins = Vec::with_capacity(configured_plugins.len());
    let mut seen_mcp_server_names = HashMap::<String, String>::new();
    for (configured_name, plugin) in configured_plugins {
        let loaded_plugin = load_plugin(configured_name.clone(), &plugin, store, &scope).await;
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

    plugins
}

/// Load hooks from enabled plugins without loading their skills, MCP servers, or apps.
pub async fn load_plugin_hooks_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
    extra_plugins: HashMap<String, PluginConfig>,
    store: &PluginStore,
    target_curated_marketplace: TargetCuratedMarketplace,
    remote_global_catalog_active: bool,
) -> PluginHookLoadOutcome {
    let mut plugins = load_plugins_from_layer_stack_with_scope(
        config_layer_stack,
        extra_plugins,
        store,
        remote_global_catalog_active,
        PluginLoadScope::HooksOnly,
    )
    .await;
    plugins.retain(|plugin| {
        plugin_is_eligible_for_target_marketplace(&plugin.config_name, target_curated_marketplace)
    });
    PluginHookLoadOutcome {
        hook_sources: plugins
            .iter()
            .filter(|plugin| plugin.is_active())
            .flat_map(|plugin| plugin.hook_sources.iter().cloned())
            .collect(),
        hook_load_warnings: plugins
            .iter()
            .filter(|plugin| plugin.is_active())
            .flat_map(|plugin| plugin.hook_load_warnings.iter().cloned())
            .collect(),
    }
}

fn merge_configured_plugins_with_remote_installed(
    mut configured_plugins: HashMap<String, PluginConfig>,
    extra_plugins: HashMap<String, PluginConfig>,
    store: &PluginStore,
    remote_global_catalog_active: bool,
) -> HashMap<String, PluginConfig> {
    if remote_global_catalog_active {
        configured_plugins.retain(|plugin_key, _| match PluginId::parse(plugin_key) {
            Ok(plugin_id) => plugin_id.marketplace_name != crate::OPENAI_CURATED_MARKETPLACE_NAME,
            Err(_) => true,
        });
        for (plugin_key, plugin_config) in extra_plugins {
            merge_remote_plugin_config(&mut configured_plugins, plugin_key, plugin_config);
        }
        return configured_plugins;
    }

    let mut local_curated_installed_plugin_keys = HashMap::<String, Vec<String>>::new();
    for plugin_key in configured_plugins.keys() {
        let Ok(plugin_id) = PluginId::parse(plugin_key) else {
            continue;
        };
        if plugin_id.marketplace_name != crate::OPENAI_CURATED_MARKETPLACE_NAME
            || store.active_plugin_version(&plugin_id).is_none()
        {
            continue;
        }
        local_curated_installed_plugin_keys
            .entry(plugin_id.plugin_name)
            .or_default()
            .push(plugin_key.clone());
    }

    for (plugin_key, plugin_config) in extra_plugins {
        let remote_curated_plugin_name = installed_plugin_name_for_marketplace(
            &plugin_key,
            REMOTE_GLOBAL_MARKETPLACE_NAME,
            store,
        );
        let local_curated_plugin_keys = remote_curated_plugin_name
            .as_ref()
            .and_then(|plugin_name| local_curated_installed_plugin_keys.get(plugin_name));

        if local_curated_plugin_keys.is_some() {
            continue;
        }

        merge_remote_plugin_config(&mut configured_plugins, plugin_key, plugin_config);
    }

    configured_plugins
}

pub(crate) fn plugin_is_eligible_for_target_marketplace(
    plugin_key: &str,
    target_curated_marketplace: TargetCuratedMarketplace,
) -> bool {
    let Ok(plugin_id) = PluginId::parse(plugin_key) else {
        return true;
    };
    match target_curated_marketplace {
        TargetCuratedMarketplace::OpenAi => {
            plugin_id.marketplace_name != crate::OPENAI_API_CURATED_MARKETPLACE_NAME
                && plugin_id.marketplace_name != REMOTE_GLOBAL_MARKETPLACE_NAME
        }
        TargetCuratedMarketplace::OpenAiWithRemote => {
            plugin_id.marketplace_name != crate::OPENAI_API_CURATED_MARKETPLACE_NAME
        }
        TargetCuratedMarketplace::OpenAiApi => {
            plugin_id.marketplace_name != crate::OPENAI_CURATED_MARKETPLACE_NAME
                && plugin_id.marketplace_name != REMOTE_GLOBAL_MARKETPLACE_NAME
        }
    }
}

fn merge_remote_plugin_config(
    configured_plugins: &mut HashMap<String, PluginConfig>,
    plugin_key: String,
    mut remote_plugin_config: PluginConfig,
) {
    if let Some(configured_plugin) = configured_plugins.get(&plugin_key) {
        remote_plugin_config
            .mcp_servers
            .clone_from(&configured_plugin.mcp_servers);
    }
    configured_plugins.insert(plugin_key, remote_plugin_config);
}

fn installed_plugin_name_for_marketplace(
    plugin_key: &str,
    marketplace_name: &str,
    store: &PluginStore,
) -> Option<String> {
    let plugin_id = PluginId::parse(plugin_key).ok()?;
    if plugin_id.marketplace_name != marketplace_name {
        return None;
    }
    store.active_plugin_root(&plugin_id)?;
    Some(plugin_id.plugin_name)
}

pub fn remote_installed_plugins_to_config(
    plugins: &[RemoteInstalledPlugin],
    store: &PluginStore,
) -> HashMap<String, PluginConfig> {
    plugins
        .iter()
        .filter_map(|plugin| {
            let plugin_id =
                match PluginId::new(plugin.name.clone(), plugin.marketplace_name.clone()) {
                    Ok(plugin_id) => plugin_id,
                    Err(err) => {
                        warn!(
                            plugin = %plugin.name,
                            remote_id = %plugin.id,
                            error = %err,
                            "ignoring invalid remote installed plugin name"
                        );
                        return None;
                    }
                };
            // TODO(remote plugins): download or update missing local bundles during remote
            // installed reconciliation. Until then, only publish remote installed state for
            // bundles already present in the local plugin cache.
            store.active_plugin_root(&plugin_id)?;
            Some((
                plugin_id.as_key(),
                PluginConfig {
                    enabled: plugin.enabled,
                    mcp_servers: HashMap::new(),
                },
            ))
        })
        .collect()
}

pub fn refresh_curated_plugin_cache(
    codex_home: &Path,
    plugin_version: &str,
    configured_curated_plugin_ids: &[PluginId],
) -> Result<bool, String> {
    let cache_plugin_version = curated_plugin_cache_version(plugin_version);
    let store = PluginStore::try_new(codex_home.to_path_buf()).map_err(|err| err.to_string())?;
    let curated_marketplace_paths = curated_marketplace_paths_for_cache_refresh(codex_home)?;
    let mut loaded_marketplace_names = HashSet::<String>::new();
    let mut marketplace_plugin_keys = HashSet::<String>::new();
    let mut plugin_sources = HashMap::<String, AbsolutePathBuf>::new();

    for curated_marketplace_path in curated_marketplace_paths {
        let curated_marketplace = load_marketplace(&curated_marketplace_path).map_err(|err| {
            format!("failed to load curated marketplace for cache refresh: {err}")
        })?;
        let marketplace_name = curated_marketplace.name;
        loaded_marketplace_names.insert(marketplace_name.clone());

        for plugin in curated_marketplace.plugins {
            let plugin_id =
                PluginId::new(plugin.name.clone(), marketplace_name.clone()).map_err(|err| {
                    match err {
                        PluginIdError::Invalid(message) => {
                            format!("failed to prepare curated plugin cache refresh: {message}")
                        }
                    }
                })?;
            let plugin_key = plugin_id.as_key();
            marketplace_plugin_keys.insert(plugin_key.clone());
            if plugin_sources.contains_key(&plugin_key) {
                warn!(
                    plugin = %plugin.name,
                    marketplace = %marketplace_name,
                    "ignoring duplicate curated plugin entry during cache refresh"
                );
                continue;
            }
            if let MarketplacePluginSource::Local { path } = plugin.source {
                plugin_sources.insert(plugin_key, path);
            }
        }
    }

    let mut cache_refreshed = false;
    for plugin_id in configured_curated_plugin_ids {
        let plugin_key = plugin_id.as_key();
        if !marketplace_plugin_keys.contains(&plugin_key) {
            if !loaded_marketplace_names.contains(&plugin_id.marketplace_name) {
                continue;
            }
            warn!(
                plugin = %plugin_id.plugin_name,
                marketplace = %plugin_id.marketplace_name,
                "configured curated plugin no longer exists in curated marketplace during cache refresh"
            );
            if store.plugin_base_root(plugin_id).as_path().exists() {
                store.uninstall(plugin_id).map_err(|err| {
                    format!(
                        "failed to remove stale curated plugin cache for {}: {err}",
                        plugin_id.as_key()
                    )
                })?;
                cache_refreshed = true;
            }
            continue;
        }

        let Some(source_path) = plugin_sources.get(&plugin_key).cloned() else {
            continue;
        };

        if store.active_plugin_version(plugin_id).as_deref() == Some(cache_plugin_version.as_str())
        {
            continue;
        }

        store
            .install_with_version(source_path, plugin_id.clone(), cache_plugin_version.clone())
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

fn curated_marketplace_paths_for_cache_refresh(
    codex_home: &Path,
) -> Result<Vec<AbsolutePathBuf>, String> {
    let curated_marketplace_path = AbsolutePathBuf::try_from(
        codex_home
            .join(".tmp/plugins")
            .join(".agents/plugins/marketplace.json"),
    )
    .map_err(|_| "local curated marketplace is not available".to_string())?;
    let mut paths = vec![curated_marketplace_path];

    let api_marketplace_path = codex_home
        .join(".tmp/plugins")
        .join(".agents/plugins/api_marketplace.json");
    if api_marketplace_path.is_file() {
        paths.push(
            AbsolutePathBuf::try_from(api_marketplace_path)
                .map_err(|_| "local API curated marketplace is not available".to_string())?,
        );
    }

    Ok(paths)
}

pub fn curated_plugin_cache_version(plugin_version: &str) -> String {
    if is_full_git_sha(plugin_version) {
        plugin_version[..CURATED_PLUGIN_CACHE_VERSION_SHA_PREFIX_LEN].to_string()
    } else {
        plugin_version.to_string()
    }
}

#[cfg(test)]
pub(crate) fn refresh_non_curated_plugin_cache(
    codex_home: &Path,
    additional_roots: &[AbsolutePathBuf],
    configured_plugin_keys: &[String],
) -> Result<bool, String> {
    collapse_non_curated_cache_refresh(refresh_non_curated_plugin_cache_detailed(
        codex_home,
        additional_roots,
        configured_plugin_keys,
        PluginGitMode::Automatic,
    ))
}

pub(crate) fn refresh_non_curated_plugin_cache_detailed(
    codex_home: &Path,
    additional_roots: &[AbsolutePathBuf],
    configured_plugin_keys: &[String],
    git_mode: PluginGitMode,
) -> Result<NonCuratedCacheRefreshOutcome, String> {
    refresh_non_curated_plugin_cache_with_mode(
        codex_home,
        additional_roots,
        configured_plugin_keys,
        NonCuratedCacheRefreshMode::IfVersionChanged,
        git_mode,
    )
}

#[cfg(test)]
pub(crate) fn refresh_non_curated_plugin_cache_force_reinstall(
    codex_home: &Path,
    additional_roots: &[AbsolutePathBuf],
    configured_plugin_keys: &[String],
) -> Result<bool, String> {
    collapse_non_curated_cache_refresh(refresh_non_curated_plugin_cache_force_reinstall_detailed(
        codex_home,
        additional_roots,
        configured_plugin_keys,
        PluginGitMode::Automatic,
    ))
}

pub(crate) fn refresh_non_curated_plugin_cache_force_reinstall_detailed(
    codex_home: &Path,
    additional_roots: &[AbsolutePathBuf],
    configured_plugin_keys: &[String],
    git_mode: PluginGitMode,
) -> Result<NonCuratedCacheRefreshOutcome, String> {
    refresh_non_curated_plugin_cache_with_mode(
        codex_home,
        additional_roots,
        configured_plugin_keys,
        NonCuratedCacheRefreshMode::ForceReinstall,
        git_mode,
    )
}

fn refresh_non_curated_plugin_cache_with_mode(
    codex_home: &Path,
    additional_roots: &[AbsolutePathBuf],
    configured_plugin_keys: &[String],
    mode: NonCuratedCacheRefreshMode,
    git_mode: PluginGitMode,
) -> Result<NonCuratedCacheRefreshOutcome, String> {
    let mut configured_non_curated_plugin_ids = configured_plugin_keys
        .iter()
        .filter_map(|plugin_key| match PluginId::parse(plugin_key) {
            Ok(plugin_id) if !is_openai_curated_marketplace_name(&plugin_id.marketplace_name) => {
                Some(plugin_id)
            }
            Ok(_) => None,
            Err(err) => {
                warn!(
                    plugin_key,
                    error = %err,
                    "ignoring invalid plugin key during non-curated cache refresh setup"
                );
                None
            }
        })
        .collect::<Vec<_>>();
    configured_non_curated_plugin_ids.sort_unstable_by_key(PluginId::as_key);
    if configured_non_curated_plugin_ids.is_empty() {
        return Ok(NonCuratedCacheRefreshOutcome {
            cache_refreshed: false,
            errors: Vec::new(),
        });
    }
    let configured_non_curated_plugin_keys = configured_non_curated_plugin_ids
        .iter()
        .map(PluginId::as_key)
        .collect::<HashSet<_>>();

    let store = PluginStore::try_new(codex_home.to_path_buf()).map_err(|err| err.to_string())?;
    let marketplace_outcome = list_marketplaces_with_home(additional_roots, /*home_dir*/ None)
        .map_err(|err| format!("failed to discover marketplaces for cache refresh: {err}"))?;
    let mut plugin_sources = HashMap::<String, (MarketplacePluginSource, Option<String>)>::new();

    for marketplace in marketplace_outcome.marketplaces {
        if is_openai_curated_marketplace_name(&marketplace.name) {
            continue;
        }

        for plugin in marketplace.plugins {
            let plugin_id = match PluginId::new(plugin.name.clone(), marketplace.name.clone()) {
                Ok(plugin_id) => plugin_id,
                Err(PluginIdError::Invalid(message)) => {
                    warn!(
                        plugin = plugin.name,
                        marketplace = marketplace.name,
                        error = %message,
                        "ignoring invalid plugin entry during cache refresh"
                    );
                    continue;
                }
            };
            let plugin_key = plugin_id.as_key();
            if !configured_non_curated_plugin_keys.contains(&plugin_key) {
                continue;
            }
            if plugin_sources.contains_key(&plugin_key) {
                warn!(
                    plugin = plugin.name,
                    marketplace = marketplace.name,
                    "ignoring duplicate non-curated plugin entry during cache refresh"
                );
                continue;
            }

            let manifest_fallback = find_marketplace_plugin(&marketplace.path, &plugin.name)
                .map(|resolved| {
                    resolved
                        .manifest_fallback
                        .contents_if_has_metadata()
                        .map(str::to_string)
                })
                .unwrap_or_else(|err| {
                    warn!(
                        plugin = plugin.name,
                        marketplace = marketplace.name,
                        error = %err,
                        "failed to resolve marketplace plugin manifest fallback during cache refresh"
                    );
                    None
                });
            plugin_sources.insert(plugin_key, (plugin.source, manifest_fallback));
        }
    }

    let mut cache_refreshed = false;
    let mut refresh_errors = Vec::new();
    for plugin_id in configured_non_curated_plugin_ids {
        let plugin_key = plugin_id.as_key();
        let Some((source, manifest_fallback_contents)) = plugin_sources.get(&plugin_key).cloned()
        else {
            warn!(
                plugin = plugin_id.plugin_name,
                marketplace = plugin_id.marketplace_name,
                "configured non-curated plugin no longer exists in discovered marketplaces during cache refresh"
            );
            continue;
        };
        let refresh_result = (|| -> Result<bool, String> {
            let materialized =
                materialize_marketplace_plugin_source_with_mode(codex_home, &source, git_mode)
                    .map_err(|err| {
                        format!("failed to materialize plugin source for {plugin_key}: {err}")
                    })?;
            let source_path = materialized.path;
            let plugin_version = match manifest_fallback_contents.as_deref() {
                Some(manifest_contents) => plugin_version_for_source_with_fallback_manifest(
                    source_path.as_path(),
                    manifest_contents,
                ),
                None => plugin_version_for_source(source_path.as_path()),
            }
            .map_err(|err| format!("failed to read plugin version for {plugin_key}: {err}"))?;

            if mode == NonCuratedCacheRefreshMode::IfVersionChanged
                && store.active_plugin_version(&plugin_id).as_deref()
                    == Some(plugin_version.as_str())
            {
                return Ok(false);
            }

            match manifest_fallback_contents.as_deref() {
                Some(manifest_contents) => store.install_with_version_and_fallback_manifest(
                    source_path,
                    plugin_id.clone(),
                    plugin_version,
                    manifest_contents,
                ),
                None => store.install_with_version(source_path, plugin_id.clone(), plugin_version),
            }
            .map_err(|err| format!("failed to refresh plugin cache for {plugin_key}: {err}"))?;
            Ok(true)
        })();
        match refresh_result {
            Ok(refreshed) => cache_refreshed |= refreshed,
            Err(message) => refresh_errors.push(NonCuratedCacheRefreshError {
                marketplace_name: plugin_id.marketplace_name,
                message,
            }),
        }
    }

    Ok(NonCuratedCacheRefreshOutcome {
        cache_refreshed,
        errors: refresh_errors,
    })
}

#[cfg(test)]
fn collapse_non_curated_cache_refresh(
    outcome: Result<NonCuratedCacheRefreshOutcome, String>,
) -> Result<bool, String> {
    let outcome = outcome?;
    if outcome.errors.is_empty() {
        Ok(outcome.cache_refreshed)
    } else {
        Err(outcome
            .errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; "))
    }
}

fn is_full_git_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn configured_plugins_from_user_config_value(
    user_config: &toml::Value,
) -> HashMap<String, PluginConfig> {
    let Some(plugins_value) = user_config.get("plugins") else {
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

fn configured_plugins_from_codex_home(
    codex_home: &Path,
    read_error_message: &str,
    parse_error_message: &str,
) -> HashMap<String, PluginConfig> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let user_config = match fs::read_to_string(&config_path) {
        Ok(user_config) => user_config,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return HashMap::new(),
        Err(err) => {
            warn!(
                path = %config_path.display(),
                error = %err,
                "{read_error_message}"
            );
            return HashMap::new();
        }
    };

    let user_config = match toml::from_str::<toml::Value>(&user_config) {
        Ok(user_config) => user_config,
        Err(err) => {
            warn!(
                path = %config_path.display(),
                error = %err,
                "{parse_error_message}"
            );
            return HashMap::new();
        }
    };

    configured_plugins_from_user_config_value(&user_config)
}

fn configured_plugin_ids(
    configured_plugins: HashMap<String, PluginConfig>,
    invalid_plugin_key_message: &str,
) -> Vec<PluginId> {
    configured_plugins
        .into_keys()
        .filter_map(|plugin_key| match PluginId::parse(&plugin_key) {
            Ok(plugin_id) => Some(plugin_id),
            Err(err) => {
                warn!(
                    plugin_key,
                    error = %err,
                    "{invalid_plugin_key_message}"
                );
                None
            }
        })
        .collect()
}

fn curated_plugin_ids_from_config_keys(
    configured_plugins: HashMap<String, PluginConfig>,
) -> Vec<PluginId> {
    let mut configured_curated_plugin_ids = configured_plugin_ids(
        configured_plugins,
        "ignoring invalid configured plugin key during curated sync setup",
    )
    .into_iter()
    .filter(|plugin_id| is_openai_curated_marketplace_name(&plugin_id.marketplace_name))
    .collect::<Vec<_>>();
    configured_curated_plugin_ids.sort_unstable_by_key(PluginId::as_key);
    configured_curated_plugin_ids
}

pub fn configured_curated_plugin_ids_from_codex_home(codex_home: &Path) -> Vec<PluginId> {
    curated_plugin_ids_from_config_keys(configured_plugins_from_codex_home(
        codex_home,
        "failed to read user config while refreshing curated plugin cache",
        "failed to parse user config while refreshing curated plugin cache",
    ))
}

async fn load_plugin(
    config_name: String,
    plugin: &PluginConfig,
    store: &PluginStore,
    scope: &PluginLoadScope<'_>,
) -> LoadedPlugin<McpServerConfig> {
    let plugin_id = PluginId::parse(&config_name);
    let active_plugin_installation = plugin_id
        .as_ref()
        .ok()
        .and_then(|plugin_id| store.active_plugin_installation(plugin_id));
    let root = active_plugin_installation
        .as_ref()
        .map(|installation| installation.root.clone())
        .unwrap_or_else(|| match &plugin_id {
            Ok(plugin_id) => store.plugin_base_root(plugin_id),
            Err(_) => store.root().clone(),
        });
    let mut loaded_plugin = LoadedPlugin {
        config_name,
        remote_plugin_id: None,
        manifest_name: None,
        plugin_namespace: None,
        manifest_description: None,
        root,
        enabled: plugin.enabled,
        skill_roots: Vec::new(),
        skill_discovery_mode: SkillDiscoveryMode::Recursive,
        disabled_skill_paths: HashSet::new(),
        has_enabled_skills: false,
        mcp_servers: HashMap::new(),
        apps: Vec::new(),
        hook_sources: Vec::new(),
        hook_load_warnings: Vec::new(),
        error: None,
    };

    if !plugin.enabled {
        return loaded_plugin;
    }

    let (loaded_plugin_id, installation) = match plugin_id {
        Ok(plugin_id) => {
            let Some(installation) = active_plugin_installation else {
                loaded_plugin.error = Some("plugin is not installed".to_string());
                return loaded_plugin;
            };
            (plugin_id, installation)
        }
        Err(err) => {
            loaded_plugin.error = Some(err.to_string());
            return loaded_plugin;
        }
    };

    loaded_plugin.remote_plugin_id = match scope {
        PluginLoadScope::AllCapabilities {
            remote_plugin_id_resolver,
            ..
        } => remote_plugin_id_resolver.remote_plugin_id_for_installation(&installation),
        PluginLoadScope::HooksOnly => None,
    };

    let plugin_root = installation.root;

    if !plugin_root.as_path().is_dir() {
        loaded_plugin.error = Some("path does not exist or is not a directory".to_string());
        return loaded_plugin;
    }

    let Some(loaded_manifest) = load_plugin_manifest_with_format(plugin_root.as_path()) else {
        loaded_plugin.error = Some("missing or invalid plugin.json".to_string());
        return loaded_plugin;
    };
    loaded_plugin.skill_discovery_mode = match loaded_manifest.format {
        PluginManifestFormat::Legacy => SkillDiscoveryMode::Recursive,
        PluginManifestFormat::AgentPlugin => SkillDiscoveryMode::DirectChildren,
    };
    let manifest = loaded_manifest.manifest;

    let manifest_paths = &manifest.paths;
    let plugin_data_root = store.plugin_data_root(&loaded_plugin_id);
    let mcp_plugin_data_root = store.mcp_data_root(&loaded_plugin_id, loaded_manifest.format);
    loaded_plugin.plugin_namespace = Some(manifest.name.clone());
    match scope {
        PluginLoadScope::AllCapabilities {
            restriction_product,
            skill_config_rules,
            plugin_skill_snapshots,
            remote_plugin_id_resolver: _,
            skill_root_loader,
        } => {
            loaded_plugin.manifest_name = Some(manifest.display_name().to_string());
            loaded_plugin.manifest_description = manifest.description.clone();
            loaded_plugin.skill_roots =
                plugin_skill_roots(&plugin_root, manifest_paths, loaded_manifest.format);
            let plugin_identity = PluginIdentity {
                plugin_id: loaded_plugin_id.as_key(),
                remote_plugin_id: loaded_plugin.remote_plugin_id.clone(),
            };
            let resolved_skills = load_plugin_skill_inventory(
                &plugin_root,
                &plugin_identity,
                &manifest,
                loaded_manifest.format,
                *restriction_product,
                *plugin_skill_snapshots,
                *skill_root_loader,
            )
            .await
            .resolve(skill_config_rules);
            let has_enabled_skills = resolved_skills.has_enabled_skills();
            loaded_plugin.disabled_skill_paths = resolved_skills.disabled_skill_paths;
            loaded_plugin.has_enabled_skills = has_enabled_skills;
            loaded_plugin.mcp_servers = load_plugin_mcp_servers_from_manifest_with_format(
                plugin_root.as_path(),
                manifest_paths,
                Some(&plugin.mcp_servers),
                Some(mcp_plugin_data_root.as_path()),
                loaded_manifest.format,
            )
            .await;
            if loaded_manifest.format == PluginManifestFormat::Legacy {
                loaded_plugin.apps = load_plugin_apps(plugin_root.as_path()).await;
            }
        }
        PluginLoadScope::HooksOnly => {}
    }
    let (hook_sources, hook_load_warnings) =
        if loaded_manifest.format == PluginManifestFormat::AgentPlugin {
            (Vec::new(), Vec::new())
        } else {
            load_plugin_hooks(
                &plugin_root,
                &loaded_plugin_id,
                &plugin_data_root,
                manifest_paths,
            )
        };
    loaded_plugin.hook_sources = hook_sources;
    loaded_plugin.hook_load_warnings = hook_load_warnings;
    loaded_plugin
}

fn apply_plugin_mcp_server_policy(config: &mut McpServerConfig, policy: &PluginMcpServerConfig) {
    config.enabled = policy.enabled;
    if let Some(approval_mode) = policy.default_tools_approval_mode {
        config.default_tools_approval_mode = Some(approval_mode);
    }
    if let Some(enabled_tools) = &policy.enabled_tools {
        config.enabled_tools = Some(enabled_tools.clone());
    }
    if let Some(disabled_tools) = &policy.disabled_tools {
        config.disabled_tools = Some(disabled_tools.clone());
    }
    for (tool_name, tool_policy) in &policy.tools {
        let tool_config = config.tools.entry(tool_name.clone()).or_default();
        if let Some(approval_mode) = tool_policy.approval_mode {
            tool_config.approval_mode = Some(approval_mode);
        }
    }
}

pub(crate) struct PluginSkillInventory {
    skills: Vec<SkillMetadata>,
    had_errors: bool,
}

impl PluginSkillInventory {
    pub(crate) fn has_enabled_skills(&self, skill_config_rules: &SkillConfigRules) -> bool {
        contains_enabled_skill(
            &self.skills,
            &skill_config_rules.resolve_disabled_paths(
                self.skills
                    .iter()
                    .map(|skill| (skill.name.as_str(), &skill.path_to_skills_md)),
            ),
        )
    }

    pub(crate) fn resolve(self, skill_config_rules: &SkillConfigRules) -> ResolvedPluginSkills {
        let disabled_skill_paths = skill_config_rules.resolve_disabled_paths(
            self.skills
                .iter()
                .map(|skill| (skill.name.as_str(), &skill.path_to_skills_md)),
        );
        ResolvedPluginSkills {
            skills: self.skills,
            disabled_skill_paths,
            had_errors: self.had_errors,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedPluginSkills {
    pub skills: Vec<SkillMetadata>,
    pub disabled_skill_paths: HashSet<AbsolutePathBuf>,
    pub had_errors: bool,
}

impl ResolvedPluginSkills {
    pub fn has_enabled_skills(&self) -> bool {
        self.had_errors || contains_enabled_skill(&self.skills, &self.disabled_skill_paths)
    }
}

fn contains_enabled_skill(
    skills: &[SkillMetadata],
    disabled_skill_paths: &HashSet<AbsolutePathBuf>,
) -> bool {
    skills
        .iter()
        .any(|skill| !disabled_skill_paths.contains(&skill.path_to_skills_md))
}

pub(crate) async fn load_plugin_skill_inventory(
    plugin_root: &AbsolutePathBuf,
    plugin_identity: &PluginIdentity,
    manifest: &PluginManifest,
    manifest_format: PluginManifestFormat,
    restriction_product: Option<Product>,
    plugin_skill_snapshots: Option<&SkillRootSnapshots<PluginSkillRoot>>,
    skill_root_loader: &dyn SkillRootLoader<PluginSkillRoot>,
) -> PluginSkillInventory {
    let discovery_mode = match manifest_format {
        PluginManifestFormat::Legacy => SkillDiscoveryMode::Recursive,
        PluginManifestFormat::AgentPlugin => SkillDiscoveryMode::DirectChildren,
    };
    let roots = plugin_skill_roots(plugin_root, &manifest.paths, manifest_format)
        .into_iter()
        .map(|path| PluginSkillRoot {
            path,
            plugin_identity: plugin_identity.clone(),
            plugin_namespace: manifest.name.clone(),
            plugin_root: plugin_root.clone(),
            discovery_mode,
        })
        .collect();
    let outcome = skill_root_loader
        .load_roots(SkillRootLoadRequest {
            roots,
            restriction_product,
            snapshots: plugin_skill_snapshots.cloned(),
        })
        .await;

    PluginSkillInventory {
        skills: outcome.skills,
        had_errors: !outcome.errors.is_empty(),
    }
}

fn plugin_skill_roots(
    plugin_root: &AbsolutePathBuf,
    manifest_paths: &PluginManifestPaths,
    manifest_format: PluginManifestFormat,
) -> Vec<AbsolutePathBuf> {
    let mut paths = if manifest_paths.skills.is_empty() {
        default_skill_roots(plugin_root)
    } else {
        manifest_paths.skills.clone()
    };
    if manifest_format == PluginManifestFormat::Legacy {
        let migrated_command_skills = migrated_command_skills_root(plugin_root);
        if migrated_command_skills.is_dir() {
            paths.push(migrated_command_skills);
        }
    }
    paths.sort_unstable();
    paths.dedup();
    paths
}

fn default_skill_roots(plugin_root: &AbsolutePathBuf) -> Vec<AbsolutePathBuf> {
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
    if let Some(PluginManifestMcpServers::Path(path)) = &manifest_paths.mcp_servers {
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

pub async fn load_plugin_apps(plugin_root: &Path) -> Vec<AppDeclaration> {
    if let Some(loaded_manifest) = load_plugin_manifest_with_format(plugin_root) {
        if loaded_manifest.format == PluginManifestFormat::AgentPlugin {
            return Vec::new();
        }
        return load_plugin_apps_from_manifest(plugin_root, &loaded_manifest.manifest.paths).await;
    }
    load_apps_from_paths(plugin_root, default_app_config_paths(plugin_root)).await
}

pub(crate) async fn load_plugin_apps_from_manifest(
    plugin_root: &Path,
    manifest_paths: &PluginManifestPaths,
) -> Vec<AppDeclaration> {
    load_apps_from_paths(
        plugin_root,
        plugin_app_config_paths(plugin_root, manifest_paths),
    )
    .await
}

pub fn plugin_app_declarations_from_value(value: &JsonValue) -> Vec<AppDeclaration> {
    let Ok(mut apps) = parse_plugin_app_config_value(value.clone()) else {
        return Vec::new();
    };
    apps.retain(|app| !app.connector_id.0.trim().is_empty());
    let mut seen_connector_ids = HashSet::new();
    apps.retain(|app| seen_connector_ids.insert(app.connector_id.0.clone()));
    apps
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

// Discover plugin-bundled hooks from manifest `hooks` entries when present
// (path, paths, inline object, or inline objects), otherwise from the default
// `hooks/hooks.json` file.
pub fn load_plugin_hooks(
    plugin_root: &AbsolutePathBuf,
    plugin_id: &PluginId,
    plugin_data_root: &AbsolutePathBuf,
    manifest_paths: &PluginManifestPaths,
) -> (Vec<PluginHookSource>, Vec<String>) {
    let mut sources = Vec::new();
    let mut warnings = Vec::new();
    match &manifest_paths.hooks {
        Some(PluginManifestHooks::Paths(paths)) => {
            for path in paths {
                append_plugin_hook_file(
                    plugin_root,
                    plugin_id,
                    plugin_data_root,
                    path,
                    &mut sources,
                    &mut warnings,
                );
            }
        }
        Some(PluginManifestHooks::Inline(hooks_files)) => {
            let manifest_path = find_plugin_manifest_path(plugin_root.as_path())
                .and_then(|path| AbsolutePathBuf::try_from(path).ok())
                .unwrap_or_else(|| plugin_root.join(".codex-plugin/plugin.json"));
            for (index, hooks_file) in hooks_files.iter().enumerate() {
                if hooks_file.hooks.is_empty() {
                    continue;
                }
                sources.push(PluginHookSource {
                    plugin_id: plugin_id.clone(),
                    plugin_root: plugin_root.clone(),
                    plugin_data_root: plugin_data_root.clone(),
                    source_path: manifest_path.clone(),
                    source_relative_path: format!("plugin.json#hooks[{index}]"),
                    hooks: hooks_file.hooks.clone(),
                });
            }
        }
        None => {
            let default_path = plugin_root.join(DEFAULT_HOOKS_CONFIG_FILE);
            if default_path.as_path().is_file() {
                append_plugin_hook_file(
                    plugin_root,
                    plugin_id,
                    plugin_data_root,
                    &default_path,
                    &mut sources,
                    &mut warnings,
                );
            }
        }
    }
    (sources, warnings)
}

// Append one resolved plugin hook file, keeping source metadata for runtime
// reporting and collecting load warnings for startup surfacing.
fn append_plugin_hook_file(
    plugin_root: &AbsolutePathBuf,
    plugin_id: &PluginId,
    plugin_data_root: &AbsolutePathBuf,
    path: &AbsolutePathBuf,
    sources: &mut Vec<PluginHookSource>,
    warnings: &mut Vec<String>,
) {
    let contents = match fs::read_to_string(path.as_path()) {
        Ok(contents) => contents,
        Err(err) => {
            warnings.push(format!(
                "failed to read plugin hooks config {}: {err}",
                path.display()
            ));
            return;
        }
    };
    let parsed = match serde_json::from_str::<HooksFile>(&contents) {
        Ok(parsed) => parsed,
        Err(err) => {
            warnings.push(format!(
                "failed to parse plugin hooks config {}: {err}",
                path.display()
            ));
            return;
        }
    };
    if parsed.hooks.is_empty() {
        return;
    }

    let source_relative_path = path
        .as_path()
        .strip_prefix(plugin_root.as_path())
        .unwrap_or(path.as_path())
        .to_string_lossy()
        .replace('\\', "/");

    sources.push(PluginHookSource {
        plugin_id: plugin_id.clone(),
        plugin_root: plugin_root.clone(),
        plugin_data_root: plugin_data_root.clone(),
        source_path: path.clone(),
        source_relative_path,
        hooks: parsed.hooks,
    });
}

async fn load_apps_from_paths(
    plugin_root: &Path,
    app_config_paths: Vec<AbsolutePathBuf>,
) -> Vec<AppDeclaration> {
    let mut app_declarations = Vec::new();
    for app_config_path in app_config_paths {
        let Ok(contents) = tokio::fs::read_to_string(app_config_path.as_path()).await else {
            continue;
        };
        let declarations = match parse_plugin_app_config(&contents) {
            Ok(declarations) => declarations,
            Err(err) => {
                warn!(
                    path = %app_config_path.display(),
                    "failed to parse plugin app config: {err}"
                );
                continue;
            }
        };

        app_declarations.extend(declarations.into_iter().filter(|app| {
            if app.connector_id.0.trim().is_empty() {
                warn!(
                    plugin = %plugin_root.display(),
                    "plugin app config is missing an app id"
                );
                false
            } else {
                true
            }
        }));
    }
    app_declarations
}

pub async fn plugin_capability_summary_from_root(
    plugin_id: &PluginId,
    plugin_root: &AbsolutePathBuf,
    skill_root_loader: &dyn SkillRootLoader<PluginSkillRoot>,
) -> Option<PluginCapabilitySummary> {
    let loaded_manifest = load_plugin_manifest_with_format(plugin_root.as_path())?;
    let manifest_format = loaded_manifest.format;
    let manifest = loaded_manifest.manifest;
    let plugin_identity = PluginIdentity {
        plugin_id: plugin_id.as_key(),
        remote_plugin_id: None,
    };

    let manifest_paths = &manifest.paths;
    let has_skills = match manifest_format {
        PluginManifestFormat::Legacy => {
            !plugin_skill_roots(plugin_root, manifest_paths, manifest_format).is_empty()
        }
        PluginManifestFormat::AgentPlugin => {
            !load_plugin_skill_inventory(
                plugin_root,
                &plugin_identity,
                &manifest,
                manifest_format,
                /*restriction_product*/ None,
                /*plugin_skill_snapshots*/ None,
                skill_root_loader,
            )
            .await
            .skills
            .is_empty()
        }
    };
    let mut mcp_server_names = load_plugin_mcp_servers_from_manifest_with_format(
        plugin_root.as_path(),
        manifest_paths,
        /*plugin_policy*/ None,
        /*plugin_data_root*/ None,
        manifest_format,
    )
    .await
    .into_keys()
    .collect::<Vec<_>>();
    mcp_server_names.sort_unstable();
    mcp_server_names.dedup();

    let app_declarations = if manifest_format == PluginManifestFormat::AgentPlugin {
        Vec::new()
    } else {
        load_plugin_apps_from_manifest(plugin_root.as_path(), manifest_paths).await
    };
    let app_connector_ids = app_connector_ids_from_declarations(&app_declarations);

    Some(PluginCapabilitySummary {
        config_name: plugin_id.as_key(),
        display_name: plugin_id.plugin_name.clone(),
        plugin_namespace: Some(manifest.name.clone()),
        description: None,
        has_skills,
        mcp_server_names,
        app_connector_ids,
    })
}

/// Loads plugin MCP servers without applying user-specific policy overrides.
pub async fn load_plugin_mcp_servers(
    plugin_root: &Path,
    auth_mode: Option<AuthMode>,
) -> HashMap<String, McpServerConfig> {
    load_plugin_mcp_servers_with_policy(plugin_root, auth_mode, /*plugin_policy*/ None).await
}

/// Loads plugin MCP servers with the effective user policy for an installed plugin.
pub async fn load_configured_plugin_mcp_servers(
    plugin_root: &Path,
    auth_mode: Option<AuthMode>,
    plugin_id: &PluginId,
    config_layer_stack: &ConfigLayerStack,
    codex_home: &Path,
) -> HashMap<String, McpServerConfig> {
    let configured_plugins = configured_plugins_from_stack(config_layer_stack, codex_home);
    let plugin_id = plugin_id.as_key();
    let plugin_policy = configured_plugins
        .get(&plugin_id)
        .map(|plugin| &plugin.mcp_servers);

    load_plugin_mcp_servers_with_policy(plugin_root, auth_mode, plugin_policy).await
}

/// Resolves effective per-plugin MCP policies without validating opaque selected-root IDs.
pub fn configured_plugin_mcp_server_policies(
    config_layer_stack: &ConfigLayerStack,
) -> HashMap<String, HashMap<String, PluginMcpServerConfig>> {
    config_layer_stack
        .effective_user_config()
        .map(|config| configured_plugins_from_user_config_value(&config))
        .unwrap_or_default()
        .into_iter()
        .map(|(plugin_id, plugin)| (plugin_id, plugin.mcp_servers))
        .collect()
}

/// Applies user policy without widening the selected plugin's declared restrictions.
pub fn apply_configured_plugin_mcp_server_policies(
    policies: &HashMap<String, PluginMcpServerConfig>,
    servers: &mut HashMap<String, McpServerConfig>,
) {
    for (name, server) in servers {
        if let Some(policy) = policies.get(name) {
            let declared_approval_mode = server.default_tools_approval_mode.unwrap_or_default();
            server.enabled &= policy.enabled;

            if let Some(approval_mode) = policy.default_tools_approval_mode {
                server.default_tools_approval_mode =
                    Some(declared_approval_mode.restrict_to(approval_mode));
            }
            if let Some(enabled_tools) = &policy.enabled_tools {
                match &mut server.enabled_tools {
                    Some(declared_tools) => {
                        declared_tools.retain(|tool| enabled_tools.contains(tool));
                    }
                    None => server.enabled_tools = Some(enabled_tools.clone()),
                }
            }
            if let Some(disabled_tools) = &policy.disabled_tools {
                let declared_tools = server.disabled_tools.get_or_insert_default();
                for tool in disabled_tools {
                    if !declared_tools.contains(tool) {
                        declared_tools.push(tool.clone());
                    }
                }
            }
            for (tool_name, tool_policy) in &policy.tools {
                if tool_policy.approval_mode.is_some() {
                    server.tools.entry(tool_name.clone()).or_default();
                }
            }
            for (tool_name, tool_config) in &mut server.tools {
                if let Some(approval_mode) = policy
                    .tools
                    .get(tool_name)
                    .and_then(|tool_policy| tool_policy.approval_mode)
                    .or(policy.default_tools_approval_mode)
                {
                    tool_config.approval_mode = Some(
                        tool_config
                            .approval_mode
                            .unwrap_or(declared_approval_mode)
                            .restrict_to(approval_mode),
                    );
                }
            }
        }
    }
}

async fn load_plugin_mcp_servers_with_policy(
    plugin_root: &Path,
    auth_mode: Option<AuthMode>,
    plugin_policy: Option<&HashMap<String, PluginMcpServerConfig>>,
) -> HashMap<String, McpServerConfig> {
    let mut mcp_servers = load_declared_plugin_mcp_servers(plugin_root, plugin_policy).await;
    if !apps_route_available(auth_mode) || mcp_servers.is_empty() {
        return mcp_servers;
    }

    let mut app_declarations = load_plugin_apps(plugin_root).await;
    apply_app_mcp_routing_policy(
        &mut app_declarations,
        &mut mcp_servers,
        auth_mode,
        /*plugin_active*/ true,
    );
    mcp_servers
}

async fn load_declared_plugin_mcp_servers(
    plugin_root: &Path,
    plugin_policy: Option<&HashMap<String, PluginMcpServerConfig>>,
) -> HashMap<String, McpServerConfig> {
    let Some(loaded_manifest) = load_plugin_manifest_with_format(plugin_root) else {
        return HashMap::new();
    };

    load_plugin_mcp_servers_from_manifest_with_format(
        plugin_root,
        &loaded_manifest.manifest.paths,
        plugin_policy,
        /*plugin_data_root*/ None,
        loaded_manifest.format,
    )
    .await
}

pub(crate) async fn load_plugin_mcp_servers_from_manifest_with_format(
    plugin_root: &Path,
    manifest_paths: &PluginManifestPaths,
    plugin_policy: Option<&HashMap<String, PluginMcpServerConfig>>,
    plugin_data_root: Option<&Path>,
    manifest_format: PluginManifestFormat,
) -> HashMap<String, McpServerConfig> {
    let mut mcp_servers = HashMap::new();
    match &manifest_paths.mcp_servers {
        Some(PluginManifestMcpServers::Object(object_servers)) => {
            let plugin_mcp = load_mcp_servers_from_manifest_object(plugin_root, object_servers);
            for (name, mut config) in plugin_mcp.mcp_servers {
                if let Some(policy) = plugin_policy.and_then(|policy| policy.get(&name)) {
                    apply_plugin_mcp_server_policy(&mut config, policy);
                }
                if mcp_servers.insert(name.clone(), config).is_some() {
                    warn!(
                        plugin = %plugin_root.display(),
                        server = name,
                        "plugin manifest MCP object overwrote an earlier server definition"
                    );
                }
            }
        }
        Some(PluginManifestMcpServers::Path(_)) | None => {
            for mcp_config_path in plugin_mcp_config_paths(plugin_root, manifest_paths) {
                let plugin_mcp = load_mcp_servers_from_file(
                    plugin_root,
                    plugin_data_root,
                    manifest_format,
                    &mcp_config_path,
                )
                .await;
                for (name, mut config) in plugin_mcp.mcp_servers {
                    if let Some(policy) = plugin_policy.and_then(|policy| policy.get(&name)) {
                        apply_plugin_mcp_server_policy(&mut config, policy);
                    }
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
        }
    }

    if manifest_format == PluginManifestFormat::AgentPlugin {
        agent_plugin_mcp_overlay::apply_codex_env_overlay(plugin_root, &mut mcp_servers).await;
    }

    mcp_servers
}

async fn load_mcp_servers_from_file(
    plugin_root: &Path,
    plugin_data_root: Option<&Path>,
    manifest_format: PluginManifestFormat,
    mcp_config_path: &AbsolutePathBuf,
) -> PluginMcpDiscovery {
    let is_agent_plugin_mcp = manifest_format == PluginManifestFormat::AgentPlugin;
    if is_agent_plugin_mcp {
        match tokio::fs::symlink_metadata(mcp_config_path.as_path()).await {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                warn!(
                    path = %mcp_config_path.display(),
                    "Agent Plugins MCP config is not a regular file; disabling MCP"
                );
                return PluginMcpDiscovery::default();
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return PluginMcpDiscovery::default();
            }
            Err(err) => {
                warn!(
                    path = %mcp_config_path.display(),
                    "failed to inspect Agent Plugins MCP config; disabling MCP: {err}"
                );
                return PluginMcpDiscovery::default();
            }
        }
        let resolved_root = match tokio::fs::canonicalize(plugin_root).await {
            Ok(path) => path,
            Err(err) => {
                warn!(
                    plugin = %plugin_root.display(),
                    "failed to resolve Agent Plugins root; disabling MCP: {err}"
                );
                return PluginMcpDiscovery::default();
            }
        };
        let resolved_config = match tokio::fs::canonicalize(mcp_config_path.as_path()).await {
            Ok(path) => path,
            Err(err) => {
                warn!(
                    path = %mcp_config_path.display(),
                    "failed to resolve Agent Plugins MCP config; disabling MCP: {err}"
                );
                return PluginMcpDiscovery::default();
            }
        };
        if !resolved_config.starts_with(&resolved_root) {
            warn!(
                plugin = %plugin_root.display(),
                path = %mcp_config_path.display(),
                "Agent Plugins MCP config resolves outside the plugin root; disabling MCP"
            );
            return PluginMcpDiscovery::default();
        }
    }
    let Ok(contents) = tokio::fs::read_to_string(mcp_config_path.as_path()).await else {
        return PluginMcpDiscovery::default();
    };
    let fallback_data_root = plugin_root.join(".plugin-data");
    let mut parsed = match if is_agent_plugin_mcp {
        parse_agent_plugin_mcp_config(
            plugin_root,
            plugin_data_root.unwrap_or(&fallback_data_root),
            &contents,
        )
    } else {
        parse_plugin_mcp_config(plugin_root, &contents)
    } {
        Ok(parsed) => parsed,
        Err(err) => {
            warn!(
                path = %mcp_config_path.display(),
                "failed to parse plugin MCP config: {err}"
            );
            return PluginMcpDiscovery::default();
        }
    };
    if is_agent_plugin_mcp
        && let Some(plugin_data_root) = plugin_data_root
        && parsed
            .servers
            .values()
            .any(|server| matches!(&server.transport, McpServerTransportConfig::Stdio { .. }))
        && let Err(err) = tokio::fs::create_dir_all(plugin_data_root).await
    {
        warn!(
            plugin = %plugin_root.display(),
            path = %plugin_data_root.display(),
            "failed to create Agent Plugins data directory; disabling stdio MCP servers: {err}"
        );
        parsed.servers.retain(|_, server| {
            !matches!(&server.transport, McpServerTransportConfig::Stdio { .. })
        });
    }
    for error in parsed.errors {
        warn!(
            plugin = %plugin_root.display(),
            server = error.name,
            path = %mcp_config_path.display(),
            error = error.message,
            "failed to parse plugin MCP server"
        );
    }
    PluginMcpDiscovery {
        mcp_servers: parsed.servers.into_iter().collect(),
    }
}

fn load_mcp_servers_from_manifest_object(
    plugin_root: &Path,
    object_config: &str,
) -> PluginMcpDiscovery {
    let parsed = match parse_plugin_mcp_config(plugin_root, object_config) {
        Ok(parsed) => parsed,
        Err(err) => {
            warn!(
                plugin = %plugin_root.display(),
                "failed to parse plugin manifest MCP object: {err}"
            );
            return PluginMcpDiscovery::default();
        }
    };
    for error in parsed.errors {
        warn!(
            plugin = %plugin_root.display(),
            server = error.name,
            error = error.message,
            "failed to parse plugin manifest MCP object server"
        );
    }
    PluginMcpDiscovery {
        mcp_servers: parsed.servers.into_iter().collect(),
    }
}

#[derive(Debug, Default)]
struct PluginMcpDiscovery {
    mcp_servers: HashMap<String, McpServerConfig>,
}

#[derive(Debug)]
pub struct MaterializedMarketplacePluginSource {
    pub path: AbsolutePathBuf,
    _tempdir: Option<TempDir>,
}

pub fn materialize_marketplace_plugin_source(
    codex_home: &Path,
    source: &MarketplacePluginSource,
) -> Result<MaterializedMarketplacePluginSource, String> {
    materialize_marketplace_plugin_source_with_mode(codex_home, source, PluginGitMode::Manual)
}

/// Applies the initiating operation's Git trust policy throughout plugin materialization.
pub(crate) fn materialize_marketplace_plugin_source_with_mode(
    codex_home: &Path,
    source: &MarketplacePluginSource,
    mode: PluginGitMode,
) -> Result<MaterializedMarketplacePluginSource, String> {
    match source {
        MarketplacePluginSource::Local { path } => Ok(MaterializedMarketplacePluginSource {
            path: path.clone(),
            _tempdir: None,
        }),
        MarketplacePluginSource::Git {
            url,
            path,
            ref_name,
            sha,
        } => {
            let staging_root = codex_home.join("plugins/.marketplace-plugin-source-staging");
            fs::create_dir_all(&staging_root).map_err(|err| {
                format!(
                    "failed to create marketplace plugin source staging directory {}: {err}",
                    staging_root.display()
                )
            })?;
            let tempdir = tempfile::Builder::new()
                .prefix("marketplace-plugin-source-")
                .tempdir_in(&staging_root)
                .map_err(|err| {
                    format!(
                        "failed to create marketplace plugin source staging directory in {}: {err}",
                        staging_root.display()
                    )
                })?;
            clone_git_plugin_source(
                codex_home,
                url,
                ref_name.as_deref(),
                sha.as_deref(),
                path.as_deref(),
                tempdir.path(),
                mode,
            )?;
            let path = if let Some(path) = path {
                AbsolutePathBuf::try_from(tempdir.path().join(path)).map_err(|err| {
                    format!("failed to resolve materialized plugin source path: {err}")
                })?
            } else {
                AbsolutePathBuf::try_from(tempdir.path().to_path_buf()).map_err(|err| {
                    format!("failed to resolve materialized plugin source path: {err}")
                })?
            };
            Ok(MaterializedMarketplacePluginSource {
                path,
                _tempdir: Some(tempdir),
            })
        }
        MarketplacePluginSource::Npm {
            package,
            version,
            registry,
        } => {
            let (path, tempdir) = materialize_npm_plugin_source(
                codex_home,
                package,
                version.as_deref(),
                registry.as_deref(),
            )?;
            Ok(MaterializedMarketplacePluginSource {
                path,
                _tempdir: Some(tempdir),
            })
        }
    }
}

fn clone_git_plugin_source(
    codex_home: &Path,
    url: &str,
    ref_name: Option<&str>,
    sha: Option<&str>,
    sparse_checkout_path: Option<&str>,
    destination: &Path,
    mode: PluginGitMode,
) -> Result<(), String> {
    let clone_cwd = match mode {
        PluginGitMode::Automatic => Some(codex_home),
        PluginGitMode::Manual => None,
    };
    if let Some(sparse_checkout_path) = sparse_checkout_path {
        run_git(
            &[
                "clone",
                "--filter=blob:none",
                "--sparse",
                "--no-checkout",
                url,
                destination.to_string_lossy().as_ref(),
            ],
            clone_cwd,
            mode,
        )?;
        run_git(
            &[
                "sparse-checkout",
                "set",
                "--no-cone",
                "--",
                sparse_checkout_path,
            ],
            Some(destination),
            mode,
        )?;
    } else {
        run_git(
            &["clone", url, destination.to_string_lossy().as_ref()],
            clone_cwd,
            mode,
        )?;
    }
    if let Some(sha) = sha {
        run_git(&["checkout", sha], Some(destination), mode)?;
        let checked_out_sha = run_git_output(&["rev-parse", "HEAD"], Some(destination), mode)?;
        if !checked_out_sha.eq_ignore_ascii_case(sha) {
            return Err(format!(
                "checked out Git SHA {checked_out_sha} does not match requested SHA {sha}"
            ));
        }
    } else if let Some(ref_name) = ref_name {
        run_git(&["checkout", ref_name], Some(destination), mode)?;
    } else if sparse_checkout_path.is_some() {
        run_git(&["checkout"], Some(destination), mode)?;
    }
    Ok(())
}

fn run_git(args: &[&str], cwd: Option<&Path>, mode: PluginGitMode) -> Result<(), String> {
    run_git_output(args, cwd, mode).map(drop)
}

fn run_git_output(
    args: &[&str],
    cwd: Option<&Path>,
    mode: PluginGitMode,
) -> Result<String, String> {
    let mut command = mode.command(Path::new("git"));
    command.args(args);
    command.env("GIT_TERMINAL_PROMPT", "0");
    let _trusted_repository = if let Some(cwd) = cwd
        && args.first() == Some(&"clone")
    {
        Some(crate::configure_trusted_git_repository(&mut command, cwd)?)
    } else {
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
            if matches!(mode, PluginGitMode::Manual) {
                command.env_remove("GIT_DIR");
            }
        }
        None
    };

    let output = command
        .output()
        .map_err(|err| format!("failed to run git {}: {err}", args.join(" ")))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    Err(format!(
        "git {} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;
