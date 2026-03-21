use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp_tool_call::MCP_TOOL_ARGS_CODEX_KEY;
use crate::mcp_tool_call::MCP_TOOL_ARGS_ELICITATION_DESCRIPTION_KEY;
use crate::mcp_tool_call::MCP_TOOL_ARGS_META_KEY;
use codex_protocol::protocol::APPS_INSTRUCTIONS_CLOSE_TAG;
use codex_protocol::protocol::APPS_INSTRUCTIONS_OPEN_TAG;

pub(crate) fn render_apps_section() -> String {
    let body = format!(
        "## Apps (Connectors)\nApps (Connectors) can be explicitly triggered in user messages in the format `[$app-name](app://{{connector_id}})`. Apps can also be implicitly triggered as long as the context suggests usage of available apps, the available apps will be listed by the `tool_search` tool.\nAn app is equivalent to a set of MCP tools within the `{CODEX_APPS_MCP_SERVER_NAME}` MCP.\nAn installed app's MCP tools are either provided to you already, or can be lazy-loaded through the `tool_search` tool.\nFor consequential `{CODEX_APPS_MCP_SERVER_NAME}` tool calls that require approval, you need to include user-facing approval copy in the tool arguments under:\n```json\n{{\n  \"{MCP_TOOL_ARGS_META_KEY}\": {{\n    \"{MCP_TOOL_ARGS_CODEX_KEY}\": {{\n      \"{MCP_TOOL_ARGS_ELICITATION_DESCRIPTION_KEY}\": \"Allow Calendar to create this event?\"\n    }}\n  }}\n}}\n```\nUse `{MCP_TOOL_ARGS_META_KEY}.{MCP_TOOL_ARGS_CODEX_KEY}.{MCP_TOOL_ARGS_ELICITATION_DESCRIPTION_KEY}` only for consequential app tools that may require approval. Make it a short user-facing approval sentence, not execution data, and do not rely on it being forwarded to the app.\nDo not additionally call list_mcp_resources or list_mcp_resource_templates for apps."
    );
    format!("{APPS_INSTRUCTIONS_OPEN_TAG}\n{body}\n{APPS_INSTRUCTIONS_CLOSE_TAG}")
}

#[cfg(test)]
mod tests {
    use super::render_apps_section;

    #[test]
    fn render_apps_section_mentions_elicitation_description_contract() {
        let rendered = render_apps_section();

        assert!(rendered.contains("`_meta._codex.elicitation_description`"));
        assert!(
            rendered
                .contains("\"elicitation_description\": \"Allow Calendar to create this event?\"")
        );
        assert!(rendered.contains("not execution data"));
        assert!(rendered.contains("do not rely on it being forwarded to the app"));
    }
}
