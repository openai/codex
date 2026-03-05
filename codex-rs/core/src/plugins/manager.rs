use super::load_plugin_manifest;
use super::marketplace::MarketplaceError;
use super::marketplace::resolve_marketplace_plugin;
use super::plugin_manifest_name;
use super::store::DEFAULT_PLUGIN_VERSION;
use super::store::PluginId;
use super::store::PluginInstallResult;
use super::store::PluginStore;
use super::store::PluginStoreError;
use crate::config::Config;
use crate::config::ConfigService;
use crate::config::ConfigServiceError;
use crate::config::ConfigToml;
use crate::config::profile::ConfigProfile;
use crate::config::types::McpServerConfig;
use crate::config::types::PluginConfig;
use crate::config_loader::ConfigLayerStack;
use crate::features::Feature;
use crate::features::FeatureOverrides;
use crate::features::Features;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::MergeStrategy;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde_json::Map as JsonMap;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::RwLock;
use tracing::warn;

const DEFAULT_SKILLS_DIR_NAME: &str = "skills";
const DEFAULT_MCP_CONFIG_FILE: &str = ".mcp.json";
const DEFAULT_APP_CONFIG_FILE: &str = ".app.json";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppConnectorId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInstallRequest {
    pub plugin_name: String,
    pub marketplace_name: String,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedPlugin {
    pub config_name: String,
    pub manifest_name: Option<String>,
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
    pub has_skills: bool,
    pub mcp_server_names: Vec<String>,
    pub app_connector_ids: Vec<AppConnectorId>,
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
            has_skills: !plugin.skill_roots.is_empty(),
            mcp_server_names,
            app_connector_ids: plugin.apps.clone(),
        };

        (summary.has_skills
            || !summary.mcp_server_names.is_empty()
            || !summary.app_connector_ids.is_empty())
        .then_some(summary)
    }
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

pub struct PluginsManager {
    codex_home: PathBuf,
    store: PluginStore,
    cache_by_cwd: RwLock<HashMap<PathBuf, PluginLoadOutcome>>,
}

impl PluginsManager {
    pub fn new(codex_home: PathBuf) -> Self {
        Self {
            codex_home: codex_home.clone(),
            store: PluginStore::new(codex_home),
            cache_by_cwd: RwLock::new(HashMap::new()),
        }
    }

    pub fn plugins_for_config(&self, config: &Config) -> PluginLoadOutcome {
        self.plugins_for_layer_stack(&config.cwd, &config.config_layer_stack, false)
    }

