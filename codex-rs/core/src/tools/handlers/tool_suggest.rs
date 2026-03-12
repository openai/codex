use std::collections::BTreeMap;
use std::collections::HashSet;

use async_trait::async_trait;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::McpElicitationObjectType;
use codex_app_server_protocol::McpElicitationSchema;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_rmcp_client::ElicitationAction;
use rmcp::model::RequestId;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use tracing::warn;

use crate::connectors;
use crate::function_tool::FunctionCallError;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::discoverable::DiscoverableTool;
use crate::tools::discoverable::DiscoverableToolAction;
use crate::tools::discoverable::DiscoverableToolType;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct ToolSuggestHandler;

pub(crate) const TOOL_SUGGEST_TOOL_NAME: &str = "tool_suggest";
const TOOL_SUGGEST_APPROVAL_KIND_VALUE: &str = "tool_suggestion";

#[derive(Debug, Deserialize)]
struct ToolSuggestArgs {
    tool_type: DiscoverableToolType,
    action_type: DiscoverableToolAction,
    tool_id: String,
    suggest_reason: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ToolSuggestResult {
    completed: bool,
    user_confirmed: bool,
    tool_type: DiscoverableToolType,
    action_type: DiscoverableToolAction,
    tool_id: String,
    tool_name: String,
    suggest_reason: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ToolSuggestMeta<'a> {
    codex_approval_kind: &'static str,
    tool_type: DiscoverableToolType,
    suggest_type: DiscoverableToolAction,
    suggest_reason: &'a str,
    tool_id: &'a str,
    tool_name: &'a str,
    install_url: &'a str,
}

#[async_trait]
impl ToolHandler for ToolSuggestHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            payload,
            session,
            turn,
            call_id,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::Fatal(format!(
                    "{TOOL_SUGGEST_TOOL_NAME} handler received unsupported payload"
                )));
            }
        };

        let args: ToolSuggestArgs = parse_arguments(&arguments)?;
        let suggest_reason = args.suggest_reason.trim();
        if suggest_reason.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "suggest_reason must not be empty".to_string(),
            ));
        }
        if args.tool_type == DiscoverableToolType::Plugin {
            return Err(FunctionCallError::RespondToModel(
                "plugin tool suggestions are not currently available".to_string(),
            ));
        }
        if args.action_type != DiscoverableToolAction::Install {
            return Err(FunctionCallError::RespondToModel(
                "connector tool suggestions currently support only action_type=\"install\""
                    .to_string(),
            ));
        }

        let auth = session.services.auth_manager.auth().await;
        let manager = session.services.mcp_connection_manager.read().await;
        let mcp_tools = manager.list_all_tools().await;
        drop(manager);
        let accessible_connectors = connectors::with_app_enabled_state(
            connectors::accessible_connectors_from_mcp_tools(&mcp_tools),
            &turn.config,
        );
        let discoverable_tools = connectors::list_tool_suggest_discoverable_tools_with_auth(
            &turn.config,
            auth.as_ref(),
            &accessible_connectors,
        )
        .await
        .map(|connectors| {
            connectors
                .into_iter()
                .map(DiscoverableTool::from)
                .collect::<Vec<_>>()
        })
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "tool suggestions are unavailable right now: {err}"
            ))
        })?;

        let connector = discoverable_tools
            .into_iter()
            .find_map(|tool| match tool {
                DiscoverableTool::Connector(connector) if connector.id == args.tool_id => {
                    Some(*connector)
                }
                DiscoverableTool::Connector(_) | DiscoverableTool::Plugin(_) => None,
            })
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(format!(
                    "tool_id must match one of the discoverable tools exposed by {TOOL_SUGGEST_TOOL_NAME}"
                ))
            })?;

        let request_id = RequestId::String(format!("tool_suggestion_{call_id}").into());
        let params = build_tool_suggestion_elicitation_request(
            session.conversation_id.to_string(),
            turn.sub_id.clone(),
            &args,
            suggest_reason,
            &connector,
        );
        let response = session
            .request_mcp_server_elicitation(turn.as_ref(), request_id, params)
            .await;
        let user_confirmed = response
            .as_ref()
            .is_some_and(|response| response.action == ElicitationAction::Accept);

        let completed = if user_confirmed {
            let manager = session.services.mcp_connection_manager.read().await;
            match manager.hard_refresh_codex_apps_tools_cache().await {
                Ok(mcp_tools) => {
                    let accessible_connectors = connectors::with_app_enabled_state(
                        connectors::accessible_connectors_from_mcp_tools(&mcp_tools),
                        &turn.config,
                    );
                    connectors::refresh_accessible_connectors_cache_from_mcp_tools(
                        &turn.config,
                        auth.as_ref(),
                        &mcp_tools,
                    );
                    verified_connector_suggestion_completed(
                        args.action_type,
                        connector.id.as_str(),
                        &accessible_connectors,
                    )
                }
                Err(err) => {
                    warn!(
                        "failed to refresh codex apps tools cache after tool suggestion for {}: {err:#}",
                        connector.id
                    );
                    false
                }
            }
        } else {
            false
        };

        if completed {
            session
                .merge_connector_selection(HashSet::from([connector.id.clone()]))
                .await;
        }

        let content = serde_json::to_string(&ToolSuggestResult {
            completed,
            user_confirmed,
            tool_type: args.tool_type,
            action_type: args.action_type,
            tool_id: connector.id,
            tool_name: connector.name,
            suggest_reason: suggest_reason.to_string(),
        })
        .map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize {TOOL_SUGGEST_TOOL_NAME} response: {err}"
            ))
        })?;

        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}

