use codex_config::McpServerConfig;
use codex_core_plugins::ResolvedExecutorPlugin;
use codex_exec_server::ExecutorFileSystem;
use codex_mcp::parse_executor_plugin_mcp_config;
use codex_plugin::PluginResourceLocator;
use codex_plugin::ResolvedPlugin;
use codex_plugin::ResolvedPluginLocation;
use codex_plugin::manifest::PluginManifestMcpServers;
use codex_utils_path_uri::PathUri;
use codex_utils_path_uri::PathUriParseError;
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
        path: PathUri,
        #[source]
        source: io::Error,
    },
    #[error(
        "failed to resolve MCP config path `{relative_path}` below selected plugin `{plugin_id}` at `{root}`: {source}"
    )]
    InvalidConfigPath {
        plugin_id: String,
        root: PathUri,
        relative_path: &'static str,
        #[source]
        source: PathUriParseError,
    },
    #[error("failed to parse MCP config for selected plugin `{plugin_id}` at `{path}`: {source}")]
    ParseConfig {
        plugin_id: String,
        path: PathUri,
        #[source]
        source: serde_json::Error,
    },
}

impl ExecutorPluginMcpProvider {
    /// Returns MCP servers declared by `plugin`, bound to its environment.
    #[tracing::instrument(name = "mcp.executor_plugin.servers.load", skip_all)]
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
    plugin_root: &PathUri,
    file_system: &dyn ExecutorFileSystem,
) -> Result<Vec<(String, McpServerConfig)>, ExecutorPluginMcpProviderError> {
    let ResolvedPluginLocation::Environment { environment_id, .. } = plugin.location();
    let plugin_id = plugin.selected_root_id();
    let (contents, config_path) = match plugin.manifest().paths.mcp_servers.as_ref() {
        Some(PluginManifestMcpServers::Path(PluginResourceLocator::Environment {
            path, ..
        })) => {
            (
                file_system
                    .read_file_text(path, /*sandbox*/ None)
                    .await
                    .map_err(|source| ExecutorPluginMcpProviderError::ReadConfig {
                        plugin_id: plugin_id.to_string(),
                        path: path.clone(),
                        source,
                    })?,
                path.clone(),
            )
        }
        Some(PluginManifestMcpServers::Object(object_config)) => {
            let PluginResourceLocator::Environment { path, .. } = plugin.manifest_path();
            (object_config.clone(), path.clone())
        }
        None => {
            let config_path = plugin_root
                .join(DEFAULT_MCP_CONFIG_FILE)
                .map_err(|source| ExecutorPluginMcpProviderError::InvalidConfigPath {
                    plugin_id: plugin_id.to_string(),
                    root: plugin_root.clone(),
                    relative_path: DEFAULT_MCP_CONFIG_FILE,
                    source,
                })?;
            let contents = match file_system
                .read_file_text(&config_path, /*sandbox*/ None)
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
    let parsed = parse_executor_plugin_mcp_config(plugin_root, &contents, environment_id).map_err(
        |source| ExecutorPluginMcpProviderError::ParseConfig {
            plugin_id: plugin_id.to_string(),
            path: config_path,
            source,
        },
    )?;

    for error in parsed.errors {
        tracing::warn!(
            plugin = plugin_id,
            server = error.name,
            error = error.message,
            "ignoring invalid executor plugin MCP server"
        );
    }

    Ok(parsed.servers.into_iter().collect())
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
