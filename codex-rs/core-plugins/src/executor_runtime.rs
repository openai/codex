use codex_config::McpServerConfig;
use codex_connectors::parse_plugin_app_config;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_mcp::parse_executor_plugin_mcp_config;
use codex_plugin::AppDeclaration;
use codex_plugin::PluginResourceLocator;
use codex_plugin::ResolvedPlugin;
use codex_plugin::ResolvedPluginLocation;
use codex_plugin::manifest::PluginManifestMcpServers;
use codex_utils_path_uri::PathUri;
use codex_utils_path_uri::PathUriParseError;
use std::io;
use thiserror::Error;

use crate::ExecutorPluginProvider;
use crate::ExecutorPluginProviderError;
use crate::ResolvedExecutorPlugin;

const DEFAULT_MCP_CONFIG_FILE: &str = ".mcp.json";

/// MCP and connector declarations read from one exact executor binding.
#[derive(Clone, Debug)]
pub struct ExecutorPluginRuntime {
    plugin: ResolvedPlugin,
    mcp_servers: Vec<(String, McpServerConfig)>,
    apps: Vec<AppDeclaration>,
}

/// Failure to project runtime capabilities from an executor plugin.
#[derive(Debug, Error)]
pub enum ExecutorPluginRuntimeError {
    #[error(transparent)]
    Resolve(#[from] ExecutorPluginProviderError),
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
    #[error("failed to read app config for selected plugin `{plugin_id}` at `{path}`: {source}")]
    ReadAppConfig {
        plugin_id: String,
        path: PathUri,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse app config for selected plugin `{plugin_id}` at `{path}`: {source}")]
    ParseAppConfig {
        plugin_id: String,
        path: PathUri,
        #[source]
        source: serde_json::Error,
    },
}

impl ExecutorPluginRuntime {
    /// Reads both runtime declaration files through the root's pinned filesystem.
    ///
    /// `Ok(None)` is intentionally not cacheable: the plugin manifest may appear
    /// once a deferred executor finishes starting.
    pub async fn project(
        root: &ResolvedSelectedCapabilityRoot,
    ) -> Result<Option<Self>, ExecutorPluginRuntimeError> {
        let Some(plugin) = ExecutorPluginProvider::resolve_pinned(root).await? else {
            return Ok(None);
        };
        let ResolvedPluginLocation::Environment {
            root: plugin_root, ..
        } = plugin.plugin().location();
        let mcp_servers =
            load_from_file_system(plugin.plugin(), plugin_root, plugin.file_system()).await?;
        let apps = match load_apps(&plugin).await {
            Ok(apps) => apps,
            Err(err) => {
                tracing::warn!(
                    plugin = plugin.plugin().selected_root_id(),
                    error = %err,
                    "ignoring invalid executor plugin app declarations"
                );
                Vec::new()
            }
        };
        Ok(Some(Self {
            plugin: plugin.plugin().clone(),
            mcp_servers,
            apps,
        }))
    }

    pub fn plugin(&self) -> &ResolvedPlugin {
        &self.plugin
    }

    pub fn mcp_servers(&self) -> &[(String, McpServerConfig)] {
        &self.mcp_servers
    }

    pub fn apps(&self) -> &[AppDeclaration] {
        &self.apps
    }
}

async fn load_from_file_system(
    plugin: &ResolvedPlugin,
    plugin_root: &PathUri,
    file_system: &dyn ExecutorFileSystem,
) -> Result<Vec<(String, McpServerConfig)>, ExecutorPluginRuntimeError> {
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
                    .map_err(|source| ExecutorPluginRuntimeError::ReadConfig {
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
                .map_err(|source| ExecutorPluginRuntimeError::InvalidConfigPath {
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
                    return Err(ExecutorPluginRuntimeError::ReadConfig {
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
        |source| ExecutorPluginRuntimeError::ParseConfig {
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

async fn load_apps(
    plugin: &ResolvedExecutorPlugin,
) -> Result<Vec<AppDeclaration>, ExecutorPluginRuntimeError> {
    let resolved_plugin = plugin.plugin();
    let plugin_id = resolved_plugin.selected_root_id();
    let Some(PluginResourceLocator::Environment {
        path: config_path, ..
    }) = resolved_plugin.manifest().paths.apps.as_ref()
    else {
        return Ok(Vec::new());
    };
    let contents = plugin
        .file_system()
        .read_file_text(config_path, /*sandbox*/ None)
        .await
        .map_err(|source| ExecutorPluginRuntimeError::ReadAppConfig {
            plugin_id: plugin_id.to_string(),
            path: config_path.clone(),
            source,
        })?;
    parse_plugin_app_config(&contents).map_err(|source| {
        ExecutorPluginRuntimeError::ParseAppConfig {
            plugin_id: plugin_id.to_string(),
            path: config_path.clone(),
            source,
        }
    })
}