    pub fn plugins_for_layer_stack(
        &self,
        cwd: &Path,
        config_layer_stack: &ConfigLayerStack,
        force_reload: bool,
    ) -> PluginLoadOutcome {
        if !plugins_feature_enabled_from_stack(config_layer_stack) {
            let mut cache = match self.cache_by_cwd.write() {
                Ok(cache) => cache,
                Err(err) => err.into_inner(),
            };
            cache.insert(cwd.to_path_buf(), PluginLoadOutcome::default());
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
    ) -> Result<PluginInstallResult, PluginInstallError> {
        let resolved = resolve_marketplace_plugin(
            &request.cwd,
            &request.plugin_name,
            &request.marketplace_name,
        )?;
        let store = self.store.clone();
        let result = tokio::task::spawn_blocking(move || {
            store.install(resolved.source_path.into_path_buf(), resolved.plugin_id)
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

        Ok(result)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PluginInstallError {
    #[error("{0}")]
    Marketplace(#[from] MarketplaceError),

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
                MarketplaceError::InvalidMarketplaceFile { .. }
                    | MarketplaceError::PluginNotFound { .. }
                    | MarketplaceError::DuplicatePlugin { .. }
                    | MarketplaceError::InvalidPlugin(_)
            ) | Self::Store(PluginStoreError::Invalid(_))
        )
    }
}

fn plugins_feature_enabled_from_stack(config_layer_stack: &ConfigLayerStack) -> bool {
    let effective_config = config_layer_stack.effective_config();
    let Ok(config_toml) = effective_config.try_into::<ConfigToml>() else {
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

fn configured_plugins_from_stack(
    config_layer_stack: &ConfigLayerStack,
) -> HashMap<String, PluginConfig> {
    let effective_config = config_layer_stack.effective_config();
    let Some(plugins_value) = effective_config.get("plugins") else {
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
    let plugin_version = DEFAULT_PLUGIN_VERSION.to_string();
    let plugin_root = PluginId::parse(&config_name)
        .map(|plugin_id| store.plugin_root(&plugin_id, &plugin_version));
    let root = match &plugin_root {
        Ok(plugin_root) => plugin_root.clone(),
        Err(_) => store.root().clone(),
    };
    let mut loaded_plugin = LoadedPlugin {
        config_name,
        manifest_name: None,
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

    loaded_plugin.manifest_name = Some(plugin_manifest_name(&manifest, plugin_root.as_path()));
    loaded_plugin.skill_roots = default_skill_roots(plugin_root.as_path());
    let mut mcp_servers = HashMap::new();
    for mcp_config_path in default_mcp_config_paths(plugin_root.as_path()) {
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
    loaded_plugin.apps = load_apps_from_file(
        plugin_root.as_path(),
        &plugin_root.as_path().join(DEFAULT_APP_CONFIG_FILE),
    );
    loaded_plugin
}

fn default_skill_roots(plugin_root: &Path) -> Vec<PathBuf> {
    let skills_dir = plugin_root.join(DEFAULT_SKILLS_DIR_NAME);
    if skills_dir.is_dir() {
        vec![skills_dir]
    } else {
        Vec::new()
    }
}

fn default_mcp_config_paths(plugin_root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let default_path = plugin_root.join(DEFAULT_MCP_CONFIG_FILE);
    if default_path.is_file() {
        paths.push(default_path);
    }
    paths.sort_unstable();
    paths.dedup();
    paths
}

fn load_apps_from_file(plugin_root: &Path, app_config_path: &Path) -> Vec<AppConnectorId> {
    let Ok(contents) = fs::read_to_string(app_config_path) else {
        return Vec::new();
    };
    let parsed = match serde_json::from_str::<PluginAppFile>(&contents) {
        Ok(parsed) => parsed,
        Err(err) => {
            warn!(
                path = %app_config_path.display(),
                "failed to parse plugin app config: {err}"
            );
            return Vec::new();
        }
    };

    let mut apps: Vec<PluginAppConfig> = parsed.apps.into_values().collect();
    apps.sort_unstable_by(|left, right| left.id.cmp(&right.id));

    let mut connector_ids: Vec<AppConnectorId> = apps
        .into_iter()
        .filter_map(|app| {
            if app.id.trim().is_empty() {
                warn!(
                    plugin = %plugin_root.display(),
                    "plugin app config is missing an app id"
                );
                None
            } else {
                Some(AppConnectorId(app.id))
            }
        })
        .collect();
    connector_ids.dedup();
    connector_ids
}

fn load_mcp_servers_from_file(plugin_root: &Path, mcp_config_path: &Path) -> PluginMcpDiscovery {
    let Ok(contents) = fs::read_to_string(mcp_config_path) else {
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
mod tests {
    use super::*;
    use crate::config::CONFIG_TOML_FILE;
    use crate::config::ConfigBuilder;
    use crate::config::types::McpServerTransportConfig;
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::TempDir;
    use toml::Value;

    fn write_file(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().expect("file should have a parent")).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn write_plugin(root: &Path, dir_name: &str, manifest_name: &str) {
        let plugin_root = root.join(dir_name);
        fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
        fs::create_dir_all(plugin_root.join("skills")).unwrap();
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            format!(r#"{{"name":"{manifest_name}"}}"#),
        )
        .unwrap();
        fs::write(plugin_root.join("skills/SKILL.md"), "skill").unwrap();
        fs::write(plugin_root.join(".mcp.json"), r#"{"mcpServers":{}}"#).unwrap();
    }

    fn plugin_config_toml(enabled: bool, plugins_feature_enabled: bool) -> String {
        let mut root = toml::map::Map::new();

        let mut features = toml::map::Map::new();
        features.insert(
            "plugins".to_string(),
            Value::Boolean(plugins_feature_enabled),
        );
        root.insert("features".to_string(), Value::Table(features));

        let mut plugin = toml::map::Map::new();
        plugin.insert("enabled".to_string(), Value::Boolean(enabled));

        let mut plugins = toml::map::Map::new();
        plugins.insert("sample@test".to_string(), Value::Table(plugin));
        root.insert("plugins".to_string(), Value::Table(plugins));

        toml::to_string(&Value::Table(root)).expect("plugin test config should serialize")
    }

    async fn load_plugins_from_config(config_toml: &str, codex_home: &Path) -> PluginLoadOutcome {
        write_file(&codex_home.join(CONFIG_TOML_FILE), config_toml);
        let config = ConfigBuilder::default()
            .codex_home(codex_home.to_path_buf())
            .build()
            .await
            .expect("config should load");
        PluginsManager::new(codex_home.to_path_buf()).plugins_for_config(&config)
    }

    #[tokio::test]
    async fn load_plugins_loads_default_skills_and_mcp_servers() {
        let codex_home = TempDir::new().unwrap();
        let plugin_root = codex_home
            .path()
            .join("plugins/cache")
            .join("test/sample/local");

        write_file(
            &plugin_root.join(".codex-plugin/plugin.json"),
            r#"{"name":"sample"}"#,
        );
        write_file(
            &plugin_root.join("skills/sample-search/SKILL.md"),
            "---\nname: sample-search\ndescription: search sample data\n---\n",
        );
        write_file(
            &plugin_root.join(".mcp.json"),
            r#"{
  "mcpServers": {
    "sample": {
      "type": "http",
      "url": "https://sample.example/mcp",
      "oauth": {
        "clientId": "client-id",
        "callbackPort": 3118
      }
    }
  }
}"#,
        );
        write_file(
            &plugin_root.join(".app.json"),
            r#"{
  "apps": {
    "example": {
      "id": "connector_example"
    }
  }
}"#,
        );

        let outcome =
            load_plugins_from_config(&plugin_config_toml(true, true), codex_home.path()).await;

        assert_eq!(
            outcome.plugins,
            vec![LoadedPlugin {
                config_name: "sample@test".to_string(),
                manifest_name: Some("sample".to_string()),
                root: AbsolutePathBuf::try_from(plugin_root.clone()).unwrap(),
                enabled: true,
                skill_roots: vec![plugin_root.join("skills")],
                mcp_servers: HashMap::from([(
                    "sample".to_string(),
                    McpServerConfig {
                        transport: McpServerTransportConfig::StreamableHttp {
                            url: "https://sample.example/mcp".to_string(),
                            bearer_token_env_var: None,
                            http_headers: None,
                            env_http_headers: None,
                        },
                        enabled: true,
                        required: false,
                        disabled_reason: None,
                        startup_timeout_sec: None,
                        tool_timeout_sec: None,
                        enabled_tools: None,
                        disabled_tools: None,
                        scopes: None,
                        oauth_resource: None,
                    },
                )]),
                apps: vec![AppConnectorId("connector_example".to_string())],
                error: None,
            }]
        );
        assert_eq!(
            outcome.effective_skill_roots(),
            vec![plugin_root.join("skills")]
        );
        assert_eq!(outcome.effective_mcp_servers().len(), 1);
        assert_eq!(
            outcome.effective_apps(),
            vec![AppConnectorId("connector_example".to_string())]
        );
    }

    #[tokio::test]
    async fn load_plugins_preserves_disabled_plugins_without_effective_contributions() {
        let codex_home = TempDir::new().unwrap();
        let plugin_root = codex_home
            .path()
            .join("plugins/cache")
            .join("test/sample/local");

        write_file(
            &plugin_root.join(".codex-plugin/plugin.json"),
            r#"{"name":"sample"}"#,
        );
        write_file(
            &plugin_root.join(".mcp.json"),
            r#"{
  "mcpServers": {
    "sample": {
      "type": "http",
      "url": "https://sample.example/mcp"
    }
  }
}"#,
        );

        let outcome =
            load_plugins_from_config(&plugin_config_toml(false, true), codex_home.path()).await;

        assert_eq!(
            outcome.plugins,
            vec![LoadedPlugin {
                config_name: "sample@test".to_string(),
                manifest_name: None,
                root: AbsolutePathBuf::try_from(plugin_root).unwrap(),
                enabled: false,
                skill_roots: Vec::new(),
                mcp_servers: HashMap::new(),
                apps: Vec::new(),
                error: None,
            }]
        );
        assert!(outcome.effective_skill_roots().is_empty());
        assert!(outcome.effective_mcp_servers().is_empty());
    }

    #[tokio::test]
    async fn effective_apps_dedupes_connector_ids_across_plugins() {
        let codex_home = TempDir::new().unwrap();
        let plugin_a_root = codex_home
            .path()
            .join("plugins/cache")
            .join("test/plugin-a/local");
        let plugin_b_root = codex_home
            .path()
            .join("plugins/cache")
            .join("test/plugin-b/local");

        write_file(
            &plugin_a_root.join(".codex-plugin/plugin.json"),
            r#"{"name":"plugin-a"}"#,
        );
        write_file(
            &plugin_a_root.join(".app.json"),
            r#"{
  "apps": {
    "example": {
      "id": "connector_example"
    }
  }
}"#,
        );
        write_file(
            &plugin_b_root.join(".codex-plugin/plugin.json"),
            r#"{"name":"plugin-b"}"#,
        );
        write_file(
            &plugin_b_root.join(".app.json"),
            r#"{
  "apps": {
    "chat": {
      "id": "connector_example"
    },
    "gmail": {
      "id": "connector_gmail"
    }
  }
}"#,
        );

        let mut root = toml::map::Map::new();
        let mut features = toml::map::Map::new();
        features.insert("plugins".to_string(), Value::Boolean(true));
        root.insert("features".to_string(), Value::Table(features));

        let mut plugins = toml::map::Map::new();

        let mut plugin_a = toml::map::Map::new();
        plugin_a.insert("enabled".to_string(), Value::Boolean(true));
        plugins.insert("plugin-a@test".to_string(), Value::Table(plugin_a));

        let mut plugin_b = toml::map::Map::new();
        plugin_b.insert("enabled".to_string(), Value::Boolean(true));
        plugins.insert("plugin-b@test".to_string(), Value::Table(plugin_b));

        root.insert("plugins".to_string(), Value::Table(plugins));
        let config_toml =
            toml::to_string(&Value::Table(root)).expect("plugin test config should serialize");

        let outcome = load_plugins_from_config(&config_toml, codex_home.path()).await;

        assert_eq!(
            outcome.effective_apps(),
            vec![
                AppConnectorId("connector_example".to_string()),
                AppConnectorId("connector_gmail".to_string()),
            ]
        );
    }

    #[test]
    fn capability_index_filters_inactive_and_zero_capability_plugins() {
        let codex_home = TempDir::new().unwrap();
        let connector = |id: &str| AppConnectorId(id.to_string());
        let http_server = |url: &str| McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: url.to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
        };
        let plugin = |config_name: &str, dir_name: &str, manifest_name: &str| LoadedPlugin {
            config_name: config_name.to_string(),
            manifest_name: Some(manifest_name.to_string()),
            root: AbsolutePathBuf::try_from(codex_home.path().join(dir_name)).unwrap(),
            enabled: true,
            skill_roots: Vec::new(),
            mcp_servers: HashMap::new(),
            apps: Vec::new(),
            error: None,
        };
        let summary = |config_name: &str, display_name: &str| PluginCapabilitySummary {
            config_name: config_name.to_string(),
            display_name: display_name.to_string(),
            ..PluginCapabilitySummary::default()
        };
        let outcome = PluginLoadOutcome::from_plugins(vec![
            LoadedPlugin {
                skill_roots: vec![codex_home.path().join("skills-plugin/skills")],
                ..plugin("skills@test", "skills-plugin", "skills-plugin")
            },
            LoadedPlugin {
                mcp_servers: HashMap::from([("alpha".to_string(), http_server("https://alpha"))]),
                apps: vec![connector("connector_example")],
                ..plugin("alpha@test", "alpha-plugin", "alpha-plugin")
            },
            LoadedPlugin {
                mcp_servers: HashMap::from([("beta".to_string(), http_server("https://beta"))]),
                apps: vec![connector("connector_example"), connector("connector_gmail")],
                ..plugin("beta@test", "beta-plugin", "beta-plugin")
            },
            plugin("empty@test", "empty-plugin", "empty-plugin"),
            LoadedPlugin {
                enabled: false,
                skill_roots: vec![codex_home.path().join("disabled-plugin/skills")],
                apps: vec![connector("connector_hidden")],
                ..plugin("disabled@test", "disabled-plugin", "disabled-plugin")
            },
            LoadedPlugin {
                apps: vec![connector("connector_broken")],
                error: Some("failed to load".to_string()),
                ..plugin("broken@test", "broken-plugin", "broken-plugin")
            },
        ]);

        assert_eq!(
            outcome.capability_summaries(),
            &[
                PluginCapabilitySummary {
                    has_skills: true,
                    ..summary("skills@test", "skills-plugin")
                },
                PluginCapabilitySummary {
                    mcp_server_names: vec!["alpha".to_string()],
                    app_connector_ids: vec![connector("connector_example")],
                    ..summary("alpha@test", "alpha-plugin")
                },
                PluginCapabilitySummary {
                    mcp_server_names: vec!["beta".to_string()],
                    app_connector_ids: vec![
                        connector("connector_example"),
                        connector("connector_gmail"),
                    ],
                    ..summary("beta@test", "beta-plugin")
                },
            ]
        );
    }

    #[test]
    fn plugin_namespace_for_skill_path_uses_manifest_name() {
        let codex_home = TempDir::new().unwrap();
        let plugin_root = codex_home.path().join("plugins/sample");
        let skill_path = plugin_root.join("skills/search/SKILL.md");

        write_file(
            &plugin_root.join(".codex-plugin/plugin.json"),
            r#"{"name":"sample"}"#,
        );
        write_file(&skill_path, "---\ndescription: search\n---\n");

        assert_eq!(
            plugin_namespace_for_skill_path(&skill_path),
            Some("sample".to_string())
        );
    }

    #[tokio::test]
    async fn load_plugins_returns_empty_when_feature_disabled() {
        let codex_home = TempDir::new().unwrap();
        let plugin_root = codex_home
            .path()
            .join("plugins/cache")
            .join("test/sample/local");

        write_file(
            &plugin_root.join(".codex-plugin/plugin.json"),
            r#"{"name":"sample"}"#,
        );
        write_file(
            &plugin_root.join("skills/sample-search/SKILL.md"),
            "---\nname: sample-search\ndescription: search sample data\n---\n",
        );

        let outcome =
            load_plugins_from_config(&plugin_config_toml(true, false), codex_home.path()).await;

        assert_eq!(outcome, PluginLoadOutcome::default());
    }

    #[tokio::test]
    async fn load_plugins_rejects_invalid_plugin_keys() {
        let codex_home = TempDir::new().unwrap();
        let plugin_root = codex_home
            .path()
            .join("plugins/cache")
            .join("test/sample/local");

        write_file(
            &plugin_root.join(".codex-plugin/plugin.json"),
            r#"{"name":"sample"}"#,
        );

        let mut root = toml::map::Map::new();
        let mut features = toml::map::Map::new();
        features.insert("plugins".to_string(), Value::Boolean(true));
        root.insert("features".to_string(), Value::Table(features));

        let mut plugin = toml::map::Map::new();
        plugin.insert("enabled".to_string(), Value::Boolean(true));

        let mut plugins = toml::map::Map::new();
        plugins.insert("sample".to_string(), Value::Table(plugin));
        root.insert("plugins".to_string(), Value::Table(plugins));

        let outcome = load_plugins_from_config(
            &toml::to_string(&Value::Table(root)).expect("plugin test config should serialize"),
            codex_home.path(),
        )
        .await;

        assert_eq!(outcome.plugins.len(), 1);
        assert_eq!(
            outcome.plugins[0].error.as_deref(),
            Some("invalid plugin key `sample`; expected <plugin>@<marketplace>")
        );
        assert!(outcome.effective_skill_roots().is_empty());
        assert!(outcome.effective_mcp_servers().is_empty());
    }

    #[tokio::test]
    async fn install_plugin_updates_config_with_relative_path_and_plugin_key() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_root = tmp.path().join("repo");
        fs::create_dir_all(repo_root.join(".git")).unwrap();
        fs::create_dir_all(repo_root.join(".agents/plugins")).unwrap();
        write_plugin(
            &repo_root.join(".agents/plugins"),
            "sample-plugin",
            "sample-plugin",
        );
        fs::write(
            repo_root.join(".agents/plugins/marketplace.json"),
            r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample-plugin",
      "source": {
        "source": "local",
        "path": "./sample-plugin"
      }
    }
  ]
}"#,
        )
        .unwrap();

        let result = PluginsManager::new(tmp.path().to_path_buf())
            .install_plugin(PluginInstallRequest {
                plugin_name: "sample-plugin".to_string(),
                marketplace_name: "debug".to_string(),
                cwd: repo_root.clone(),
            })
            .await
            .unwrap();

        let installed_path = tmp.path().join("plugins/cache/debug/sample-plugin/local");
        assert_eq!(
            result,
            PluginInstallResult {
                plugin_id: PluginId::new("sample-plugin".to_string(), "debug".to_string()).unwrap(),
                plugin_version: "local".to_string(),
                installed_path,
            }
        );

        let config = fs::read_to_string(tmp.path().join("config.toml")).unwrap();
        assert!(config.contains(r#"[plugins."sample-plugin@debug"]"#));
        assert!(config.contains("enabled = true"));
    }
}
