use crate::app_mcp_routing::apply_app_mcp_routing_policy;
use crate::app_mcp_routing::apps_route_available;
use crate::command_migration::migrated_command_skills_root;
use crate::is_openai_curated_marketplace_name;
use crate::manifest::PluginManifest;
use crate::manifest::PluginManifestHooks;
use crate::manifest::PluginManifestMcpServers;
use crate::manifest::PluginManifestPaths;
use crate::manifest::load_plugin_manifest;
use crate::marketplace::MarketplacePluginSource;
use crate::marketplace::find_marketplace_plugin;
use crate::marketplace::list_marketplaces_with_home;
use crate::marketplace::load_marketplace;
use crate::marketplace_policy::configured_plugins_from_stack;
use crate::npm_source::materialize_npm_plugin_source;
use crate::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use crate::remote::RemoteInstalledPlugin;
use crate::store::PluginStore;
use crate::store::plugin_version_for_source;
use crate::store::plugin_version_for_source_with_fallback_manifest;
use codex_config::ConfigLayerStack;
use codex_config::HooksFile;
use codex_config::types::McpServerConfig;
use codex_config::types::PluginConfig;
use codex_config::types::PluginMcpServerConfig;
use codex_connectors::parse_plugin_app_config;
use codex_connectors::parse_plugin_app_config_value;
use codex_core_skills::PluginSkillSnapshots;
use codex_core_skills::config_rules::resolve_disabled_skill_paths;
use codex_core_skills::config_rules::skill_config_rules_from_stack;
use codex_core_skills::loader::SkillRoot;
use codex_core_skills::loader::load_skills_from_roots;
use codex_exec_server::LOCAL_FS;
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
use codex_protocol::protocol::SkillScope;
use codex_skills::SkillConfigRules;
use codex_skills::SkillMetadata;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::find_plugin_manifest_path;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Semaphore;
use tracing::instrument;
use tracing::warn;

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

enum PluginLoadScope<'a> {
    AllCapabilities {
        restriction_product: Option<Product>,
        skill_config_rules: &'a SkillConfigRules,
        plugin_skill_snapshots: Option<&'a PluginSkillSnapshots>,
        root_scan_slots: Arc<Semaphore>,
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
    extra_plugins: HashMap<String, PluginConfig>,
    store: &PluginStore,
    plugin_skill_snapshots: Option<&PluginSkillSnapshots>,
    restriction_product: Option<Product>,
    remote_global_catalog_active: bool,
    root_scan_slots: Arc<Semaphore>,
) -> Vec<LoadedPlugin<McpServerConfig>> {
    let skill_config_rules = skill_config_rules_from_stack(config_layer_stack);
    load_plugins_from_layer_stack_with_scope(
        config_layer_stack,
        extra_plugins,
        store,
        remote_global_catalog_active,
        PluginLoadScope::AllCapabilities {
            restriction_product,
            skill_config_rules: &skill_config_rules,
            plugin_skill_snapshots,
            root_scan_slots,
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
    remote_global_catalog_active: bool,
) -> PluginHookLoadOutcome {
    let plugins = load_plugins_from_layer_stack_with_scope(
        config_layer_stack,
        extra_plugins,
        store,
        remote_global_catalog_active,
        PluginLoadScope::HooksOnly,
    )
    .await;
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
        configured_plugins.extend(extra_plugins);
        return configured_plugins;
    }

    let mut local_curated_installed_plugin_keys = HashMap::<String, Vec<String>>::new();
    for plugin_key in configured_plugins.keys() {
        let Ok(plugin_id) = PluginId::parse(plugin_key) else {
            continue;
        };
        if !is_openai_curated_marketplace_name(&plugin_id.marketplace_name)
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

        configured_plugins.insert(plugin_key, plugin_config);
    }

    configured_plugins
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
    ))
}

