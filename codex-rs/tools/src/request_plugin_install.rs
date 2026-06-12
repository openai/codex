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

pub const REQUEST_PLUGIN_INSTALL_APPROVAL_KIND_VALUE: &str = "tool_suggestion";
pub const REQUEST_PLUGIN_INSTALL_PERSIST_KEY: &str = "persist";
pub const REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE: &str = "always";
const REQUEST_PLUGIN_INSTALL_MESSAGE: &str = "Choose integrations";

#[derive(Debug, Deserialize)]
pub struct RequestPluginInstallArgs {
    pub action_type: DiscoverableToolAction,
    pub entries: Option<Vec<RequestPluginInstallPickerEntry>>,
    pub categories: Option<Vec<RequestPluginInstallPickerCategory>>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct RequestPluginInstallPickerEntry {
    pub tool_id: String,
    pub tool_type: DiscoverableToolType,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct RequestPluginInstallPickerCategory {
    pub title: String,
    pub entries: Vec<RequestPluginInstallPickerEntry>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RequestPluginInstallResult {
    pub completed: bool,
    pub user_confirmed: bool,
    pub action_type: DiscoverableToolAction,
    pub entries: Vec<RequestPluginInstallEntryResult>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct RequestPluginInstallInstalledEntry {
    pub tool_id: String,
    pub tool_type: DiscoverableToolType,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RequestPluginInstallEntryResult {
    pub tool_type: DiscoverableToolType,
    pub tool_id: String,
    pub tool_name: String,
    pub completed: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RequestPluginInstallMeta<'a> {
    pub codex_approval_kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persist: Option<&'static str>,
    pub suggest_type: DiscoverableToolAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<RequestPluginInstallEntryMeta<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<RequestPluginInstallCategoryMeta<'a>>>,
}

#[derive(Debug)]
pub struct RequestPluginInstallResolvedPickerEntry<'a> {
    pub category_index: Option<usize>,
    pub tool: &'a DiscoverableTool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RequestPluginInstallEntryMeta<'a> {
    pub tool_id: &'a str,
    pub tool_name: &'a str,
    pub tool_type: DiscoverableToolType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_url: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_plugin_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_connector_ids: Option<&'a [String]>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RequestPluginInstallCategoryMeta<'a> {
    pub title: &'a str,
    pub entries: Vec<RequestPluginInstallEntryMeta<'a>>,
}

pub fn build_request_plugin_install_elicitation_request<'a>(
    server_name: &str,
    thread_id: String,
    turn_id: String,
    args: &'a RequestPluginInstallArgs,
    resolved_entries: &'a [RequestPluginInstallResolvedPickerEntry<'a>],
) -> McpServerElicitationRequestParams {
    McpServerElicitationRequestParams {
        thread_id,
        turn_id: Some(turn_id),
        server_name: server_name.to_string(),
        request: McpServerElicitationRequest::Form {
            meta: Some(json!(build_request_plugin_install_meta(
                args,
                resolved_entries
            ))),
            message: REQUEST_PLUGIN_INSTALL_MESSAGE.to_string(),
            requested_schema: McpElicitationSchema {
                schema_uri: None,
                type_: McpElicitationObjectType::Object,
                properties: BTreeMap::new(),
                required: None,
            },
        },
    }
}

pub fn all_requested_connectors_picked_up(
    expected_connector_ids: &[String],
    accessible_connectors: &[AppInfo],
) -> bool {
    expected_connector_ids.iter().all(|connector_id| {
        verified_connector_install_completed(connector_id, accessible_connectors)
    })
}

pub fn verified_connector_install_completed(
    tool_id: &str,
    accessible_connectors: &[AppInfo],
) -> bool {
    accessible_connectors
        .iter()
        .find(|connector| connector.id == tool_id)
        .is_some_and(|connector| connector.is_accessible)
}

fn build_request_plugin_install_meta<'a>(
    args: &'a RequestPluginInstallArgs,
    resolved_entries: &'a [RequestPluginInstallResolvedPickerEntry<'a>],
) -> RequestPluginInstallMeta<'a> {
    let entries = args.entries.as_ref().map(|_| {
        resolved_entries
            .iter()
            .map(build_request_plugin_install_entry_meta)
            .collect()
    });
    let categories = args.categories.as_ref().map(|categories| {
        categories
            .iter()
            .enumerate()
            .map(|(category_index, category)| {
                let entries = resolved_entries
                    .iter()
                    .filter(|entry| entry.category_index == Some(category_index))
                    .map(build_request_plugin_install_entry_meta)
                    .collect();
                RequestPluginInstallCategoryMeta {
                    title: category.title.as_str(),
                    entries,
                }
            })
            .collect()
    });

    RequestPluginInstallMeta {
        codex_approval_kind: REQUEST_PLUGIN_INSTALL_APPROVAL_KIND_VALUE,
        persist: Some(REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE),
        suggest_type: args.action_type,
        entries,
        categories,
    }
}

fn build_request_plugin_install_entry_meta<'a>(
    entry: &'a RequestPluginInstallResolvedPickerEntry<'a>,
) -> RequestPluginInstallEntryMeta<'a> {
    let tool = entry.tool;
    let (remote_plugin_id, app_connector_ids) = match tool {
        DiscoverableTool::Connector(_) => (None, None),
        DiscoverableTool::Plugin(plugin) => (
            plugin.remote_plugin_id.as_deref(),
            Some(plugin.app_connector_ids.as_slice()),
        ),
    };

    RequestPluginInstallEntryMeta {
        tool_id: tool.id(),
        tool_name: tool.name(),
        tool_type: tool.tool_type(),
        description: discoverable_tool_description(tool),
        install_url: tool.install_url(),
        remote_plugin_id,
        app_connector_ids,
    }
}

fn discoverable_tool_description(tool: &DiscoverableTool) -> Option<&str> {
    match tool {
        DiscoverableTool::Connector(connector) => connector.description.as_deref(),
        DiscoverableTool::Plugin(plugin) => plugin.description.as_deref(),
    }
}

#[cfg(test)]
#[path = "request_plugin_install_tests.rs"]
mod tests;
