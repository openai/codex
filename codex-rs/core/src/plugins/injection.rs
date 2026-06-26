use std::collections::BTreeSet;

use codex_protocol::models::ResponseItem;

use crate::context::ContextualUserFragment;
use crate::context::PluginInstructions;
use crate::plugins::PluginCapabilitySummary;
use crate::plugins::render_explicit_plugin_instructions;
use codex_mcp::ToolInfo;

pub(crate) fn build_plugin_injections(
    mentioned_plugins: &[PluginCapabilitySummary],
    mcp_tools: &[ToolInfo],
) -> Vec<ResponseItem> {
    if mentioned_plugins.is_empty() {
        return Vec::new();
    }

    // Turn each explicit plugin mention into a developer hint that points the
    // model at the plugin's visible MCP servers and skill prefix.
    mentioned_plugins
        .iter()
        .filter_map(|plugin| {
            let available_mcp_servers = mcp_tools
                .iter()
                .filter(|tool| {
                    tool.plugin_display_names
                        .iter()
                        .any(|plugin_name| plugin_name == &plugin.display_name)
                })
                .map(|tool| tool.callable_namespace.clone())
                .collect::<BTreeSet<String>>()
                .into_iter()
                .collect::<Vec<_>>();
            render_explicit_plugin_instructions(plugin, &available_mcp_servers)
                .map(PluginInstructions::new)
                .map(ContextualUserFragment::into)
        })
        .collect()
}

#[cfg(test)]
#[path = "injection_tests.rs"]
mod tests;
