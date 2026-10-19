use crate::cloud_plugin::MAX_CONNECTOR_IDS_PER_PLUGIN;
use crate::cloud_plugin::MAX_PLUGIN_DISPLAY_NAME_CHARS;
use crate::plugin_contributor_state::SelectedPluginMetadata;
use codex_connectors::parse_plugin_app_config;
use codex_core_plugins::PluginCatalogEntry;
use codex_core_plugins::PluginSourceLocation;
use codex_core_plugins::manifest::UriPluginManifest;
use codex_core_plugins::manifest::parse_plugin_manifest_uri;
use codex_exec_server::CapabilityRootDiscovery;
use codex_exec_server::DiscoveredPluginFiles;
use codex_extension_api::SelectedPluginContribution;
use codex_mcp::PluginMcpConfigParseOutcome;
use codex_mcp::normalize_executor_plugin_mcp_declarations;
use codex_mcp::parse_executor_plugin_mcp_config;
use codex_plugin::manifest::PluginManifestMcpServers;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use std::collections::HashSet;

pub(super) fn manifest_from_discovery<'a>(
    selected_root: &SelectedCapabilityRoot,
    discovery: &'a CapabilityRootDiscovery,
) -> Option<(UriPluginManifest, &'a DiscoveredPluginFiles)> {
    for warning in &discovery.warnings {
        tracing::warn!(
            selected_root = selected_root.id,
            warning,
            "exec-server capability discovery warning"
        );
    }
    if let Some(error) = &discovery.error {
        tracing::warn!(
            selected_root = selected_root.id,
            error,
            "exec-server capability discovery failed"
        );
        return None;
    }
    let plugin_files = discovery.plugin.as_ref()?;
    match parse_plugin_manifest_uri(
        &discovery.path,
        &plugin_files.manifest.path,
        &plugin_files.manifest.contents,
    ) {
        Ok(manifest) => Some((manifest, plugin_files)),
        Err(error) => {
            tracing::warn!(
                selected_root = selected_root.id,
                path = %plugin_files.manifest.path,
                %error,
                "failed to parse exec-server-discovered plugin manifest"
            );
            None
        }
    }
}

pub(super) fn metadata_from_discovery(
    selected_root: &SelectedCapabilityRoot,
    discovery: &CapabilityRootDiscovery,
    plugin_files: &DiscoveredPluginFiles,
    manifest: UriPluginManifest,
) -> SelectedPluginContribution {
    let CapabilityRootLocation::Environment { environment_id, .. } = &selected_root.location;
    let servers = match manifest.paths.mcp_servers.as_ref() {
        Some(PluginManifestMcpServers::Object(contents)) => {
            parse_mcp_servers(selected_root, &discovery.path, contents, environment_id)
        }
        Some(PluginManifestMcpServers::Path(path)) => plugin_files
            .mcp_config
            .as_ref()
            .filter(|file| file.path == *path)
            .map(|file| {
                parse_mcp_servers(
                    selected_root,
                    &discovery.path,
                    &file.contents,
                    environment_id,
                )
            })
            .unwrap_or_else(|| {
                tracing::warn!(
                    selected_root = selected_root.id,
                    path = %path,
                    "exec-server capability bundle omitted declared MCP config"
                );
                Vec::new()
            }),
        None => plugin_files
            .mcp_config
            .as_ref()
            .map(|file| {
                parse_mcp_servers(
                    selected_root,
                    &discovery.path,
                    &file.contents,
                    environment_id,
                )
            })
            .unwrap_or_default(),
    };
    let connector_ids = manifest
        .paths
        .apps
        .as_ref()
        .and_then(|path| {
            plugin_files
                .apps_config
                .as_ref()
                .filter(|file| file.path == *path)
                .or_else(|| {
                    tracing::warn!(
                        selected_root = selected_root.id,
                        path = %path,
                        "exec-server capability bundle omitted declared connector config"
                    );
                    None
                })
        })
        .and_then(|file| match parse_plugin_app_config(&file.contents) {
            Ok(declarations) => Some(declarations),
            Err(error) => {
                tracing::warn!(
                    selected_root = selected_root.id,
                    path = %file.path,
                    %error,
                    "failed to parse exec-server-discovered connector config"
                );
                None
            }
        })
        .unwrap_or_default()
        .into_iter()
        .map(|declaration| declaration.connector_id.0)
        .collect();

    SelectedPluginContribution {
        plugin_display_name: manifest.display_name().to_string(),
        source_environment_id: environment_id.clone(),
        servers,
        connector_ids,
    }
}

pub(super) fn v2_metadata_from_catalog(
    entry: &PluginCatalogEntry,
) -> Vec<(String, SelectedPluginMetadata)> {
    let plugin_display_name: String = entry
        .display_name
        .chars()
        .take(MAX_PLUGIN_DISPLAY_NAME_CHARS)
        .collect();
    let mut seen_connector_ids = HashSet::new();
    let connector_ids: Vec<String> = entry
        .connector_ids
        .iter()
        .filter(|id| seen_connector_ids.insert(id.as_str()))
        .take(MAX_CONNECTOR_IDS_PER_PLUGIN)
        .cloned()
        .collect();
    let installations = entry
        .locations
        .iter()
        .filter_map(|location| match location {
            PluginSourceLocation::Executor {
                environment_id,
                plugin_id,
                root,
            } => Some((environment_id, plugin_id, root)),
            PluginSourceLocation::Cloud { .. } => None,
        });
    let mut projections = Vec::new();
    for (index, (environment_id, plugin_id, root)) in installations.enumerate() {
        let root_id = format!("{environment_id}:{plugin_id}");
        // Only the first retained installation supplies MCPs; the others retain package attribution.
        let servers = if index == 0 {
            let parsed = normalize_executor_plugin_mcp_declarations(
                root,
                entry.mcp_servers.clone(),
                environment_id,
            );
            servers_from_outcome(&root_id, parsed)
        } else {
            Vec::new()
        };
        let metadata = SelectedPluginMetadata {
            plugin_display_name: plugin_display_name.clone(),
            servers,
            connector_ids: connector_ids.clone(),
        };
        projections.push((root_id, metadata));
    }
    if entry
        .locations
        .iter()
        .any(|location| matches!(location, PluginSourceLocation::Cloud { .. }))
    {
        projections.push((
            format!("cloud:{}", entry.id.as_str()),
            SelectedPluginMetadata {
                plugin_display_name,
                servers: Vec::new(),
                connector_ids,
            },
        ));
    }
    projections
}

fn parse_mcp_servers(
    selected_root: &SelectedCapabilityRoot,
    plugin_root: &codex_utils_path_uri::PathUri,
    contents: &str,
    environment_id: &str,
) -> Vec<(String, codex_config::McpServerConfig)> {
    let parsed = match parse_executor_plugin_mcp_config(plugin_root, contents, environment_id) {
        Ok(parsed) => parsed,
        Err(error) => {
            tracing::warn!(
                selected_root = selected_root.id,
                %error,
                "failed to parse exec-server-discovered MCP config"
            );
            return Vec::new();
        }
    };
    servers_from_outcome(&selected_root.id, parsed)
}

fn servers_from_outcome(
    selected_root_id: &str,
    parsed: PluginMcpConfigParseOutcome,
) -> Vec<(String, codex_config::McpServerConfig)> {
    for error in parsed.errors {
        tracing::warn!(
            selected_root = selected_root_id,
            server = error.name,
            error = error.message,
            "ignoring invalid exec-server-discovered MCP server"
        );
    }
    parsed.servers.into_iter().collect()
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
