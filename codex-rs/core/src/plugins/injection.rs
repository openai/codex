use std::collections::BTreeSet;

use codex_app_server_protocol::AuthMode;
use codex_connectors::metadata::connector_display_label;
use codex_protocol::models::ResponseItem;

use crate::connectors;
use crate::context::ContextualUserFragment;
use crate::context::PluginInstructions;
use crate::plugins::PluginCapabilitySummary;
use crate::plugins::render_explicit_plugin_instructions;
use crate::plugins::routing::PluginRoute;
use crate::plugins::routing::route_for_plugin;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ToolInfo;

pub(crate) fn build_plugin_injections(
    mentioned_plugins: &[PluginCapabilitySummary],
    mcp_tools: &[ToolInfo],
    available_connectors: &[connectors::AppInfo],
    auth_mode: Option<AuthMode>,
) -> Vec<ResponseItem> {
    if mentioned_plugins.is_empty() {
        return Vec::new();
    }

    // Turn each explicit plugin mention into a developer hint that points the
    // model at the plugin's visible MCP servers, enabled apps, and skill prefix.
    mentioned_plugins
        .iter()
        .filter_map(|plugin| {
            let route = route_for_plugin(plugin, auth_mode);
            let (available_mcp_servers, available_apps) = match route {
                PluginRoute::Default => (
                    available_mcp_servers_for_plugin(plugin, mcp_tools),
                    available_apps_for_plugin(plugin, available_connectors),
                ),
                PluginRoute::McpOnly => (
                    available_mcp_servers_for_plugin(plugin, mcp_tools),
                    Vec::new(),
                ),
            };
            render_explicit_plugin_instructions(plugin, &available_mcp_servers, &available_apps)
                .map(PluginInstructions::new)
                .map(ContextualUserFragment::into)
        })
        .collect()
}

fn available_mcp_servers_for_plugin(
    plugin: &PluginCapabilitySummary,
    mcp_tools: &[ToolInfo],
) -> Vec<String> {
    mcp_tools
        .iter()
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
        .collect()
}

