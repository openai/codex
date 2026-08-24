use super::load_mcp_servers_from_file;
use super::load_mcp_servers_from_manifest_object;
use super::plugin_mcp_config_paths;
use crate::manifest::PluginManifestFormat;
use crate::manifest::PluginManifestMcpServers;
use crate::manifest::parse_plugin_manifest;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use std::collections::HashMap;
use std::path::Path;
use tracing::warn;

pub(super) async fn apply_codex_env_overlay(
    plugin_root: &Path,
    agent_servers: &mut HashMap<String, McpServerConfig>,
) {
    if agent_servers.is_empty() {
        return;
    }

    let overlay_path = plugin_root.join(".codex-plugin/plugin.json");
    let Ok(contents) = tokio::fs::read_to_string(&overlay_path).await else {
        return;
    };
    let overlay_manifest = match parse_plugin_manifest(plugin_root, &overlay_path, &contents) {
        Ok(manifest) => manifest,
        Err(err) => {
            warn!(
                path = %overlay_path.display(),
                "failed to parse Codex Agent Plugin MCP overlay: {err}"
            );
            return;
        }
    };

    let overlay_servers = match &overlay_manifest.paths.mcp_servers {
        Some(PluginManifestMcpServers::Object(servers)) => {
            load_mcp_servers_from_manifest_object(plugin_root, servers).mcp_servers
        }
        Some(PluginManifestMcpServers::Path(_)) | None => {
            let mut servers = HashMap::new();
            for path in plugin_mcp_config_paths(plugin_root, &overlay_manifest.paths) {
                servers.extend(
                    load_mcp_servers_from_file(
                        plugin_root,
                        /*plugin_data_root*/ None,
                        PluginManifestFormat::Legacy,
                        &path,
                    )
                    .await
                    .mcp_servers,
                );
            }
            servers
        }
    };

    for (name, overlay_server) in overlay_servers {
        let Some(agent_server) = agent_servers.get_mut(&name) else {
            continue;
        };
        let (
            McpServerTransportConfig::Stdio { env, env_vars, .. },
            McpServerTransportConfig::Stdio {
                env_vars: mut overlay_env_vars,
                ..
            },
        ) = (&mut agent_server.transport, overlay_server.transport)
        else {
            continue;
        };

        overlay_env_vars.retain(|variable| !variable.is_remote_source());

        if let Some(env) = env {
            env.retain(|name, value| {
                let Some(reference) = value
                    .strip_prefix("${")
                    .and_then(|reference| reference.strip_suffix('}'))
                else {
                    return true;
                };
                !overlay_env_vars.iter().any(|variable| {
                    environment_names_match(name, variable.name())
                        && environment_names_match(reference, variable.name())
                })
            });
        }
        env_vars.extend(overlay_env_vars);
    }
}

fn environment_names_match(left: &str, right: &str) -> bool {
    if cfg!(windows) {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}
