use codex_config::HookEventsToml;
use codex_config::HookHandlerConfig;
use codex_config::MatcherGroup;
use codex_exec_server::ExecutorCapabilityDiscoverySnapshot;
use codex_plugin::ExecutorPluginHookSource;
use codex_plugin::PluginId;
use codex_plugin::manifest::PluginManifestHooks;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::protocol::HookEventName;

use crate::manifest::parse_plugin_manifest_uri;

struct AllowlistedExecutorPluginHook {
    plugin_id: &'static str,
    event: HookEventName,
    server: &'static str,
    tool: &'static str,
}

// Executor plugin manifests are unsigned, so temporarily hardcode the expected
// bundled cleanup hook identity and MCP target until plugin signing lands.
const ALLOWLISTED_EXECUTOR_PLUGIN_HOOKS: &[AllowlistedExecutorPluginHook] =
    &[AllowlistedExecutorPluginHook {
        plugin_id: "computer-use@openai-bundled",
        event: HookEventName::Stop,
        server: "node_repl",
        tool: "turn_ended",
    }];

/// Returns accepted inline hook sources from executor-discovered plugin manifests.
///
/// Executor scoped hooks are best-effort because executor capabilities can become available
/// after earlier lifecycle events have passed.
///
/// Note: Executor manifests are not signed yet, so temporarily we only admit the known Computer Use cleanup hook.
pub fn executor_plugin_hook_sources(
    snapshot: &ExecutorCapabilityDiscoverySnapshot,
) -> Vec<ExecutorPluginHookSource> {
    let mut sources = Vec::new();

    for entry in snapshot.roots() {
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
                plugin_root: plugin_root.clone(),
                manifest_path: plugin.manifest.path.clone(),
                source_relative_path: format!("{manifest_relative_path}#hooks[{hook_index}]"),
                hooks: hook_file.hooks,
            });
        }
    }

    // FIXME: Remove this temporary filter once executor plugin hooks can be trusted.
    sources.into_iter().filter_map(allowlisted_source).collect()
}

fn allowlisted_source(mut source: ExecutorPluginHookSource) -> Option<ExecutorPluginHookSource> {
    let allowlisted_hook = ALLOWLISTED_EXECUTOR_PLUGIN_HOOKS.iter().find(|hook| {
        hook.plugin_id == source.plugin_id.as_key() && hook.event == HookEventName::Stop
    })?;
    let handler = source
        .hooks
        .stop
        .into_iter()
        .filter(|group| group.matcher.is_none())
        .flat_map(|group| group.hooks)
        .find(|handler| {
            matches!(
                handler,
                HookHandlerConfig::McpTool { server, tool, .. }
                    if server == allowlisted_hook.server && tool == allowlisted_hook.tool
            )
        })?;

    source.hooks = HookEventsToml {
        stop: vec![MatcherGroup {
            matcher: None,
            hooks: vec![handler],
        }],
        ..Default::default()
    };
    Some(source)
}

#[cfg(test)]
#[path = "executor_hooks_tests.rs"]
mod tests;