fn available_apps_for_plugin(
    plugin: &PluginCapabilitySummary,
    available_connectors: &[connectors::AppInfo],
) -> Vec<String> {
    available_connectors
        .iter()
        .filter(|connector| {
            connector.is_enabled
                && connector
                    .plugin_display_names
                    .iter()
                    .any(|plugin_name| plugin_name == &plugin.display_name)
        })
        .map(connector_display_label)
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use codex_plugin::AppConnectorId;
    use codex_protocol::models::ContentItem;
    use pretty_assertions::assert_eq;
    use rmcp::model::JsonObject;
    use rmcp::model::Tool;

    use super::*;

    fn plugin(app_ids: &[&str], mcp_server_names: &[&str]) -> PluginCapabilitySummary {
        PluginCapabilitySummary {
            config_name: "sample@personal".to_string(),
            display_name: "Sample".to_string(),
            app_connector_ids: app_ids
                .iter()
                .map(|id| AppConnectorId((*id).to_string()))
                .collect(),
            mcp_server_names: mcp_server_names
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
            ..PluginCapabilitySummary::default()
        }
    }

    fn mcp_tool(server_name: &str) -> ToolInfo {
        ToolInfo {
            server_name: server_name.to_string(),
            supports_parallel_tool_calls: false,
            server_origin: None,
            callable_name: "search".to_string(),
            callable_namespace: format!("mcp__{server_name}"),
            namespace_description: None,
            tool: Tool::new(
                "search".to_string(),
                "Search sample data".to_string(),
                Arc::new(JsonObject::default()),
            ),
            connector_id: None,
            connector_name: None,
            plugin_display_names: vec!["Sample".to_string()],
        }
    }

    fn connector(
        id: &str,
        name: &str,
        is_accessible: bool,
        is_enabled: bool,
    ) -> connectors::AppInfo {
        connectors::AppInfo {
            id: id.to_string(),
            name: name.to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible,
            is_enabled,
            plugin_display_names: vec!["Sample".to_string()],
        }
    }

    fn render_single_injection(
        plugin: PluginCapabilitySummary,
        mcp_tools: Vec<ToolInfo>,
        available_connectors: Vec<connectors::AppInfo>,
        auth_mode: Option<AuthMode>,
    ) -> String {
        let items =
            build_plugin_injections(&[plugin], &mcp_tools, &available_connectors, auth_mode);
        assert_eq!(items.len(), 1);

        let ResponseItem::Message { role, content, .. } = &items[0] else {
            panic!("expected developer message");
        };
        assert_eq!(role, "developer");
        let [ContentItem::InputText { text }] = content.as_slice() else {
            panic!("expected one input text content item");
        };
        text.clone()
    }

    struct GuidanceCase {
        name: &'static str,
        plugin: PluginCapabilitySummary,
        mcp_tools: Vec<ToolInfo>,
        available_connectors: Vec<connectors::AppInfo>,
        auth_mode: Option<AuthMode>,
        expected_contains: &'static [&'static str],
        expected_not_contains: &'static [&'static str],
    }

    #[test]
    fn renders_plugin_mention_guidance_by_auth_route() {
        let cases = vec![
            GuidanceCase {
                name: "chatgpt dual-surface app available",
                plugin: plugin(&["connector_sample"], &["sample-mcp"]),
                mcp_tools: vec![mcp_tool("sample-mcp")],
                available_connectors: vec![connector(
                    "connector_sample",
                    "Sample App",
                    /*is_accessible*/ true,
                    /*is_enabled*/ true,
                )],
                auth_mode: Some(AuthMode::Chatgpt),
                expected_contains: &[
                    "MCP servers from this plugin available in this session: `sample-mcp`.",
                    "Apps from this plugin available in this session: `Sample App`.",
                ],
                expected_not_contains: &["do not use this plugin's MCP servers as a fallback"],
            },
            GuidanceCase {
                name: "api-key dual-surface plugin",
                plugin: plugin(&["connector_sample"], &["sample-mcp"]),
                mcp_tools: vec![mcp_tool("sample-mcp")],
                available_connectors: vec![connector(
                    "connector_sample",
                    "Sample App",
                    /*is_accessible*/ true,
                    /*is_enabled*/ true,
                )],
                auth_mode: Some(AuthMode::ApiKey),
                expected_contains: &[
                    "MCP servers from this plugin available in this session: `sample-mcp`.",
                ],
                expected_not_contains: &["Apps from this plugin available"],
            },
            GuidanceCase {
                name: "chatgpt dual-surface app unavailable falls back to mcp guidance",
                plugin: plugin(&["connector_sample"], &["sample-mcp"]),
                mcp_tools: vec![mcp_tool("sample-mcp")],
                available_connectors: Vec::new(),
                auth_mode: Some(AuthMode::Chatgpt),
                expected_contains: &[
                    "MCP servers from this plugin available in this session: `sample-mcp`.",
                ],
                expected_not_contains: &[
                    "Apps from this plugin available",
                    "Apps from this plugin are not available",
                    "do not use this plugin's MCP servers as a fallback",
                ],
            },
            GuidanceCase {
                name: "chatgpt mcp-only plugin",
                plugin: plugin(&[], &["sample-mcp"]),
                mcp_tools: vec![mcp_tool("sample-mcp")],
                available_connectors: Vec::new(),
                auth_mode: Some(AuthMode::Chatgpt),
                expected_contains: &[
                    "MCP servers from this plugin available in this session: `sample-mcp`.",
                ],
                expected_not_contains: &[],
            },
        ];

        for case in cases {
            let rendered = render_single_injection(
                case.plugin,
                case.mcp_tools,
                case.available_connectors,
                case.auth_mode,
            );

            for expected in case.expected_contains {
                assert!(
                    rendered.contains(*expected),
                    "{} should contain {:?}\n{}",
                    case.name,
                    expected,
                    rendered
                );
            }
            for unexpected in case.expected_not_contains {
                assert!(
                    !rendered.contains(*unexpected),
                    "{} should not contain {:?}\n{}",
                    case.name,
                    unexpected,
                    rendered
                );
            }
        }
    }
}
