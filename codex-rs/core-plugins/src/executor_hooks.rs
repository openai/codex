//! Executor-discovered plugin hook admission and trusted MCP routing.

use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::HookHandlerConfig;
use codex_config::types::PluginsConfigToml;
use codex_exec_server::ExecutorCapabilityDiscoverySnapshot;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::MCP_TOOL_CODEX_APPS_META_KEY;
use codex_mcp::ToolInfo;
use codex_plugin::ExecutorPluginHookSource;
use codex_plugin::PluginId;
use codex_plugin::is_allowlisted_bundled_cleanup_hook;
use codex_plugin::manifest::PluginManifestHooks;
use codex_protocol::capabilities::CapabilityRootLocation;
use serde_json::Map;
use serde_json::Value;

use crate::manifest::parse_plugin_manifest_uri;

/// Returns accepted inline hook sources from executor-discovered plugin manifests.
/// Each source carries its trusted MCP routing metadata. `lookup_enabled_tool` must use a
/// consistent catalog/config snapshot, include app-only tools, and exclude tools disabled by
/// app policy.
///
/// Executor scoped hooks are best-effort because executor capabilities can become available
/// after earlier lifecycle events have passed.
///
/// Note: Executor manifests are not signed yet, so temporarily we only admit the known cleanup
/// hooks from the bundled cleanup allowlist and the remote Browser plugin.
pub fn executor_plugin_hook_sources<'a>(
    snapshot: &ExecutorCapabilityDiscoverySnapshot,
    plugins_config: &PluginsConfigToml,
    lookup_enabled_tool: impl Fn(&str, &str) -> Option<&'a ToolInfo>,
) -> Vec<ExecutorPluginHookSource> {
    let mut sources = Vec::new();

    for entry in snapshot.roots() {
        if !plugins_config.allows_plugin(&entry.selected_root.id) {
            continue;
        }
        let Ok(plugin_id) = PluginId::parse(&entry.selected_root.id) else {
            continue;
        };
        let CapabilityRootLocation::Environment {
            environment_id,
            path: plugin_root,
        } = &entry.selected_root.location;
        let Ok(discovery) = &entry.result else {
            continue;
        };
        let Some(plugin) = &discovery.plugin else {
            continue;
        };
        let Ok(manifest) = parse_plugin_manifest_uri(
            plugin_root,
            &plugin.manifest.path,
            &plugin.manifest.contents,
        ) else {
            continue;
        };
        // Only inline hooks are supported for now, so skip any other source types.
        let Some(PluginManifestHooks::Inline(hook_files)) = manifest.paths.hooks else {
            continue;
        };

        for (hook_index, hook_file) in hook_files.into_iter().enumerate() {
            let manifest_relative_path = plugin
                .manifest
                .path
                .relative_path_from(plugin_root)
                .unwrap_or_else(|| plugin.manifest.path.to_string());

            sources.push(ExecutorPluginHookSource {
                plugin_id: plugin_id.clone(),
                environment_id: environment_id.clone(),
                mcp_environment_id: None,
                mcp_metadata: None,
                plugin_root: plugin_root.clone(),
                manifest_path: plugin.manifest.path.clone(),
                source_relative_path: format!("{manifest_relative_path}#hooks[{hook_index}]"),
                hooks: hook_file.hooks,
            });
        }
    }

    // FIXME: Remove this temporary filter once executor plugin hooks can be trusted.
    sources
        .into_iter()
        .filter_map(|mut source| {
            let plugin_id = source.plugin_id.as_key();
            for (event, groups) in source.hooks.matcher_groups_mut() {
                groups.retain_mut(|group| {
                    group.hooks.retain(|handler| {
                        let app_connector_id = match handler {
                            HookHandlerConfig::McpTool { server, tool, .. }
                                if server == CODEX_APPS_MCP_SERVER_NAME =>
                            {
                                lookup_enabled_tool(server, tool)
                                    .and_then(|info| info.connector_id.as_deref())
                            }
                            _ => None,
                        };
                        is_allowlisted_bundled_cleanup_hook(
                            &plugin_id,
                            event,
                            group.matcher.as_deref(),
                            handler,
                            app_connector_id,
                        )
                    });
                    !group.hooks.is_empty()
                });
            }
            (!source.hooks.is_empty()).then_some(source)
        })
        .filter_map(|source| resolve_mcp_routing(source, &lookup_enabled_tool))
        .collect()
}

/// Resolves routing for an admitted MCP hook source.
fn resolve_mcp_routing<'a>(
    mut source: ExecutorPluginHookSource,
    lookup_enabled_tool: &impl Fn(&str, &str) -> Option<&'a ToolInfo>,
) -> Option<ExecutorPluginHookSource> {
    let HookHandlerConfig::McpTool { server, tool, .. } = source
        .hooks
        .matcher_groups_mut()
        .into_iter()
        .find_map(|(_, groups)| groups.first())?
        .hooks
        .first()?
    else {
        return None;
    };
    if server != CODEX_APPS_MCP_SERVER_NAME {
        return Some(source);
    }

    let tool_info = lookup_enabled_tool(server, tool)?;
    let routing_metadata = tool_info
        .tool
        .meta
        .as_ref()?
        .get(MCP_TOOL_CODEX_APPS_META_KEY)?
        .as_object()?;
    routing_metadata.get("resource_uri")?.as_str()?;
    source.mcp_environment_id = Some(DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string());
    source.mcp_metadata = Some(Map::from_iter([(
        MCP_TOOL_CODEX_APPS_META_KEY.to_string(),
        Value::Object(routing_metadata.clone()),
    )]));
    Some(source)
}

#[cfg(test)]
#[path = "executor_hooks_tests.rs"]
mod tests;