fn build_tool_suggestion_elicitation_request(
    thread_id: String,
    turn_id: String,
    args: &ToolSuggestArgs,
    suggest_reason: &str,
    connector: &AppInfo,
) -> McpServerElicitationRequestParams {
    let tool_name = connector.name.clone();
    let install_url = connector
        .install_url
        .clone()
        .unwrap_or_else(|| connectors::connector_install_url(&tool_name, &connector.id));

    let message = format!(
        "{tool_name} could help with this request.\n\n{suggest_reason}\n\nOpen ChatGPT to {} it, then confirm here if you finish.",
        args.action_type.as_str()
    );

    McpServerElicitationRequestParams {
        thread_id,
        turn_id: Some(turn_id),
        server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        request: McpServerElicitationRequest::Form {
            meta: Some(json!(build_tool_suggestion_meta(
                args.tool_type,
                args.action_type,
                suggest_reason,
                connector.id.as_str(),
                tool_name.as_str(),
                install_url.as_str(),
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

fn build_tool_suggestion_meta<'a>(
    tool_type: DiscoverableToolType,
    action_type: DiscoverableToolAction,
    suggest_reason: &'a str,
    tool_id: &'a str,
    tool_name: &'a str,
    install_url: &'a str,
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

fn verified_connector_suggestion_completed(
    action_type: DiscoverableToolAction,
    tool_id: &str,
    accessible_connectors: &[AppInfo],
) -> bool {
    accessible_connectors
        .iter()
        .find(|connector| connector.id == tool_id)
        .is_some_and(|connector| match action_type {
            DiscoverableToolAction::Install => connector.is_accessible,
            DiscoverableToolAction::Enable => connector.is_accessible && connector.is_enabled,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn build_tool_suggestion_elicitation_request_uses_expected_shape() {
        let args = ToolSuggestArgs {
            tool_type: DiscoverableToolType::Connector,
            action_type: DiscoverableToolAction::Install,
            tool_id: "connector_2128aebfecb84f64a069897515042a44".to_string(),
            suggest_reason: "Plan and reference events from your calendar".to_string(),
        };
        let connector = AppInfo {
            id: "connector_2128aebfecb84f64a069897515042a44".to_string(),
            name: "Google Calendar".to_string(),
            description: Some("Plan events and schedules.".to_string()),
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some(
                "https://chatgpt.com/apps/google-calendar/connector_2128aebfecb84f64a069897515042a44"
                    .to_string(),
            ),
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        };

        let request = build_tool_suggestion_elicitation_request(
            "thread-1".to_string(),
            "turn-1".to_string(),
            &args,
            "Plan and reference events from your calendar",
            &connector,
        );

        assert_eq!(
            request,
            McpServerElicitationRequestParams {
                thread_id: "thread-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                request: McpServerElicitationRequest::Form {
                    meta: Some(json!(ToolSuggestMeta {
                        codex_approval_kind: TOOL_SUGGEST_APPROVAL_KIND_VALUE,
                        tool_type: DiscoverableToolType::Connector,
                        suggest_type: DiscoverableToolAction::Install,
                        suggest_reason: "Plan and reference events from your calendar",
                        tool_id: "connector_2128aebfecb84f64a069897515042a44",
                        tool_name: "Google Calendar",
                        install_url: "https://chatgpt.com/apps/google-calendar/connector_2128aebfecb84f64a069897515042a44",
                    })),
                    message: "Google Calendar could help with this request.\n\nPlan and reference events from your calendar\n\nOpen ChatGPT to install it, then confirm here if you finish.".to_string(),
                    requested_schema: McpElicitationSchema {
                        schema_uri: None,
                        type_: McpElicitationObjectType::Object,
                        properties: BTreeMap::new(),
                        required: None,
                    },
                },
            }
        );
    }

    #[test]
    fn build_tool_suggestion_meta_uses_expected_shape() {
        let meta = build_tool_suggestion_meta(
            DiscoverableToolType::Connector,
            DiscoverableToolAction::Install,
            "Find and reference emails from your inbox",
            "connector_68df038e0ba48191908c8434991bbac2",
            "Gmail",
            "https://chatgpt.com/apps/gmail/connector_68df038e0ba48191908c8434991bbac2",
        );

        assert_eq!(
            meta,
            ToolSuggestMeta {
                codex_approval_kind: TOOL_SUGGEST_APPROVAL_KIND_VALUE,
                tool_type: DiscoverableToolType::Connector,
                suggest_type: DiscoverableToolAction::Install,
                suggest_reason: "Find and reference emails from your inbox",
                tool_id: "connector_68df038e0ba48191908c8434991bbac2",
                tool_name: "Gmail",
                install_url: "https://chatgpt.com/apps/gmail/connector_68df038e0ba48191908c8434991bbac2",
            }
        );
    }

    #[test]
    fn verified_connector_suggestion_completed_requires_installed_connector() {
        let accessible_connectors = vec![AppInfo {
            id: "calendar".to_string(),
            name: "Google Calendar".to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: true,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        }];

        assert!(verified_connector_suggestion_completed(
            DiscoverableToolAction::Install,
            "calendar",
            &accessible_connectors,
        ));
        assert!(!verified_connector_suggestion_completed(
            DiscoverableToolAction::Install,
            "gmail",
            &accessible_connectors,
        ));
    }

    #[test]
    fn verified_connector_suggestion_completed_requires_enabled_connector_for_enable() {
        let accessible_connectors = vec![
            AppInfo {
                id: "calendar".to_string(),
                name: "Google Calendar".to_string(),
                description: None,
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: None,
                is_accessible: true,
                is_enabled: false,
                plugin_display_names: Vec::new(),
            },
            AppInfo {
                id: "gmail".to_string(),
                name: "Gmail".to_string(),
                description: None,
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: None,
                is_accessible: true,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            },
        ];

        assert!(!verified_connector_suggestion_completed(
            DiscoverableToolAction::Enable,
            "calendar",
            &accessible_connectors,
        ));
        assert!(verified_connector_suggestion_completed(
            DiscoverableToolAction::Enable,
            "gmail",
            &accessible_connectors,
        ));
    }
}
