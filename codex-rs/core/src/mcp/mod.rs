pub mod auth;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use async_channel::unbounded;
use codex_protocol::protocol::McpListToolsResponseEvent;
use codex_protocol::protocol::SandboxPolicy;
use mcp_types::Tool as McpTool;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::mcp::auth::compute_auth_statuses;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::mcp_connection_manager::SandboxState;

const MCP_TOOL_NAME_PREFIX: &str = "mcp";
const MCP_TOOL_NAME_DELIMITER: &str = "__";

pub async fn collect_mcp_snapshot(config: &Config) -> McpListToolsResponseEvent {
    if config.mcp_servers.is_empty() {
        return McpListToolsResponseEvent {
            tools: HashMap::new(),
            resources: HashMap::new(),
            resource_templates: HashMap::new(),
            auth_statuses: HashMap::new(),
        };
    }

    let auth_status_entries = compute_auth_statuses(
        config.mcp_servers.iter(),
        config.mcp_oauth_credentials_store_mode,
    )
    .await;

    let mut mcp_connection_manager = McpConnectionManager::default();
    let (tx_event, rx_event) = unbounded();
    drop(rx_event);
    let cancel_token = CancellationToken::new();

    // Use ReadOnly sandbox policy for MCP snapshot collection (safest default)
    let sandbox_state = SandboxState {
        sandbox_policy: SandboxPolicy::ReadOnly,
        codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
        sandbox_cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
    };

    mcp_connection_manager
        .initialize(
            config.mcp_servers.clone(),
            config.mcp_oauth_credentials_store_mode,
            auth_status_entries.clone(),
            tx_event,
            cancel_token.clone(),
            sandbox_state,
        )
        .await;

    let snapshot =
        collect_mcp_snapshot_from_manager(&mcp_connection_manager, auth_status_entries).await;

    cancel_token.cancel();

    snapshot
}

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
    tools: &HashMap<String, McpTool>,
) -> HashMap<String, HashMap<String, McpTool>> {
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

pub(crate) async fn collect_mcp_snapshot_from_manager(
    mcp_connection_manager: &McpConnectionManager,
    auth_status_entries: HashMap<String, crate::mcp::auth::McpAuthStatusEntry>,
) -> McpListToolsResponseEvent {
    let (tools, resources, resource_templates) = tokio::join!(
        mcp_connection_manager.list_all_tools(),
        mcp_connection_manager.list_all_resources(),
        mcp_connection_manager.list_all_resource_templates(),
    );

    let auth_statuses = auth_status_entries
        .iter()
        .map(|(name, entry)| (name.clone(), entry.auth_status))
        .collect();

    McpListToolsResponseEvent {
        tools: tools
            .into_iter()
            .map(|(name, tool)| (name, tool.tool))
            .collect(),
        resources,
        resource_templates,
        auth_statuses,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcp_types::ToolInputSchema;
    use pretty_assertions::assert_eq;

    fn make_tool(name: &str) -> McpTool {
        McpTool {
            annotations: None,
            description: None,
            input_schema: ToolInputSchema {
                properties: None,
                required: None,
                r#type: "object".to_string(),
            },
            name: name.to_string(),
            output_schema: None,
            title: None,
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
