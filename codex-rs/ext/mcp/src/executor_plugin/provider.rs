use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_core_plugins::ResolvedExecutorPlugin;
use codex_exec_server::ExecutorFileSystem;
use codex_mcp::PluginMcpServerPlacement;
use codex_mcp::parse_plugin_mcp_config;
use codex_plugin::PluginResourceLocator;
use codex_plugin::ResolvedPlugin;
use codex_plugin::ResolvedPluginLocation;
use codex_plugin::manifest::PluginManifestMcpServers;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use std::io;
use thiserror::Error;

const DEFAULT_MCP_CONFIG_FILE: &str = ".mcp.json";

/// Loads MCP declarations from resolved plugins through their owning executor.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ExecutorPluginMcpProvider;

/// Failure to load an executor plugin's MCP declarations.
#[derive(Debug, Error)]
pub(super) enum ExecutorPluginMcpProviderError {
    #[error("failed to read MCP config for selected plugin `{plugin_id}` at `{path}`: {source}")]
    ReadConfig {
        plugin_id: String,
        path: AbsolutePathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse MCP config for selected plugin `{plugin_id}` at `{path}`: {source}")]
    ParseConfig {
        plugin_id: String,
        path: AbsolutePathBuf,
        #[source]
        source: serde_json::Error,
    },
}

impl ExecutorPluginMcpProvider {
    /// Returns stdio servers declared by `plugin`, bound to its environment.
    pub(super) async fn load(
        &self,
        plugin: &ResolvedExecutorPlugin,
    ) -> Result<Vec<(String, McpServerConfig)>, ExecutorPluginMcpProviderError> {
        let ResolvedPluginLocation::Environment { root, .. } = plugin.plugin().location();

        load_from_file_system(plugin.plugin(), root, plugin.file_system()).await
    }
}

async fn load_from_file_system(
    plugin: &ResolvedPlugin,
    plugin_root: &AbsolutePathBuf,
    file_system: &dyn ExecutorFileSystem,
) -> Result<Vec<(String, McpServerConfig)>, ExecutorPluginMcpProviderError> {
    let ResolvedPluginLocation::Environment { environment_id, .. } = plugin.location();
    let plugin_id = plugin.selected_root_id();
    let (contents, config_path) = match plugin.manifest().paths.mcp_servers.as_ref() {
        Some(PluginManifestMcpServers::Path(PluginResourceLocator::Environment {
            path, ..
        })) => {
            let config_uri = PathUri::from_abs_path(path);
            (
                file_system
                    .read_file_text(&config_uri, /*sandbox*/ None)
                    .await
                    .map_err(|source| ExecutorPluginMcpProviderError::ReadConfig {
                        plugin_id: plugin_id.to_string(),
                        path: path.clone(),
                        source,
                    })?,
                path.clone(),
            )
        }
        Some(PluginManifestMcpServers::Object(object_config)) => (
            object_config.clone(),
            plugin_root.join(".codex-plugin/plugin.json"),
        ),
        None => {
            let config_path = plugin_root.join(DEFAULT_MCP_CONFIG_FILE);
            let config_uri = PathUri::from_abs_path(&config_path);
            let contents = match file_system
                .read_file_text(&config_uri, /*sandbox*/ None)
                .await
            {
                Ok(contents) => contents,
                Err(source) if source.kind() == io::ErrorKind::NotFound => {
                    return Ok(Vec::new());
                }
                Err(source) => {
                    return Err(ExecutorPluginMcpProviderError::ReadConfig {
                        plugin_id: plugin_id.to_string(),
                        path: config_path.clone(),
                        source,
                    });
                }
            };
            (contents, config_path)
        }
    };
    let parsed = parse_plugin_mcp_config(
        plugin_root.as_path(),
        &contents,
        PluginMcpServerPlacement::Environment { environment_id },
    )
    .map_err(|source| ExecutorPluginMcpProviderError::ParseConfig {
        plugin_id: plugin_id.to_string(),
        path: config_path,
        source,
    })?;

    for error in parsed.errors {
        tracing::warn!(
            plugin = plugin_id,
            server = error.name,
            error = error.message,
            "ignoring invalid executor plugin MCP server"
        );
    }

    Ok(parsed
        .servers
        .into_iter()
        .filter_map(|(name, config)| match &config.transport {
            McpServerTransportConfig::Stdio { .. } => Some((name, config)),
            McpServerTransportConfig::StreamableHttp { .. } => {
                tracing::warn!(
                    plugin = plugin_id,
                    server = name,
                    "ignoring HTTP MCP server from executor plugin"
                );
                None
            }
        })
        .collect())
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
