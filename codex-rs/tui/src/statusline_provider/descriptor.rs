//! Interpret the statusline entrypoint advertised by one MCP tool.

use super::McpServerName;
use super::McpToolName;
use super::ProviderId;
use codex_plugin::PluginId;
use codex_protocol::mcp::Tool;
use std::cell::Cell;
use url::SyntaxViolation;
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StatuslineProvider {
    pub(crate) id: ProviderId,
    pub(crate) title: String,
    pub(crate) plugin_label: Option<String>,
    pub(crate) uri: String,
    pub(super) plugin_id: Option<PluginId>,
}

impl StatuslineProvider {
    pub(super) fn from_tool(
        server: &McpServerName,
        plugin_id: Option<&PluginId>,
        tool: &Tool,
    ) -> Option<Self> {
        let meta = tool.meta.as_ref()?;
        let entries = meta.get("openai/ui")?.get("entrypoints")?.as_array()?;
        let mut valid_uris = entries
            .iter()
            .filter(|entry| entry["type"] == "statusline")
            .filter_map(|entry| entry["resourceUri"].as_str())
            .filter(|uri| {
                // The resource read forwards the original spelling. Reject errors the URL parser recovers from.
                let valid = Cell::new(true);
                let violation = |issue| {
                    if issue != SyntaxViolation::EmbeddedCredentials {
                        valid.set(false);
                    }
                };
                uri.starts_with("ui://")
                    && Url::options()
                        .syntax_violation_callback(Some(&violation))
                        .parse(uri)
                        .is_ok()
                    && valid.get()
            });
        let uri = valid_uris.next()?;
        if valid_uris.next().is_some() {
            tracing::warn!(
                server = server.as_str(),
                tool = tool.name.as_str(),
                "ignoring MCP tool with multiple valid statusline entrypoints"
            );
            return None;
        }
        Some(Self {
            id: ProviderId {
                server: server.clone(),
                tool: McpToolName(tool.name.clone()),
            },
            title: tool.title.clone().unwrap_or_else(|| tool.name.clone()),
            plugin_label: plugin_id.map(PluginId::as_key),
            uri: uri.to_owned(),
            plugin_id: plugin_id.cloned(),
        })
    }
}