pub(crate) fn refresh_non_curated_plugin_cache_detailed(
    codex_home: &Path,
    additional_roots: &[AbsolutePathBuf],
    configured_plugin_keys: &[String],
) -> Result<NonCuratedCacheRefreshOutcome, String> {
    refresh_non_curated_plugin_cache_with_mode(
        codex_home,
        additional_roots,
        configured_plugin_keys,
        NonCuratedCacheRefreshMode::IfVersionChanged,
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
    ))
}

pub(crate) fn refresh_non_curated_plugin_cache_force_reinstall_detailed(
    codex_home: &Path,
    additional_roots: &[AbsolutePathBuf],
    configured_plugin_keys: &[String],
) -> Result<NonCuratedCacheRefreshOutcome, String> {
    refresh_non_curated_plugin_cache_with_mode(
        codex_home,
        additional_roots,
        configured_plugin_keys,
        NonCuratedCacheRefreshMode::ForceReinstall,
    )
}

fn refresh_non_curated_plugin_cache_with_mode(
    codex_home: &Path,
    additional_roots: &[AbsolutePathBuf],
    configured_plugin_keys: &[String],
    mode: NonCuratedCacheRefreshMode,
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
                materialize_marketplace_plugin_source(codex_home, &source).map_err(|err| {
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
    let active_plugin_root = plugin_id
        .as_ref()
        .ok()
        .and_then(|plugin_id| store.active_plugin_root(plugin_id));
    let root = active_plugin_root
        .clone()
        .unwrap_or_else(|| match &plugin_id {
            Ok(plugin_id) => store.plugin_base_root(plugin_id),
            Err(_) => store.root().clone(),
        });
    let mut loaded_plugin = LoadedPlugin {
        config_name,
        manifest_name: None,
        plugin_namespace: None,
        manifest_description: None,
        root,
        enabled: plugin.enabled,
        skill_roots: Vec::new(),
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

    let (loaded_plugin_id, plugin_root) = match plugin_id {
        Ok(plugin_id) => {
            let Some(plugin_root) = active_plugin_root else {
                loaded_plugin.error = Some("plugin is not installed".to_string());
                return loaded_plugin;
            };
            (plugin_id, plugin_root)
        }
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
        loaded_plugin.error = Some("missing or invalid plugin.json".to_string());
        return loaded_plugin;
    };

    let manifest_paths = &manifest.paths;
    loaded_plugin.plugin_namespace = Some(manifest.name.clone());
    match scope {
        PluginLoadScope::AllCapabilities {
            restriction_product,
            skill_config_rules,
            plugin_skill_snapshots,
            root_scan_slots,
        } => {
            loaded_plugin.manifest_name = Some(manifest.display_name().to_string());
            loaded_plugin.manifest_description = manifest.description.clone();
            loaded_plugin.skill_roots = plugin_skill_roots(&plugin_root, manifest_paths);
            let resolved_skills = load_plugin_skills(
                &plugin_root,
                &loaded_plugin_id,
                &manifest,
                *restriction_product,
                skill_config_rules,
                *plugin_skill_snapshots,
                Arc::clone(root_scan_slots),
            )
            .await;
            let has_enabled_skills = resolved_skills.has_enabled_skills();
            loaded_plugin.disabled_skill_paths = resolved_skills.disabled_skill_paths;
            loaded_plugin.has_enabled_skills = has_enabled_skills;
            loaded_plugin.mcp_servers = load_plugin_mcp_servers_from_manifest(
                plugin_root.as_path(),
                manifest_paths,
                Some(&plugin.mcp_servers),
            )
            .await;
            loaded_plugin.apps = load_plugin_apps(plugin_root.as_path()).await;
        }
        PluginLoadScope::HooksOnly => {}
    }
    let (hook_sources, hook_load_warnings) = load_plugin_hooks(
        &plugin_root,
        &loaded_plugin_id,
        &store.plugin_data_root(&loaded_plugin_id),
        manifest_paths,
    );
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
            &resolve_disabled_skill_paths(&self.skills, skill_config_rules),
        )
    }

    fn resolve(self, skill_config_rules: &SkillConfigRules) -> ResolvedPluginSkills {
        let disabled_skill_paths = resolve_disabled_skill_paths(&self.skills, skill_config_rules);
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

pub async fn load_plugin_skills(
    plugin_root: &AbsolutePathBuf,
    plugin_id: &PluginId,
    manifest: &PluginManifest,
    restriction_product: Option<Product>,
    skill_config_rules: &SkillConfigRules,
    plugin_skill_snapshots: Option<&PluginSkillSnapshots>,
    root_scan_slots: Arc<Semaphore>,
) -> ResolvedPluginSkills {
    load_plugin_skill_inventory(
        plugin_root,
        plugin_id,
        manifest,
        restriction_product,
        plugin_skill_snapshots,
        root_scan_slots,
    )
    .await
    .resolve(skill_config_rules)
}

pub(crate) async fn load_plugin_skill_inventory(
    plugin_root: &AbsolutePathBuf,
    plugin_id: &PluginId,
    manifest: &PluginManifest,
    restriction_product: Option<Product>,
    plugin_skill_snapshots: Option<&PluginSkillSnapshots>,
    root_scan_slots: Arc<Semaphore>,
) -> PluginSkillInventory {
    let roots = plugin_skill_roots(plugin_root, &manifest.paths)
        .into_iter()
        .map(|path| SkillRoot {
            path,
            scope: SkillScope::User,
            file_system: Arc::clone(&LOCAL_FS),
            plugin_id: Some(plugin_id.as_key()),
            plugin_namespace: Some(manifest.name.clone()),
            plugin_root: Some(plugin_root.clone()),
        })
        .collect::<Vec<_>>();
    let outcome = load_skills_from_roots(roots, plugin_skill_snapshots, root_scan_slots).await;
    let had_errors = !outcome.errors.is_empty();
    let migrated_command_skills = migrated_command_skills_root(plugin_root);
    let migrated_command_skills = fs::canonicalize(migrated_command_skills.as_path())
        .ok()
        .and_then(|path| AbsolutePathBuf::from_absolute_path_checked(path).ok())
        .unwrap_or(migrated_command_skills);
    let skills = outcome
        .skills
        .into_iter()
        .filter(|skill| skill.matches_product_restriction_for_product(restriction_product))
        .collect::<Vec<_>>();
    let native_skill_names = skills
        .iter()
        .filter(|skill| {
            !skill
                .path_to_skills_md
                .as_path()
                .starts_with(migrated_command_skills.as_path())
        })
        .map(|skill| skill.name.clone())
        .collect::<HashSet<_>>();
    let skills = skills
        .into_iter()
        .filter(|skill| {
            !skill
                .path_to_skills_md
                .as_path()
                .starts_with(migrated_command_skills.as_path())
                || !native_skill_names.contains(&skill.name)
        })
        .collect::<Vec<_>>();

    PluginSkillInventory { skills, had_errors }
}

fn plugin_skill_roots(
    plugin_root: &AbsolutePathBuf,
    manifest_paths: &PluginManifestPaths,
) -> Vec<AbsolutePathBuf> {
    let mut paths = if manifest_paths.skills.is_empty() {
        default_skill_roots(plugin_root)
    } else {
        manifest_paths.skills.clone()
    };
    let migrated_command_skills = migrated_command_skills_root(plugin_root);
    if migrated_command_skills.is_dir() {
        paths.push(migrated_command_skills);
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
    if let Some(manifest) = load_plugin_manifest(plugin_root) {
        return load_plugin_apps_from_manifest(plugin_root, &manifest.paths).await;
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
) -> Option<PluginCapabilitySummary> {
    let manifest = load_plugin_manifest(plugin_root.as_path())?;

    let manifest_paths = &manifest.paths;
    let has_skills = !plugin_skill_roots(plugin_root, manifest_paths).is_empty();
    let mut mcp_server_names = load_plugin_mcp_servers_from_manifest(
        plugin_root.as_path(),
        manifest_paths,
        /*plugin_policy*/ None,
    )
    .await
    .into_keys()
    .collect::<Vec<_>>();
    mcp_server_names.sort_unstable();
    mcp_server_names.dedup();

    let app_declarations = load_apps_from_paths(
        plugin_root.as_path(),
        plugin_app_config_paths(plugin_root.as_path(), manifest_paths),
    )
    .await;
    let app_connector_ids = app_connector_ids_from_declarations(&app_declarations);

    Some(PluginCapabilitySummary {
        config_name: plugin_id.as_key(),
        display_name: plugin_id.plugin_name.clone(),
        description: None,
        has_skills,
        mcp_server_names,
        app_connector_ids,
    })
}

pub async fn load_plugin_mcp_servers(
    plugin_root: &Path,
    auth_mode: Option<AuthMode>,
) -> HashMap<String, McpServerConfig> {
    let mut mcp_servers = load_declared_plugin_mcp_servers(plugin_root).await;
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

async fn load_declared_plugin_mcp_servers(plugin_root: &Path) -> HashMap<String, McpServerConfig> {
    let Some(manifest) = load_plugin_manifest(plugin_root) else {
        return HashMap::new();
    };

    load_plugin_mcp_servers_from_manifest(plugin_root, &manifest.paths, /*plugin_policy*/ None)
        .await
}

pub(crate) async fn load_plugin_mcp_servers_from_manifest(
    plugin_root: &Path,
    manifest_paths: &PluginManifestPaths,
    plugin_policy: Option<&HashMap<String, PluginMcpServerConfig>>,
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
                let plugin_mcp = load_mcp_servers_from_file(plugin_root, &mcp_config_path).await;
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

    mcp_servers
}

async fn load_mcp_servers_from_file(
    plugin_root: &Path,
    mcp_config_path: &AbsolutePathBuf,
) -> PluginMcpDiscovery {
    let Ok(contents) = tokio::fs::read_to_string(mcp_config_path.as_path()).await else {
        return PluginMcpDiscovery::default();
    };
    let parsed = match parse_plugin_mcp_config(plugin_root, &contents) {
        Ok(parsed) => parsed,
        Err(err) => {
            warn!(
                path = %mcp_config_path.display(),
                "failed to parse plugin MCP config: {err}"
            );
            return PluginMcpDiscovery::default();
        }
    };
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
                url,
                ref_name.as_deref(),
                sha.as_deref(),
                path.as_deref(),
                tempdir.path(),
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
    url: &str,
    ref_name: Option<&str>,
    sha: Option<&str>,
    sparse_checkout_path: Option<&str>,
    destination: &Path,
) -> Result<(), String> {
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
            /*cwd*/ None,
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
        )?;
    } else {
        run_git(
            &["clone", url, destination.to_string_lossy().as_ref()],
            /*cwd*/ None,
        )?;
    }
    if let Some(sha) = sha {
        run_git(&["checkout", sha], Some(destination))?;
        let checked_out_sha = run_git_output(&["rev-parse", "HEAD"], Some(destination))?;
        if !checked_out_sha.eq_ignore_ascii_case(sha) {
            return Err(format!(
                "checked out Git SHA {checked_out_sha} does not match requested SHA {sha}"
            ));
        }
    } else if let Some(ref_name) = ref_name {
        run_git(&["checkout", ref_name], Some(destination))?;
    } else if sparse_checkout_path.is_some() {
        run_git(&["checkout"], Some(destination))?;
    }
    Ok(())
}

fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<(), String> {
    run_git_output(args, cwd).map(drop)
}

fn run_git_output(args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let mut command = Command::new("git");
    command.args(args);
    command.env("GIT_TERMINAL_PROMPT", "0");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }

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
