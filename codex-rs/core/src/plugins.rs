use crate::config::Config;
use crate::config::ConfigToml;
use crate::config::profile::ConfigProfile;
use crate::config::types::McpServerConfig;
use crate::config::types::PluginConfig;
use crate::config_loader::ConfigLayerStack;
use crate::features::Feature;
use crate::features::FeatureOverrides;
use crate::features::Features;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde_json::Map as JsonMap;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::RwLock;
use tracing::warn;

const PLUGIN_MANIFEST_PATH: &str = ".codex-plugin/plugin.json";
const DEFAULT_SKILLS_DIR_NAME: &str = "skills";
const DEFAULT_MCP_CONFIG_FILE: &str = ".mcp.json";

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedPlugin {
    pub config_name: String,
    pub manifest_name: Option<String>,
    pub root: AbsolutePathBuf,
    pub enabled: bool,
    pub skill_roots: Vec<PathBuf>,
    pub mcp_servers: HashMap<String, McpServerConfig>,
    pub error: Option<String>,
}

impl LoadedPlugin {
    fn is_active(&self) -> bool {
        self.enabled && self.error.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PluginLoadOutcome {
    pub plugins: Vec<LoadedPlugin>,
}

impl PluginLoadOutcome {
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
}

pub struct PluginsManager {
    cache_by_cwd: RwLock<HashMap<PathBuf, PluginLoadOutcome>>,
}

impl PluginsManager {
    pub fn new(_codex_home: PathBuf) -> Self {
        Self {
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

        let outcome = load_plugins_from_layer_stack(config_layer_stack);
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
struct PluginManifest {
    name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginMcpFile {
    #[serde(default)]
    mcp_servers: HashMap<String, JsonValue>,
}

pub fn load_plugins_from_layer_stack(config_layer_stack: &ConfigLayerStack) -> PluginLoadOutcome {
    let mut configured_plugins: Vec<_> = configured_plugins_from_stack(config_layer_stack)
        .into_iter()
        .collect();
    configured_plugins.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));

    let mut plugins = Vec::with_capacity(configured_plugins.len());
    let mut seen_mcp_server_names = HashMap::<String, String>::new();
    for (configured_name, plugin) in configured_plugins {
        let loaded_plugin = load_plugin(configured_name.clone(), &plugin);
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

    PluginLoadOutcome { plugins }
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

fn load_plugin(config_name: String, plugin: &PluginConfig) -> LoadedPlugin {
    let plugin_root = plugin.path.clone();
    let mut loaded_plugin = LoadedPlugin {
        config_name,
        manifest_name: None,
        root: plugin_root.clone(),
        enabled: plugin.enabled,
        skill_roots: Vec::new(),
        mcp_servers: HashMap::new(),
        error: None,
    };

    if !plugin.enabled {
        return loaded_plugin;
    }

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
    loaded_plugin
}

fn load_plugin_manifest(plugin_root: &Path) -> Option<PluginManifest> {
    let manifest_path = plugin_root.join(PLUGIN_MANIFEST_PATH);
    if !manifest_path.is_file() {
        return None;
    }
    let contents = fs::read_to_string(&manifest_path).ok()?;
    match serde_json::from_str(&contents) {
        Ok(manifest) => Some(manifest),
        Err(err) => {
            warn!(
                path = %manifest_path.display(),
                "failed to parse plugin manifest: {err}"
            );
            None
        }
    }
}

fn plugin_manifest_name(manifest: &PluginManifest, plugin_root: &Path) -> String {
    plugin_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|_| manifest.name.trim().is_empty())
        .unwrap_or(&manifest.name)
        .to_string()
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
    use tempfile::TempDir;
    use toml::Value;

    fn write_file(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().expect("file should have a parent")).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn plugin_config_toml(
        plugin_root: &Path,
        enabled: bool,
        plugins_feature_enabled: bool,
    ) -> String {
        let mut root = toml::map::Map::new();

        let mut features = toml::map::Map::new();
        features.insert(
            "plugins".to_string(),
            Value::Boolean(plugins_feature_enabled),
        );
        root.insert("features".to_string(), Value::Table(features));

        let mut plugin = toml::map::Map::new();
        plugin.insert(
            "path".to_string(),
            Value::String(plugin_root.display().to_string()),
        );
        plugin.insert("enabled".to_string(), Value::Boolean(enabled));

        let mut plugins = toml::map::Map::new();
        plugins.insert("sample".to_string(), Value::Table(plugin));
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
        let plugin_root = codex_home.path().join("plugin-sample");

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

        let outcome = load_plugins_from_config(
            &plugin_config_toml(&plugin_root, true, true),
            codex_home.path(),
        )
        .await;

        assert_eq!(
            outcome.plugins,
            vec![LoadedPlugin {
                config_name: "sample".to_string(),
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
                error: None,
            }]
        );
        assert_eq!(
            outcome.effective_skill_roots(),
            vec![plugin_root.join("skills")]
        );
        assert_eq!(outcome.effective_mcp_servers().len(), 1);
    }

    #[tokio::test]
    async fn load_plugins_preserves_disabled_plugins_without_effective_contributions() {
        let codex_home = TempDir::new().unwrap();
        let plugin_root = codex_home.path().join("plugin-sample");

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

        let outcome = load_plugins_from_config(
            &plugin_config_toml(&plugin_root, false, true),
            codex_home.path(),
        )
        .await;

        assert_eq!(
            outcome.plugins,
            vec![LoadedPlugin {
                config_name: "sample".to_string(),
                manifest_name: None,
                root: AbsolutePathBuf::try_from(plugin_root).unwrap(),
                enabled: false,
                skill_roots: Vec::new(),
                mcp_servers: HashMap::new(),
                error: None,
            }]
        );
        assert!(outcome.effective_skill_roots().is_empty());
        assert!(outcome.effective_mcp_servers().is_empty());
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
        let plugin_root = codex_home.path().join("plugin-sample");

        write_file(
            &plugin_root.join(".codex-plugin/plugin.json"),
            r#"{"name":"sample"}"#,
        );
        write_file(
            &plugin_root.join("skills/sample-search/SKILL.md"),
            "---\nname: sample-search\ndescription: search sample data\n---\n",
        );

        let outcome = load_plugins_from_config(
            &plugin_config_toml(&plugin_root, true, false),
            codex_home.path(),
        )
        .await;

        assert_eq!(outcome, PluginLoadOutcome::default());
    }
}
