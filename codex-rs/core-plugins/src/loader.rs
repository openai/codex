use crate::manifest::PluginManifestPaths;
use crate::manifest::load_plugin_manifest;
use crate::marketplace::MarketplacePluginSource;
use crate::marketplace::list_marketplaces;
use crate::marketplace::load_marketplace;
use crate::store::PluginStore;
use crate::store::plugin_version_for_source;
use codex_config::ConfigLayerStack;
use codex_config::types::McpServerConfig;
use codex_config::types::PluginConfig;
use codex_core_skills::SkillMetadata;
use codex_core_skills::config_rules::SkillConfigRules;
use codex_core_skills::config_rules::resolve_disabled_skill_paths;
use codex_core_skills::config_rules::skill_config_rules_from_stack;
use codex_core_skills::loader::SkillRoot;
use codex_core_skills::loader::load_skills_from_roots;
use codex_exec_server::LOCAL_FS;
use codex_plugin::AppConnectorId;
use codex_plugin::LoadedPlugin;
use codex_plugin::PluginCapabilitySummary;
use codex_plugin::PluginId;
use codex_plugin::PluginIdError;
use codex_plugin::PluginLoadOutcome;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde_json::Map as JsonMap;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tempfile::TempDir;
use tracing::warn;

const DEFAULT_SKILLS_DIR_NAME: &str = "skills";
const DEFAULT_MCP_CONFIG_FILE: &str = ".mcp.json";
const DEFAULT_APP_CONFIG_FILE: &str = ".app.json";
const OPENAI_CURATED_MARKETPLACE_NAME: &str = "openai-curated";
const CONFIG_TOML_FILE: &str = "config.toml";

#[derive(Clone, Copy, PartialEq, Eq)]
enum NonCuratedCacheRefreshMode {
    IfVersionChanged,
    ForceReinstall,
}

