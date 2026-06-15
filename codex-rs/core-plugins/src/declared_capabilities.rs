use crate::manifest::PluginManifestPaths;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tracing::warn;

const DEFAULT_SKILLS_DIR_NAME: &str = "skills";
const DEFAULT_MCP_CONFIG_FILE: &str = ".mcp.json";
const DEFAULT_APP_CONFIG_FILE: &str = ".app.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeclaredPluginCapabilities {
    pub(crate) skills: HashSet<DeclaredSkill>,
    pub(crate) apps: HashSet<DeclaredApp>,
    pub(crate) mcp: HashSet<DeclaredMcp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DeclaredSkill {
    pub(crate) path: AbsolutePathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DeclaredApp {
    pub(crate) name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DeclaredMcp {
    pub(crate) name: String,
}

pub(crate) fn load_declared_plugin_capabilities(
    plugin_root: &AbsolutePathBuf,
    manifest_paths: &PluginManifestPaths,
) -> DeclaredPluginCapabilities {
    DeclaredPluginCapabilities {
        skills: load_declared_plugin_skills(plugin_root, manifest_paths),
        apps: load_declared_plugin_apps(plugin_root.as_path(), manifest_paths),
        mcp: load_declared_plugin_mcp(plugin_root.as_path(), manifest_paths),
    }
}

fn load_declared_plugin_skills(
    plugin_root: &AbsolutePathBuf,
    manifest_paths: &PluginManifestPaths,
) -> HashSet<DeclaredSkill> {
    let mut skills = HashSet::new();
    if let Some(path) = &manifest_paths.skills {
        skills.insert(DeclaredSkill { path: path.clone() });
    } else {
        let path = plugin_root.join(DEFAULT_SKILLS_DIR_NAME);
        if path.as_path().is_dir() {
            skills.insert(DeclaredSkill { path });
        }
    }
    skills
}

fn load_declared_plugin_apps(
    plugin_root: &Path,
    manifest_paths: &PluginManifestPaths,
) -> HashSet<DeclaredApp> {
    let mut apps = HashSet::new();
    for app_config_path in plugin_app_config_paths(plugin_root, manifest_paths) {
        let Ok(contents) = fs::read_to_string(app_config_path.as_path()) else {
            continue;
        };
        let parsed = match serde_json::from_str::<DeclaredPluginAppFile>(&contents) {
            Ok(parsed) => parsed,
            Err(err) => {
                warn!(
                    path = %app_config_path.display(),
                    "failed to parse plugin app config while loading declared capabilities: {err}"
                );
                continue;
            }
        };

        apps.extend(parsed.apps.into_iter().filter_map(|(name, app)| {
            if app.id.trim().is_empty() {
                None
            } else {
                Some(DeclaredApp { name })
            }
        }));
    }

    apps
}

fn load_declared_plugin_mcp(
    plugin_root: &Path,
    manifest_paths: &PluginManifestPaths,
) -> HashSet<DeclaredMcp> {
    let mut mcp = HashSet::new();
    for mcp_config_path in plugin_mcp_config_paths(plugin_root, manifest_paths) {
        let Ok(contents) = fs::read_to_string(mcp_config_path.as_path()) else {
            continue;
        };
        let parsed = match serde_json::from_str::<DeclaredPluginMcpFile>(&contents) {
            Ok(parsed) => parsed,
            Err(err) => {
                warn!(
                    path = %mcp_config_path.display(),
                    "failed to parse plugin MCP config while loading declared capabilities: {err}"
                );
                continue;
            }
        };

        mcp.extend(
            parsed
                .into_mcp_servers()
                .into_keys()
                .map(|name| DeclaredMcp { name }),
        );
    }

    mcp
}

fn plugin_app_config_paths(
    plugin_root: &Path,
    manifest_paths: &PluginManifestPaths,
) -> Vec<AbsolutePathBuf> {
    if let Some(path) = &manifest_paths.apps {
        return vec![path.clone()];
    }
    default_plugin_config_paths(plugin_root, DEFAULT_APP_CONFIG_FILE)
}

fn plugin_mcp_config_paths(
    plugin_root: &Path,
    manifest_paths: &PluginManifestPaths,
) -> Vec<AbsolutePathBuf> {
    if let Some(path) = &manifest_paths.mcp_servers {
        return vec![path.clone()];
    }
    default_plugin_config_paths(plugin_root, DEFAULT_MCP_CONFIG_FILE)
}

fn default_plugin_config_paths(plugin_root: &Path, file_name: &str) -> Vec<AbsolutePathBuf> {
    let path = plugin_root.join(file_name);
    if path.is_file()
        && let Ok(path) = AbsolutePathBuf::try_from(path)
    {
        vec![path]
    } else {
        Vec::new()
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeclaredPluginAppFile {
    #[serde(default)]
    apps: HashMap<String, DeclaredPluginAppConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct DeclaredPluginAppConfig {
    id: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeclaredPluginMcpServersFile {
    mcp_servers: HashMap<String, JsonValue>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DeclaredPluginMcpFile {
    McpServersObject(DeclaredPluginMcpServersFile),
    ServerMap(HashMap<String, JsonValue>),
}

impl DeclaredPluginMcpFile {
    fn into_mcp_servers(self) -> HashMap<String, JsonValue> {
        match self {
            Self::McpServersObject(file) => file.mcp_servers,
            Self::ServerMap(mcp_servers) => mcp_servers,
        }
    }
}

#[cfg(test)]
#[path = "declared_capabilities_tests.rs"]
mod tests;
