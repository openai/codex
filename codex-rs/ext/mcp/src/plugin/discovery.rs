use codex_connectors::parse_plugin_app_config;
use codex_core_plugins::manifest::parse_plugin_manifest_uri;
use codex_exec_server::CapabilityRootDiscovery;
use codex_mcp::parse_executor_plugin_mcp_config;
use codex_plugin::manifest::PluginManifestMcpServers;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;

use super::SelectedPluginMetadata;

pub(super) fn metadata_from_discovery(
    selected_root: &SelectedCapabilityRoot,
    discovery: &CapabilityRootDiscovery,
) -> Option<SelectedPluginMetadata> {
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
    let manifest = match parse_plugin_manifest_uri(
        &discovery.path,
        &plugin_files.manifest.path,
        &plugin_files.manifest.contents,
    ) {
        Ok(manifest) => manifest,
        Err(error) => {
            tracing::warn!(
                selected_root = selected_root.id,
                path = %plugin_files.manifest.path,
                %error,
                "failed to parse exec-server-discovered plugin manifest"
            );
            return None;
        }
    };
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

    Some(SelectedPluginMetadata {
        plugin_id: selected_root.id.clone(),
        plugin_display_name: manifest.display_name().to_string(),
        servers,
        connector_ids,
    })
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
    for error in parsed.errors {
        tracing::warn!(
            selected_root = selected_root.id,
            server = error.name,
            error = error.message,
            "ignoring invalid exec-server-discovered MCP server"
        );
    }
    parsed.servers.into_iter().collect()
}
