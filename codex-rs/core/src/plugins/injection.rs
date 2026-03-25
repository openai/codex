use std::collections::BTreeSet;
use std::collections::HashMap;

use crate::connectors;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp_connection_manager::ToolInfo;
use crate::plugins::PluginCapabilitySummary;
use crate::plugins::render_plugin_mention_instructions;

/// Turn-local data needed to render explicit plugin-mention guidance inside the
/// canonical pre-user developer envelope.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PluginMentionInstructionsContext {
    entries: Vec<PluginMentionInstructionsEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PluginMentionInstructionsEntry {
    plugin: PluginCapabilitySummary,
    available_mcp_servers: Vec<String>,
    available_apps: Vec<String>,
}

/// Capture the turn-local plugin/tool/app ingredients needed to render explicit plugin guidance
/// later in the canonical context builders, without re-listing MCP tools.
pub(crate) fn build_plugin_mention_instructions_context(
    mentioned_plugins: &[PluginCapabilitySummary],
    mcp_tools: &HashMap<String, ToolInfo>,
    available_connectors: &[connectors::AppInfo],
) -> PluginMentionInstructionsContext {
    if mentioned_plugins.is_empty() {
        return PluginMentionInstructionsContext::default();
    }

    let entries = mentioned_plugins
        .iter()
        .map(|plugin| {
            let available_mcp_servers = mcp_tools
                .values()
                .filter(|tool| {
                    tool.server_name != CODEX_APPS_MCP_SERVER_NAME
                        && tool
                            .plugin_display_names
                            .iter()
                            .any(|plugin_name| plugin_name == &plugin.display_name)
                })
                .map(|tool| tool.server_name.clone())
                .collect::<BTreeSet<String>>()
                .into_iter()
                .collect::<Vec<_>>();
            let available_apps = available_connectors
                .iter()
                .filter(|connector| {
                    connector.is_enabled
                        && connector
                            .plugin_display_names
                            .iter()
                            .any(|plugin_name| plugin_name == &plugin.display_name)
                })
                .map(connectors::connector_display_label)
                .collect::<BTreeSet<String>>()
                .into_iter()
                .collect::<Vec<_>>();

            PluginMentionInstructionsEntry {
                plugin: plugin.clone(),
                available_mcp_servers,
                available_apps,
            }
        })
        .collect();

    PluginMentionInstructionsContext { entries }
}

/// Render plugin-mention guidance from the already-resolved per-turn plugin context.
///
/// The live turn path builds `PluginMentionInstructionsContext` once from the current turn's
/// plugin/tool/app inventory, then whichever canonical context builder runs uses this renderer.
pub(crate) fn build_plugin_mention_developer_sections(
    plugin_mention_instructions: &PluginMentionInstructionsContext,
) -> Vec<String> {
    // Turn each explicit plugin mention into developer-message sections that
    // can be folded into the canonical pre-user developer envelope for this turn.
    plugin_mention_instructions
        .entries
        .iter()
        .filter_map(|entry| {
            render_plugin_mention_instructions(
                &entry.plugin,
                &entry.available_mcp_servers,
                &entry.available_apps,
            )
        })
        .collect()
}
