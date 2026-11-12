//! Resolves literal Apps wording with a UTF-8 byte limit and the existing bundled namespace.
//! Configured strings are never interpolated or trimmed.

use codex_protocol::openai_models::AppsMessages;

// Reserve 200 bytes for markers and transition notices within a 1,000-byte fragment.
const MAX_USAGE_INSTRUCTIONS_BYTES: usize = 800;

/// Bounded Apps wording and its source, independently of connector availability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedAppsInstructions {
    pub usage_instructions: String,
    pub catalog_override: bool,
}

impl ResolvedAppsInstructions {
    pub(super) fn new(messages: Option<&AppsMessages>, mcp_server_name: &str) -> Self {
        let catalog = messages
            .and_then(|messages| messages.usage_instructions.as_deref())
            .filter(|text| {
                text.is_empty()
                    || (text.len() <= MAX_USAGE_INSTRUCTIONS_BYTES && !text.trim().is_empty())
            });
        Self {
            usage_instructions: catalog.map(str::to_owned).unwrap_or_else(|| format!(
            "\n## Apps (Connectors)\nApps (Connectors) can be explicitly triggered in user messages in the format `[$app-name](app://{{connector_id}})`. Apps can also be implicitly triggered as long as the context suggests usage of available apps.\nAn app is equivalent to a set of MCP tools within the `{mcp_server_name}` MCP.\nAn installed app's MCP tools are either provided to you already, or can be lazy-loaded through the `tool_search` tool. If `tool_search` is available, the apps that are searchable by `tools_search` will be listed by it.\nDo not additionally call list_mcp_resources or list_mcp_resource_templates for apps.\n"
            )),
            catalog_override: catalog.is_some(),
        }
    }
}