pub fn log_plugin_load_errors(outcome: &PluginLoadOutcome<McpServerConfig>) {
    for plugin in outcome
        .plugins()
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

pub async fn load_plugins_from_layer_stack(
    config_layer_stack: &ConfigLayerStack,
    store: &PluginStore,
    restriction_product: Option<Product>,
) -> PluginLoadOutcome<McpServerConfig> {
    let skill_config_rules = skill_config_rules_from_stack(config_layer_stack);
    let mut configured_plugins: Vec<_> = configured_plugins_from_stack(config_layer_stack)
        .into_iter()
        .collect();
    configured_plugins.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));

    let mut plugins = Vec::with_capacity(configured_plugins.len());
    let mut seen_mcp_server_names = HashMap::<String, String>::new();
    for (configured_name, plugin) in configured_plugins {
        let loaded_plugin = load_plugin(
            configured_name.clone(),
            &plugin,
            store,
            restriction_product,
            &skill_config_rules,
        )
        .await;
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

pub fn refresh_curated_plugin_cache(
    codex_home: &Path,
    plugin_version: &str,
    configured_curated_plugin_ids: &[PluginId],
) -> Result<bool, String> {
    let store = PluginStore::new(codex_home.to_path_buf());
    let curated_marketplace_path = AbsolutePathBuf::try_from(
        codex_home
            .join(".tmp/plugins")
            .join(".agents/plugins/marketplace.json"),
    )
    .map_err(|_| "local curated marketplace is not available".to_string())?;
    let curated_marketplace = load_marketplace(&curated_marketplace_path)
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
            MarketplacePluginSource::Local { path } => path,
            MarketplacePluginSource::Git { .. } => {
                warn!(
                    plugin = plugin_name,
                    marketplace = OPENAI_CURATED_MARKETPLACE_NAME,
                    "skipping remote curated plugin source during cache refresh"
                );
                continue;
            }
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

pub fn refresh_non_curated_plugin_cache(
    codex_home: &Path,
    additional_roots: &[AbsolutePathBuf],
) -> Result<bool, String> {
    refresh_non_curated_plugin_cache_with_mode(
        codex_home,
        additional_roots,
        NonCuratedCacheRefreshMode::IfVersionChanged,
    )
}

pub fn refresh_non_curated_plugin_cache_force_reinstall(
    codex_home: &Path,
    additional_roots: &[AbsolutePathBuf],
) -> Result<bool, String> {
    refresh_non_curated_plugin_cache_with_mode(
        codex_home,
        additional_roots,
        NonCuratedCacheRefreshMode::ForceReinstall,
    )
}

fn refresh_non_curated_plugin_cache_with_mode(
    codex_home: &Path,
    additional_roots: &[AbsolutePathBuf],
    mode: NonCuratedCacheRefreshMode,
) -> Result<bool, String> {
    let configured_non_curated_plugin_ids =
        non_curated_plugin_ids_from_config_keys(configured_plugins_from_codex_home(
            codex_home,
            "failed to read user config while refreshing non-curated plugin cache",
            "failed to parse user config while refreshing non-curated plugin cache",
        ));
    if configured_non_curated_plugin_ids.is_empty() {
        return Ok(false);
    }
    let configured_non_curated_plugin_keys = configured_non_curated_plugin_ids
        .iter()
        .map(PluginId::as_key)
        .collect::<HashSet<_>>();

    let store = PluginStore::new(codex_home.to_path_buf());
    let marketplace_outcome = list_marketplaces(additional_roots)
        .map_err(|err| format!("failed to discover marketplaces for cache refresh: {err}"))?;
    let mut plugin_sources = HashMap::<String, MarketplacePluginSource>::new();

    for marketplace in marketplace_outcome.marketplaces {
        if marketplace.name == OPENAI_CURATED_MARKETPLACE_NAME {
            continue;
        }

        for plugin in marketplace.plugins {
            let plugin_id =
                PluginId::new(plugin.name.clone(), marketplace.name.clone()).map_err(|err| {
                    match err {
                        PluginIdError::Invalid(message) => {
                            format!("failed to prepare non-curated plugin cache refresh: {message}")
                        }
                    }
                })?;
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

            plugin_sources.insert(plugin_key, plugin.source);
        }
    }

    let mut cache_refreshed = false;
    for plugin_id in configured_non_curated_plugin_ids {
        let plugin_key = plugin_id.as_key();
        let Some(source) = plugin_sources.get(&plugin_key).cloned() else {
            warn!(
                plugin = plugin_id.plugin_name,
                marketplace = plugin_id.marketplace_name,
                "configured non-curated plugin no longer exists in discovered marketplaces during cache refresh"
            );
            continue;
        };
        let materialized =
            materialize_marketplace_plugin_source(codex_home, &source).map_err(|err| {
                format!("failed to materialize plugin source for {plugin_key}: {err}")
            })?;
        let source_path = materialized.path.clone();
        let plugin_version = plugin_version_for_source(source_path.as_path())
            .map_err(|err| format!("failed to read plugin version for {plugin_key}: {err}"))?;

        if mode == NonCuratedCacheRefreshMode::IfVersionChanged
            && store.active_plugin_version(&plugin_id).as_deref() == Some(plugin_version.as_str())
        {
            continue;
        }

        store
            .install_with_version(source_path, plugin_id.clone(), plugin_version)
            .map_err(|err| format!("failed to refresh plugin cache for {plugin_key}: {err}"))?;
        cache_refreshed = true;
    }

    Ok(cache_refreshed)
}

fn configured_plugins_from_stack(
    config_layer_stack: &ConfigLayerStack,
) -> HashMap<String, PluginConfig> {
    let Some(user_layer) = config_layer_stack.get_user_layer() else {
        return HashMap::new();
    };
    configured_plugins_from_user_config_value(&user_layer.config)
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
    .filter(|plugin_id| plugin_id.marketplace_name == OPENAI_CURATED_MARKETPLACE_NAME)
    .collect::<Vec<_>>();
    configured_curated_plugin_ids.sort_unstable_by_key(PluginId::as_key);
    configured_curated_plugin_ids
}

fn non_curated_plugin_ids_from_config_keys(
    configured_plugins: HashMap<String, PluginConfig>,
) -> Vec<PluginId> {
    let mut configured_non_curated_plugin_ids = configured_plugin_ids(
        configured_plugins,
        "ignoring invalid plugin key during non-curated cache refresh setup",
    )
    .into_iter()
    .filter(|plugin_id| plugin_id.marketplace_name != OPENAI_CURATED_MARKETPLACE_NAME)
    .collect::<Vec<_>>();
    configured_non_curated_plugin_ids.sort_unstable_by_key(PluginId::as_key);
    configured_non_curated_plugin_ids
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
    restriction_product: Option<Product>,
    skill_config_rules: &SkillConfigRules,
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
        manifest_description: None,
        root,
        enabled: plugin.enabled,
        skill_roots: Vec::new(),
        disabled_skill_paths: HashSet::new(),
        has_enabled_skills: false,
        mcp_servers: HashMap::new(),
        apps: Vec::new(),
        error: None,
    };

    if !plugin.enabled {
        return loaded_plugin;
    }

    let plugin_root = match plugin_id {
        Ok(_) => match active_plugin_root {
            Some(plugin_root) => plugin_root,
            None => {
                loaded_plugin.error = Some("plugin is not installed".to_string());
                return loaded_plugin;
            }
        },
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
    loaded_plugin.manifest_name = manifest
        .interface
        .as_ref()
        .and_then(|interface| interface.display_name.as_deref())
        .map(str::trim)
        .filter(|display_name| !display_name.is_empty())
        .map(str::to_string)
        .or_else(|| Some(manifest.name.clone()));
    loaded_plugin.manifest_description = manifest.description.clone();
    loaded_plugin.skill_roots = plugin_skill_roots(&plugin_root, manifest_paths);
    let resolved_skills = load_plugin_skills(
        &plugin_root,
        manifest_paths,
        restriction_product,
        skill_config_rules,
    )
    .await;
    let has_enabled_skills = resolved_skills.has_enabled_skills();
    loaded_plugin.disabled_skill_paths = resolved_skills.disabled_skill_paths;
    loaded_plugin.has_enabled_skills = has_enabled_skills;
    let mut mcp_servers = HashMap::new();
    for mcp_config_path in plugin_mcp_config_paths(plugin_root.as_path(), manifest_paths) {
        let plugin_mcp = load_mcp_servers_from_file(plugin_root.as_path(), &mcp_config_path).await;
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
    loaded_plugin.apps = load_plugin_apps(plugin_root.as_path()).await;
    loaded_plugin
}

#[derive(Debug, Clone)]
pub struct ResolvedPluginSkills {
    pub skills: Vec<SkillMetadata>,
    pub disabled_skill_paths: HashSet<AbsolutePathBuf>,
    pub had_errors: bool,
}

impl ResolvedPluginSkills {
    pub fn has_enabled_skills(&self) -> bool {
        self.had_errors
            || self
                .skills
                .iter()
                .any(|skill| !self.disabled_skill_paths.contains(&skill.path_to_skills_md))
    }
}

pub async fn load_plugin_skills(
    plugin_root: &AbsolutePathBuf,
    manifest_paths: &PluginManifestPaths,
    restriction_product: Option<Product>,
    skill_config_rules: &SkillConfigRules,
) -> ResolvedPluginSkills {
    let roots = plugin_skill_roots(plugin_root, manifest_paths)
        .into_iter()
        .map(|path| SkillRoot {
            path,
            scope: SkillScope::User,
            file_system: Arc::clone(&LOCAL_FS),
        })
        .collect::<Vec<_>>();
    let outcome = load_skills_from_roots(roots).await;
    let had_errors = !outcome.errors.is_empty();
    let skills = outcome
        .skills
        .into_iter()
        .filter(|skill| skill.matches_product_restriction_for_product(restriction_product))
        .collect::<Vec<_>>();
    let disabled_skill_paths = resolve_disabled_skill_paths(&skills, skill_config_rules);

    ResolvedPluginSkills {
        skills,
        disabled_skill_paths,
        had_errors,
    }
}

fn plugin_skill_roots(
    plugin_root: &AbsolutePathBuf,
    manifest_paths: &PluginManifestPaths,
) -> Vec<AbsolutePathBuf> {
    let mut paths = default_skill_roots(plugin_root);
    if let Some(path) = &manifest_paths.skills {
        paths.push(path.clone());
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

pub async fn load_plugin_apps(plugin_root: &Path) -> Vec<AppConnectorId> {
    if let Some(manifest) = load_plugin_manifest(plugin_root) {
        return load_apps_from_paths(
            plugin_root,
            plugin_app_config_paths(plugin_root, &manifest.paths),
        )
        .await;
    }
    load_apps_from_paths(plugin_root, default_app_config_paths(plugin_root)).await
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

async fn load_apps_from_paths(
    plugin_root: &Path,
    app_config_paths: Vec<AbsolutePathBuf>,
) -> Vec<AppConnectorId> {
    let mut connector_ids = Vec::new();
    for app_config_path in app_config_paths {
        let Ok(contents) = tokio::fs::read_to_string(app_config_path.as_path()).await else {
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

pub async fn plugin_telemetry_metadata_from_root(
    plugin_id: &PluginId,
    plugin_root: &AbsolutePathBuf,
) -> PluginTelemetryMetadata {
    let Some(manifest) = load_plugin_manifest(plugin_root.as_path()) else {
        return PluginTelemetryMetadata::from_plugin_id(plugin_id);
    };

    let manifest_paths = &manifest.paths;
    let has_skills = !plugin_skill_roots(plugin_root, manifest_paths).is_empty();
    let mut mcp_server_names = Vec::new();
    for path in plugin_mcp_config_paths(plugin_root.as_path(), manifest_paths) {
        mcp_server_names.extend(
            load_mcp_servers_from_file(plugin_root.as_path(), &path)
                .await
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
            app_connector_ids: load_apps_from_paths(
                plugin_root.as_path(),
                plugin_app_config_paths(plugin_root.as_path(), manifest_paths),
            )
            .await,
        }),
    }
}

pub async fn load_plugin_mcp_servers(plugin_root: &Path) -> HashMap<String, McpServerConfig> {
    let Some(manifest) = load_plugin_manifest(plugin_root) else {
        return HashMap::new();
    };

    let mut mcp_servers = HashMap::new();
    for mcp_config_path in plugin_mcp_config_paths(plugin_root, &manifest.paths) {
        let plugin_mcp = load_mcp_servers_from_file(plugin_root, &mcp_config_path).await;
        for (name, config) in plugin_mcp.mcp_servers {
            mcp_servers.entry(name).or_insert(config);
        }
    }

    mcp_servers
}

pub async fn installed_plugin_telemetry_metadata(
    codex_home: &Path,
    plugin_id: &PluginId,
) -> PluginTelemetryMetadata {
    let store = PluginStore::new(codex_home.to_path_buf());
    let Some(plugin_root) = store.active_plugin_root(plugin_id) else {
        return PluginTelemetryMetadata::from_plugin_id(plugin_id);
    };

    plugin_telemetry_metadata_from_root(plugin_id, &plugin_root).await
}

async fn load_mcp_servers_from_file(
    plugin_root: &Path,
    mcp_config_path: &AbsolutePathBuf,
) -> PluginMcpDiscovery {
    let Ok(contents) = tokio::fs::read_to_string(mcp_config_path.as_path()).await else {
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
    if let Some(target) = sha.or(ref_name) {
        run_git(&["checkout", target], Some(destination))?;
    } else if sparse_checkout_path.is_some() {
        run_git(&["checkout"], Some(destination))?;
    }
    Ok(())
}

fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<(), String> {
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
        return Ok(());
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
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn materialize_git_subdir_uses_sparse_checkout() {
        let codex_home = tempfile::tempdir().expect("create codex home");
        let repo = tempfile::tempdir().expect("create git repo");
        let plugin_dir = repo.path().join("plugins/toolkit");
        fs::create_dir_all(&plugin_dir).expect("create plugin directory");
        fs::create_dir_all(repo.path().join("plugins/other")).expect("create other plugin");
        fs::write(plugin_dir.join("marker.txt"), "toolkit").expect("write plugin marker");
        fs::write(repo.path().join("plugins/other/marker.txt"), "other")
            .expect("write other marker");
        fs::write(repo.path().join("root.txt"), "root").expect("write root marker");

        run_git(&["init"], Some(repo.path())).expect("init git repo");
        run_git(
            &["config", "user.email", "test@example.com"],
            Some(repo.path()),
        )
        .expect("configure git email");
        run_git(&["config", "user.name", "Test User"], Some(repo.path()))
            .expect("configure git name");
        run_git(&["add", "."], Some(repo.path())).expect("stage git repo");
        run_git(&["commit", "-m", "init"], Some(repo.path())).expect("commit git repo");

        let materialized = materialize_marketplace_plugin_source(
            codex_home.path(),
            &MarketplacePluginSource::Git {
                url: repo.path().display().to_string(),
                path: Some("plugins/toolkit".to_string()),
                ref_name: None,
                sha: None,
            },
        )
        .expect("materialize git source");

        assert_eq!(
            plugin_dir.file_name(),
            materialized.path.as_path().file_name()
        );
        assert!(materialized.path.as_path().join("marker.txt").is_file());
        let checkout_root = materialized
            .path
            .as_path()
            .parent()
            .and_then(Path::parent)
            .expect("materialized path should be nested under checkout root");
        assert!(!checkout_root.join("root.txt").exists());
        assert!(!checkout_root.join("plugins/other/marker.txt").exists());
    }
}
