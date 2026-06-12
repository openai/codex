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

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum RequestPluginInstallArgs {
    Single(RequestPluginInstallSingleArgs),
    Picker(RequestPluginInstallPickerArgs),
}

#[derive(Debug, Deserialize)]
pub struct RequestPluginInstallSingleArgs {
    pub tool_type: DiscoverableToolType,
    pub action_type: DiscoverableToolAction,
    pub tool_id: String,
    pub suggest_reason: String,
}

#[derive(Debug, Deserialize)]
pub struct RequestPluginInstallPickerArgs {
    pub action_type: DiscoverableToolAction,
    pub suggest_reason: String,
    pub title: Option<String>,
    pub entries: Vec<RequestPluginInstallPickerEntry>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct RequestPluginInstallPickerEntry {
    pub id: String,
    pub tool_id: String,
    pub tool_name: String,
    pub tool_type: DiscoverableToolType,
    pub description: Option<String>,
}

impl RequestPluginInstallArgs {
    pub fn action_type(&self) -> DiscoverableToolAction {
        match self {
            Self::Single(args) => args.action_type,
            Self::Picker(args) => args.action_type,
        }
    }

    pub fn suggest_reason(&self) -> &str {
        match self {
            Self::Single(args) => args.suggest_reason.as_str(),
            Self::Picker(args) => args.suggest_reason.as_str(),
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RequestPluginInstallResult {
    pub completed: bool,
    pub user_confirmed: bool,
    pub tool_type: DiscoverableToolType,
    pub action_type: DiscoverableToolAction,
    pub tool_id: String,
    pub tool_name: String,
    pub suggest_reason: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RequestPluginInstallPickerResult {
    pub completed: bool,
    pub user_confirmed: bool,
    pub action_type: DiscoverableToolAction,
    pub suggest_reason: String,
    pub installed_entries: Vec<RequestPluginInstallInstalledEntry>,
    pub entries: Vec<RequestPluginInstallEntryResult>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RequestPluginInstallInstalledEntry {
    pub entry_id: String,
    pub tool_id: String,
    pub tool_type: DiscoverableToolType,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RequestPluginInstallEntryResult {
    pub entry_id: String,
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
    pub suggest_reason: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<&'a str>,
    pub entries: Vec<RequestPluginInstallEntryMeta<'a>>,
}

#[derive(Debug)]
pub struct RequestPluginInstallResolvedPickerEntry<'a> {
    pub entry_id: &'a str,
    pub tool: &'a DiscoverableTool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RequestPluginInstallEntryMeta<'a> {
    pub id: &'a str,
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

pub fn build_request_plugin_install_elicitation_request(
    server_name: &str,
    thread_id: String,
    turn_id: String,
    args: &RequestPluginInstallSingleArgs,
    suggest_reason: &str,
    tool: &DiscoverableTool,
) -> McpServerElicitationRequestParams {
    let message = suggest_reason.to_string();

    McpServerElicitationRequestParams {
        thread_id,
        turn_id: Some(turn_id),
        server_name: server_name.to_string(),
        request: McpServerElicitationRequest::Form {
            meta: Some(json!(build_request_plugin_install_meta(
                args.action_type,
                suggest_reason,
                tool,
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

pub fn build_request_plugin_install_picker_elicitation_request<'a>(
    server_name: &str,
    thread_id: String,
    turn_id: String,
    args: &'a RequestPluginInstallPickerArgs,
    suggest_reason: &'a str,
    resolved_entries: &'a [RequestPluginInstallResolvedPickerEntry<'a>],
) -> Result<McpServerElicitationRequestParams, String> {
    let message = suggest_reason.to_string();

    Ok(McpServerElicitationRequestParams {
        thread_id,
        turn_id: Some(turn_id),
        server_name: server_name.to_string(),
        request: McpServerElicitationRequest::Form {
            meta: Some(json!(build_request_plugin_install_picker_meta(
                args,
                suggest_reason,
                resolved_entries,
            )?)),
            message,
            requested_schema: McpElicitationSchema {
                schema_uri: None,
                type_: McpElicitationObjectType::Object,
                properties: BTreeMap::new(),
                required: None,
            },
        },
    })
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

fn build_request_plugin_install_picker_meta<'a>(
    args: &'a RequestPluginInstallPickerArgs,
    suggest_reason: &'a str,
    resolved_entries: &'a [RequestPluginInstallResolvedPickerEntry<'a>],
) -> Result<RequestPluginInstallMeta<'a>, String> {
    let entries = args
        .entries
        .iter()
        .map(|entry| picker_entry_meta(entry, resolved_entries))
        .collect::<Result<Vec<_>, String>>()?;

    Ok(RequestPluginInstallMeta {
        codex_approval_kind: REQUEST_PLUGIN_INSTALL_APPROVAL_KIND_VALUE,
        persist: None,
        suggest_type: args.action_type,
        suggest_reason,
        title: args.title.as_deref(),
        entries,
    })
}

fn picker_entry_meta<'a>(
    entry: &'a RequestPluginInstallPickerEntry,
    resolved_entries: &'a [RequestPluginInstallResolvedPickerEntry<'a>],
) -> Result<RequestPluginInstallEntryMeta<'a>, String> {
    let tool = resolved_entries
        .iter()
        .find(|resolved_entry| resolved_entry.entry_id == entry.id)
        .map(|resolved_entry| resolved_entry.tool)
        .ok_or_else(|| format!("missing resolved picker entry for {}", entry.id))?;
    Ok(build_request_plugin_install_entry_meta(
        entry.id.as_str(),
        entry
            .description
            .as_deref()
            .or_else(|| discoverable_tool_description(tool)),
        tool,
    ))
}

fn build_request_plugin_install_meta<'a>(
    action_type: DiscoverableToolAction,
    suggest_reason: &'a str,
    tool: &'a DiscoverableTool,
) -> RequestPluginInstallMeta<'a> {
    RequestPluginInstallMeta {
        codex_approval_kind: REQUEST_PLUGIN_INSTALL_APPROVAL_KIND_VALUE,
        persist: Some(REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE),
        suggest_type: action_type,
        suggest_reason,
        title: None,
        entries: vec![build_request_plugin_install_entry_meta(
            tool.id(),
            discoverable_tool_description(tool),
            tool,
        )],
    }
}

fn build_request_plugin_install_entry_meta<'a>(
    id: &'a str,
    description: Option<&'a str>,
    tool: &'a DiscoverableTool,
) -> RequestPluginInstallEntryMeta<'a> {
    let (remote_plugin_id, app_connector_ids) = match tool {
        DiscoverableTool::Connector(_) => (None, None),
        DiscoverableTool::Plugin(plugin) => (
            plugin.remote_plugin_id.as_deref(),
            Some(plugin.app_connector_ids.as_slice()),
        ),
    };

    RequestPluginInstallEntryMeta {
        id,
        tool_id: tool.id(),
        tool_name: tool.name(),
        tool_type: tool.tool_type(),
        description,
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
