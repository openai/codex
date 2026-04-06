use std::collections::BTreeMap;

use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::McpElicitationObjectType;
use codex_app_server_protocol::McpElicitationSchema;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

use crate::DiscoverableTool;
use crate::DiscoverableToolAction;
use crate::DiscoverableToolType;

pub const TOOL_SUGGEST_APPROVAL_KIND_VALUE: &str = "tool_suggestion";

#[derive(Debug, Deserialize)]
pub struct ToolSuggestArgs {
    pub tool_type: DiscoverableToolType,
    pub action_type: DiscoverableToolAction,
    pub tool_id: String,
    pub suggest_reason: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ToolSuggestResult {
    pub completed: bool,
    pub user_confirmed: bool,
    pub tool_type: DiscoverableToolType,
    pub action_type: DiscoverableToolAction,
    pub tool_id: String,
    pub tool_name: String,
    pub suggest_reason: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ToolSuggestMeta<'a> {
    pub codex_approval_kind: &'static str,
    pub tool_type: DiscoverableToolType,
    pub suggest_type: DiscoverableToolAction,
    pub suggest_reason: &'a str,
    pub tool_id: &'a str,
    pub tool_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_url: Option<&'a str>,
}

pub fn build_tool_suggestion_elicitation_request(
    server_name: &str,
    thread_id: String,
    turn_id: String,
    args: &ToolSuggestArgs,
    suggest_reason: &str,
    tool: &DiscoverableTool,
) -> McpServerElicitationRequestParams {
    let tool_name = tool.name().to_string();
    let install_url = tool.install_url().map(ToString::to_string);
    let message = suggest_reason.to_string();

    McpServerElicitationRequestParams {
        thread_id,
        turn_id: Some(turn_id),
        server_name: server_name.to_string(),
        request: McpServerElicitationRequest::Form {
            meta: Some(json!(build_tool_suggestion_meta(
                args.tool_type,
                args.action_type,
                suggest_reason,
                tool.id(),
                tool_name.as_str(),
                install_url.as_deref(),
            ))),
            message,
            requested_schema: McpElicitationSchema {
                schema_uri: None,
                type_: McpElicitationObjectType::Object,
                properties: BTreeMap::new(),
                required: None,
            },
        },
    }
}

pub fn all_suggested_connectors_picked_up(
    expected_connector_ids: &[String],
    accessible_connectors: &[AppInfo],
) -> bool {
    expected_connector_ids.iter().all(|connector_id| {
        verified_connector_suggestion_completed(connector_id, accessible_connectors)
    })
}

pub fn verified_connector_suggestion_completed(
    tool_id: &str,
    accessible_connectors: &[AppInfo],
) -> bool {
    accessible_connectors
        .iter()
        .find(|connector| connector.id == tool_id)
        .is_some_and(|connector| connector.is_accessible)
}

fn build_tool_suggestion_meta<'a>(
    tool_type: DiscoverableToolType,
    action_type: DiscoverableToolAction,
    suggest_reason: &'a str,
    tool_id: &'a str,
    tool_name: &'a str,
    install_url: Option<&'a str>,
) -> ToolSuggestMeta<'a> {
    ToolSuggestMeta {
        codex_approval_kind: TOOL_SUGGEST_APPROVAL_KIND_VALUE,
        tool_type,
        suggest_type: action_type,
        suggest_reason,
        tool_id,
        tool_name,
        install_url,
    }
}

#[cfg(test)]
#[path = "tool_suggest_tests.rs"]
mod tests;
