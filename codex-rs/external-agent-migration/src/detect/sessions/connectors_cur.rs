use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io::BufRead;
use std::path::Component;
use std::path::Path;

use crate::model::DetectedConnectorCandidate;
use crate::model::DetectedConnectorSource;
use crate::sessions::ExternalAgentSessionMigration;

const PLUGIN_CACHE_DIR: &str = "plugins/cache";
const PLUGIN_MANIFEST_PATH: &str = ".cursor-plugin/plugin.json";
const MCP_CONFIG_PATH: &str = ".mcp.json";
const MCP_TOOL_CALL: &str = "CallMcpTool";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginManifest {
    name: String,
    display_name: Option<String>,
    mcp_servers: Option<JsonValue>,
}

pub(crate) fn detect_cur_session_connectors(
    sessions: &[ExternalAgentSessionMigration],
    external_agent_home: &Path,
) -> Vec<DetectedConnectorCandidate> {
    let connector_names_by_server_id = cached_connector_names_by_server_id(external_agent_home);
    if connector_names_by_server_id.is_empty() {
        return Vec::new();
    }

    let mut candidates = BTreeMap::<String, DetectedConnectorCandidate>::new();
    for session in sessions {
        for (key, name) in session_connector_names(session, &connector_names_by_server_id) {
            let candidate = candidates.entry(key).or_insert(DetectedConnectorCandidate {
                name,
                session_count: 0,
                source: DetectedConnectorSource::SessionToolUse,
            });
            candidate.session_count = candidate.session_count.saturating_add(1);
        }
    }
    candidates.into_values().collect()
}

fn session_connector_names(
    session: &ExternalAgentSessionMigration,
    connector_names_by_server_id: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let Ok(file) = fs::File::open(&session.path) else {
        return BTreeMap::new();
    };
    std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<JsonValue>(line.trim()).ok())
        .flat_map(|record| session_mcp_server_ids(&record).into_iter())
        .filter_map(|server_id| connector_names_by_server_id.get(&server_id).cloned())
        .map(|name| (name.to_lowercase(), name))
        .collect()
}

fn session_mcp_server_ids(record: &JsonValue) -> BTreeSet<String> {
    record
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter(|content| {
            content.get("type").and_then(JsonValue::as_str) == Some("tool_use")
                && content.get("name").and_then(JsonValue::as_str) == Some(MCP_TOOL_CALL)
        })
        .filter_map(|content| {
            content
                .get("input")
                .and_then(|input| input.get("server"))
                .and_then(JsonValue::as_str)
                .map(str::trim)
                .filter(|server_id| !server_id.is_empty())
                .map(str::to_lowercase)
        })
        .collect()
}

fn cached_connector_names_by_server_id(external_agent_home: &Path) -> BTreeMap<String, String> {
    let cache_root = external_agent_home.join(PLUGIN_CACHE_DIR);
    let mut connector_names = BTreeMap::new();
    for marketplace_root in child_directories(&cache_root) {
        for plugin_root in child_directories(&marketplace_root) {
            for version_root in child_directories(&plugin_root) {
                let Some(manifest) = read_plugin_manifest(&version_root) else {
                    continue;
                };
                let Some(display_name) = crate::sessions::normalized_connector_display_name(
                    manifest.display_name.as_deref().or(Some(&manifest.name)),
                ) else {
                    continue;
                };
                for server_name in cached_mcp_server_names(&version_root, &manifest) {
                    connector_names.insert(
                        format!("plugin-{}-{server_name}", manifest.name).to_lowercase(),
                        display_name.clone(),
                    );
                }
            }
        }
    }
    connector_names
}

fn child_directories(root: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(std::fs::FileType::is_dir)
                .map(|_| entry.path())
        })
        .collect()
}

fn read_plugin_manifest(version_root: &Path) -> Option<PluginManifest> {
    let contents = fs::read_to_string(version_root.join(PLUGIN_MANIFEST_PATH)).ok()?;
    serde_json::from_str(&contents).ok()
}

fn cached_mcp_server_names(version_root: &Path, manifest: &PluginManifest) -> Vec<String> {
    manifest
        .mcp_servers
        .as_ref()
        .map_or_else(
            || mcp_server_names_from_file(version_root, MCP_CONFIG_PATH),
            |declaration| manifest_mcp_server_names(version_root, declaration),
        )
        .into_iter()
        .collect()
}

fn manifest_mcp_server_names(version_root: &Path, declaration: &JsonValue) -> BTreeSet<String> {
    match declaration {
        JsonValue::String(path) => mcp_server_names_from_file(version_root, path),
        JsonValue::Object(_) => mcp_server_names_from_config(declaration),
        JsonValue::Array(declarations) => declarations
            .iter()
            .flat_map(|declaration| manifest_mcp_server_names(version_root, declaration))
            .collect(),
        _ => BTreeSet::new(),
    }
}

fn mcp_server_names_from_file(version_root: &Path, relative_path: &str) -> BTreeSet<String> {
    let relative_path = Path::new(relative_path);
    if relative_path.as_os_str().is_empty()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::CurDir | Component::Normal(_)))
    {
        return BTreeSet::new();
    }
    let Ok(contents) = fs::read_to_string(version_root.join(relative_path)) else {
        return BTreeSet::new();
    };
    let Ok(config) = serde_json::from_str::<JsonValue>(&contents) else {
        return BTreeSet::new();
    };
    mcp_server_names_from_config(&config)
}

fn mcp_server_names_from_config(config: &JsonValue) -> BTreeSet<String> {
    config
        .get("mcpServers")
        .and_then(JsonValue::as_object)
        .or_else(|| config.as_object())
        .into_iter()
        .flat_map(|servers| servers.keys())
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "connectors_cur_tests.rs"]
mod tests;
