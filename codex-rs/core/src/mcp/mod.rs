pub mod auth;
mod codex_apps;
mod skill_dependencies;
mod snapshot;

pub(crate) use codex_apps::effective_mcp_servers;
pub(crate) use codex_apps::with_codex_apps_mcp;
pub(crate) use skill_dependencies::maybe_prompt_and_install_mcp_dependencies;
pub(crate) use snapshot::collect_mcp_snapshot_from_manager;

use codex_protocol::mcp::Tool;
use std::collections::HashMap;

const MCP_TOOL_NAME_PREFIX: &str = "mcp";
const MCP_TOOL_NAME_DELIMITER: &str = "__";
pub(crate) const CODEX_APPS_MCP_SERVER_NAME: &str = "codex_apps";
pub use snapshot::collect_mcp_snapshot;

pub fn split_qualified_tool_name(qualified_name: &str) -> Option<(String, String)> {
    let mut parts = qualified_name.split(MCP_TOOL_NAME_DELIMITER);
    let prefix = parts.next()?;
    if prefix != MCP_TOOL_NAME_PREFIX {
        return None;
    }
    let server_name = parts.next()?;
    let tool_name: String = parts.collect::<Vec<_>>().join(MCP_TOOL_NAME_DELIMITER);
    if tool_name.is_empty() {
        return None;
    }
    Some((server_name.to_string(), tool_name))
}

pub fn group_tools_by_server(
    tools: &HashMap<String, Tool>,
) -> HashMap<String, HashMap<String, Tool>> {
    let mut grouped = HashMap::new();
    for (qualified_name, tool) in tools {
        if let Some((server_name, tool_name)) = split_qualified_tool_name(qualified_name) {
            grouped
                .entry(server_name)
                .or_insert_with(HashMap::new)
                .insert(tool_name, tool.clone());
        }
    }
    grouped
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_tool(name: &str) -> Tool {
        Tool {
            name: name.to_string(),
            title: None,
            description: None,
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
            output_schema: None,
            annotations: None,
            icons: None,
            meta: None,
        }
    }

    #[test]
    fn split_qualified_tool_name_returns_server_and_tool() {
        assert_eq!(
            split_qualified_tool_name("mcp__alpha__do_thing"),
            Some(("alpha".to_string(), "do_thing".to_string()))
        );
    }

    #[test]
    fn split_qualified_tool_name_rejects_invalid_names() {
        assert_eq!(split_qualified_tool_name("other__alpha__do_thing"), None);
        assert_eq!(split_qualified_tool_name("mcp__alpha__"), None);
    }

    #[test]
    fn group_tools_by_server_strips_prefix_and_groups() {
        let mut tools = HashMap::new();
        tools.insert("mcp__alpha__do_thing".to_string(), make_tool("do_thing"));
        tools.insert(
            "mcp__alpha__nested__op".to_string(),
            make_tool("nested__op"),
        );
        tools.insert("mcp__beta__do_other".to_string(), make_tool("do_other"));

        let mut expected_alpha = HashMap::new();
        expected_alpha.insert("do_thing".to_string(), make_tool("do_thing"));
        expected_alpha.insert("nested__op".to_string(), make_tool("nested__op"));

        let mut expected_beta = HashMap::new();
        expected_beta.insert("do_other".to_string(), make_tool("do_other"));

        let mut expected = HashMap::new();
        expected.insert("alpha".to_string(), expected_alpha);
        expected.insert("beta".to_string(), expected_beta);

        assert_eq!(group_tools_by_server(&tools), expected);
    }
}
