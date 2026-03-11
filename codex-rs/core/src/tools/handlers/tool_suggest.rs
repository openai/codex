use std::collections::BTreeMap;
use std::collections::HashSet;

use async_trait::async_trait;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::McpElicitationObjectType;
use codex_app_server_protocol::McpElicitationSchema;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_protocol::models::FunctionCallOutputBody;
use codex_rmcp_client::ElicitationAction;
use rmcp::model::RequestId;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use tracing::warn;

use crate::connectors;
use crate::function_tool::FunctionCallError;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct ToolSuggestHandler;

pub(crate) const TOOL_SUGGEST_TOOL_NAME: &str = "tool_suggest";
const TOOL_SUGGEST_APPROVAL_KIND_KEY: &str = "codex_approval_kind";
const TOOL_SUGGEST_APPROVAL_KIND_VALUE: &str = "tool_suggestion";
const TOOL_SUGGEST_TOOL_TYPE_KEY: &str = "tool_type";
const TOOL_SUGGEST_SUGGEST_TYPE_KEY: &str = "suggest_type";
const TOOL_SUGGEST_REASON_KEY: &str = "suggest_reason";
const TOOL_SUGGEST_TOOL_ID_KEY: &str = "tool_id";
const TOOL_SUGGEST_TOOL_NAME_KEY: &str = "tool_name";
const TOOL_SUGGEST_INSTALL_URL_KEY: &str = "install_url";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolSuggestToolType {
    Connector,
    Plugin,
}

impl ToolSuggestToolType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Connector => "connector",
            Self::Plugin => "plugin",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolSuggestActionType {
    Install,
    Enable,
}

impl ToolSuggestActionType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Enable => "enable",
        }
    }
}

#[derive(Debug, Deserialize)]
struct ToolSuggestArgs {
    tool_type: ToolSuggestToolType,
    action_type: ToolSuggestActionType,
    tool_id: String,
    suggest_reason: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ToolSuggestResult {
    completed: bool,
    tool_type: ToolSuggestToolType,
    action_type: ToolSuggestActionType,
    tool_id: String,
    tool_name: String,
    suggest_reason: String,
}

#[async_trait]
impl ToolHandler for ToolSuggestHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
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

        let connector = match args.tool_type {
            ToolSuggestToolType::Connector => {
                if args.action_type != ToolSuggestActionType::Install {
                    return Err(FunctionCallError::RespondToModel(
                        "connector tool suggestions currently support only action_type=\"install\""
                            .to_string(),
                    ));
                }

                let auth = session.services.auth_manager.auth().await;
                let discoverable_connectors =
                    connectors::list_tool_suggest_discoverable_connectors_with_auth(
                        &turn.config,
                        auth.as_ref(),
                        &[],
                    )
                    .await
                    .map_err(|err| {
                        FunctionCallError::RespondToModel(format!(
                            "tool suggestions are unavailable right now: {err}"
                        ))
                    })?;

                discoverable_connectors
                    .into_iter()
                    .find(|connector| connector.id == args.tool_id)
                    .ok_or_else(|| {
                        FunctionCallError::RespondToModel(format!(
                            "tool_id must match one of the discoverable connectors exposed by {TOOL_SUGGEST_TOOL_NAME}"
                        ))
                    })?
            }
            ToolSuggestToolType::Plugin => {
                return Err(FunctionCallError::RespondToModel(
                    "plugin tool suggestions are not currently available".to_string(),
                ));
            }
        };

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
        let completed = response
            .as_ref()
            .is_some_and(|response| response.action == ElicitationAction::Accept);

        if completed {
            session
                .merge_connector_selection(HashSet::from([connector.id.clone()]))
                .await;
            let manager = session.services.mcp_connection_manager.read().await;
            if let Err(err) = manager.hard_refresh_codex_apps_tools_cache().await {
                warn!(
                    "failed to refresh codex apps tools cache after tool suggestion for {}: {err:#}",
                    connector.id
                );
            }
        }

        let content = serde_json::to_string(&ToolSuggestResult {
            completed,
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

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
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
        "{tool_name} could help with this request.\n\n{suggest_reason}\n\nOpen ChatGPT to install it, then confirm here if you finish."
    );

    McpServerElicitationRequestParams {
        thread_id,
        turn_id: Some(turn_id),
        server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        request: McpServerElicitationRequest::Form {
            meta: Some(build_tool_suggestion_meta(
                args.tool_type,
                args.action_type,
                suggest_reason,
                connector.id.as_str(),
                tool_name.as_str(),
                install_url.as_str(),
            )),
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

fn build_tool_suggestion_meta(
    tool_type: ToolSuggestToolType,
    action_type: ToolSuggestActionType,
    suggest_reason: &str,
    tool_id: &str,
    tool_name: &str,
    install_url: &str,
) -> serde_json::Value {
    json!({
        TOOL_SUGGEST_APPROVAL_KIND_KEY: TOOL_SUGGEST_APPROVAL_KIND_VALUE,
        TOOL_SUGGEST_TOOL_TYPE_KEY: tool_type.as_str(),
        TOOL_SUGGEST_SUGGEST_TYPE_KEY: action_type.as_str(),
        TOOL_SUGGEST_REASON_KEY: suggest_reason,
        TOOL_SUGGEST_TOOL_ID_KEY: tool_id,
        TOOL_SUGGEST_TOOL_NAME_KEY: tool_name,
        TOOL_SUGGEST_INSTALL_URL_KEY: install_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn build_tool_suggestion_elicitation_request_uses_expected_shape() {
        let args = ToolSuggestArgs {
            tool_type: ToolSuggestToolType::Connector,
            action_type: ToolSuggestActionType::Install,
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
                    meta: Some(json!({
                        "codex_approval_kind": "tool_suggestion",
                        "tool_type": "connector",
                        "suggest_type": "install",
                        "suggest_reason": "Plan and reference events from your calendar",
                        "tool_id": "connector_2128aebfecb84f64a069897515042a44",
                        "tool_name": "Google Calendar",
                        "install_url": "https://chatgpt.com/apps/google-calendar/connector_2128aebfecb84f64a069897515042a44",
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
            ToolSuggestToolType::Connector,
            ToolSuggestActionType::Install,
            "Find and reference emails from your inbox",
            "connector_68df038e0ba48191908c8434991bbac2",
            "Gmail",
            "https://chatgpt.com/apps/gmail/connector_68df038e0ba48191908c8434991bbac2",
        );

        assert_eq!(
            meta,
            json!({
                "codex_approval_kind": "tool_suggestion",
                "tool_type": "connector",
                "suggest_type": "install",
                "suggest_reason": "Find and reference emails from your inbox",
                "tool_id": "connector_68df038e0ba48191908c8434991bbac2",
                "tool_name": "Gmail",
                "install_url": "https://chatgpt.com/apps/gmail/connector_68df038e0ba48191908c8434991bbac2",
            })
        );
    }
}
