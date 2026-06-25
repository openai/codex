use anyhow::Context;
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
use std::io;

use crate::ExecutorPluginProvider;
use crate::ResolvedExecutorPlugin;

const DEFAULT_MCP_CONFIG_FILE: &str = ".mcp.json";

/// MCP and connector declarations read from one exact executor binding.
#[derive(Clone, Debug)]
pub struct ExecutorPluginRuntime {
    plugin: ResolvedPlugin,
    mcp_servers: Vec<(String, McpServerConfig)>,
    apps: Vec<AppDeclaration>,
}

impl ExecutorPluginRuntime {
    /// Reads both runtime declaration files through the root's pinned filesystem.
    ///
    /// `Ok(None)` is intentionally not cacheable: the plugin manifest may appear
    /// once a deferred executor finishes starting.
    pub async fn project(root: &ResolvedSelectedCapabilityRoot) -> anyhow::Result<Option<Self>> {
        let Some(plugin) = ExecutorPluginProvider::resolve_pinned(root).await? else {
            return Ok(None);
        };
        let ResolvedPluginLocation::Environment {
            root: plugin_root, ..
        } = plugin.plugin().location();
        let mcp_servers =
            load_from_file_system(plugin.plugin(), plugin_root, plugin.file_system()).await?;
        let apps = load_apps(&plugin).await?;
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
) -> anyhow::Result<Vec<(String, McpServerConfig)>> {
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
                    .with_context(|| {
                        format!(
                            "failed to read MCP config for selected plugin `{plugin_id}` at `{path}`"
                        )
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
                .with_context(|| {
                    format!(
                        "failed to resolve `{DEFAULT_MCP_CONFIG_FILE}` below selected plugin `{plugin_id}` at `{plugin_root}`"
                    )
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
                    return Err(source).with_context(|| {
                        format!(
                            "failed to read MCP config for selected plugin `{plugin_id}` at `{config_path}`"
                        )
                    });
                }
            };
            (contents, config_path)
        }
    };
    let parsed = parse_executor_plugin_mcp_config(plugin_root, &contents, environment_id)
        .with_context(|| {
            format!(
                "failed to parse MCP config for selected plugin `{plugin_id}` at `{config_path}`"
            )
        })?;

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

async fn load_apps(plugin: &ResolvedExecutorPlugin) -> anyhow::Result<Vec<AppDeclaration>> {
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
        .with_context(|| {
            format!(
                "failed to read app config for selected plugin `{plugin_id}` at `{config_path}`"
            )
        })?;
    parse_plugin_app_config(&contents).with_context(|| {
        format!("failed to parse app config for selected plugin `{plugin_id}` at `{config_path}`")
    })
}
