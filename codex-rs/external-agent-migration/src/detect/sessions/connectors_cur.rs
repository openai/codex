use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io::BufRead;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use crate::model::DetectedConnectorCandidate;
use crate::model::DetectedConnectorSource;
use crate::sessions::ExternalAgentSessionMigration;

const PLUGIN_CACHE_DIR: &str = "plugins/cache";
const PLUGIN_MANIFEST_PATH: &str = ".cursor-plugin/plugin.json";
const MCP_CONFIG_PATH: &str = ".mcp.json";
const PROJECT_MCP_DIR: &str = "mcps";
const PROJECT_MCP_SERVER_METADATA_PATH: &str = "SERVER_METADATA.json";
const AGENT_TRANSCRIPTS_DIR: &str = "agent-transcripts";
const MCP_TOOL_CALL: &str = "CallMcpTool";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginManifest {
    name: String,
    display_name: Option<String>,
    mcp_servers: Option<JsonValue>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectMcpServerMetadata {
    server_identifier: String,
    server_name: String,
}

#[derive(Default)]
struct ProjectConnectorMetadata {
    names_by_server_id: BTreeMap<String, String>,
    names_by_normalized_name: BTreeMap<String, String>,
}

pub(crate) fn detect_cur_session_connectors(
    sessions: &[ExternalAgentSessionMigration],
    external_agent_home: &Path,
) -> Vec<DetectedConnectorCandidate> {
    super::aggregate_session_connector_candidates(detect_cur_session_connectors_by_source_path(
        sessions,
        external_agent_home,
    ))
}

pub(crate) fn detect_cur_session_connectors_by_source_path(
    sessions: &[ExternalAgentSessionMigration],
    external_agent_home: &Path,
) -> BTreeMap<PathBuf, Vec<DetectedConnectorCandidate>> {
    let cached_connector_names_by_server_id =
        cached_connector_names_by_server_id(external_agent_home);
    let mut candidates_by_source_path = BTreeMap::new();
    for session in sessions {
        let Ok(source_path) = fs::canonicalize(&session.path) else {
            continue;
        };
        let project_connector_metadata = project_connector_metadata(session);
        let candidates = session_connector_names(
            session,
            &cached_connector_names_by_server_id,
            &project_connector_metadata,
        )
        .into_values()
        .map(|name| DetectedConnectorCandidate {
            name,
            session_count: 1,
            source: DetectedConnectorSource::SessionToolUse,
        })
        .collect::<Vec<_>>();
        if !candidates.is_empty() {
            candidates_by_source_path.insert(source_path, candidates);
        }
    }
    candidates_by_source_path
}

fn project_connector_metadata(session: &ExternalAgentSessionMigration) -> ProjectConnectorMetadata {
    let Some(project_root) = session
        .path
        .ancestors()
        .find(|path| path.file_name().and_then(|name| name.to_str()) == Some(AGENT_TRANSCRIPTS_DIR))
        .and_then(Path::parent)
    else {
        return ProjectConnectorMetadata::default();
    };
    let mut connector_metadata = ProjectConnectorMetadata::default();
    for server_root in child_directories(&project_root.join(PROJECT_MCP_DIR)) {
        let Some(metadata) = read_project_mcp_server_metadata(&server_root) else {
            continue;
        };
        let Some(display_name) =
            crate::sessions::normalized_connector_display_name(Some(&metadata.server_name))
        else {
            continue;
        };
        let server_identifier = metadata.server_identifier.trim();
        if server_identifier.is_empty() {
            continue;
        }
        connector_metadata
            .names_by_server_id
            .insert(server_identifier.to_lowercase(), display_name.clone());
        // Cursor has emitted both the configured identifier and display name in
        // mcpDetails.serverName. Match either only after finding it in project
        // metadata; never infer a connector from the tool namespace alone.
        connector_metadata
            .names_by_normalized_name
            .insert(server_identifier.to_lowercase(), display_name.clone());
        connector_metadata
            .names_by_normalized_name
            .insert(display_name.to_lowercase(), display_name);
    }
    connector_metadata
}

fn session_connector_names(
    session: &ExternalAgentSessionMigration,
    cached_connector_names_by_server_id: &BTreeMap<String, String>,
    project_connector_metadata: &ProjectConnectorMetadata,
) -> BTreeMap<String, String> {
    let Ok(file) = fs::File::open(&session.path) else {
        return BTreeMap::new();
    };
    std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<JsonValue>(line.trim()).ok())
        .flat_map(|record| {
            let mut connector_names = Vec::new();
            let Some(contents) = record
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(JsonValue::as_array)
            else {
                return connector_names;
            };
            for content in contents {
                if content.get("type").and_then(JsonValue::as_str) != Some("tool_use") {
                    continue;
                }
                let Some(tool_name) = content.get("name").and_then(JsonValue::as_str) else {
                    continue;
                };
                let Some(input) = content.get("input") else {
                    continue;
                };
                let connector_name = if tool_name == MCP_TOOL_CALL {
                    input
                        .get("server")
                        .and_then(JsonValue::as_str)
                        .map(str::trim)
                        .filter(|server_id| !server_id.is_empty())
                        .map(str::to_lowercase)
                        .and_then(|server_id| {
                            project_connector_metadata
                                .names_by_server_id
                                .get(&server_id)
                                .or_else(|| cached_connector_names_by_server_id.get(&server_id))
                        })
                        .cloned()
                } else if tool_name.starts_with("mcp__") {
                    input
                        .get("mcpDetails")
                        .and_then(|details| details.get("serverName"))
                        .and_then(JsonValue::as_str)
                        .and_then(|server_name| {
                            crate::sessions::normalized_connector_display_name(Some(server_name))
                        })
                        .map(|server_name| server_name.to_lowercase())
                        .and_then(|server_name| {
                            project_connector_metadata
                                .names_by_normalized_name
                                .get(&server_name)
                        })
                        .cloned()
                } else {
                    None
                };
                if let Some(connector_name) = connector_name {
                    connector_names.push(connector_name);
                }
            }
            connector_names
        })
        .map(|name| (name.to_lowercase(), name))
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

fn read_project_mcp_server_metadata(server_root: &Path) -> Option<ProjectMcpServerMetadata> {
    let contents = fs::read_to_string(server_root.join(PROJECT_MCP_SERVER_METADATA_PATH)).ok()?;
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
