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
    #[serde(skip_serializing_if = "Option::is_none")]
    install_url: Option<&'a str>,
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
        if args.action_type != DiscoverableToolAction::Install {
            return Err(FunctionCallError::RespondToModel(
                "tool suggestions currently support only action_type=\"install\"".to_string(),
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
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "tool suggestions are unavailable right now: {err}"
            ))
        })?;

        let tool = discoverable_tools
            .into_iter()
            .find(|tool| tool.tool_type() == args.tool_type && tool.id() == args.tool_id)
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
            &tool,
        );
        let response = session
            .request_mcp_server_elicitation(turn.as_ref(), request_id, params)
            .await;
        let user_confirmed = response
            .as_ref()
            .is_some_and(|response| response.action == ElicitationAction::Accept);

        let completed = if user_confirmed {
            verify_tool_suggestion_completed(&session, &turn, &tool, auth.as_ref()).await
        } else {
            false
        };

        if completed && let DiscoverableTool::Connector(connector) = &tool {
            session
                .merge_connector_selection(HashSet::from([connector.id.clone()]))
                .await;
        }

        let content = serde_json::to_string(&ToolSuggestResult {
            completed,
            user_confirmed,
            tool_type: args.tool_type,
            action_type: args.action_type,
            tool_id: tool.id().to_string(),
            tool_name: tool.name().to_string(),
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
    tool: &DiscoverableTool,
) -> McpServerElicitationRequestParams {
    let tool_name = tool.name().to_string();
    let install_url = tool.install_url().map(ToString::to_string);
    let message = suggest_reason.to_string();

    McpServerElicitationRequestParams {
        thread_id,
        turn_id: Some(turn_id),
        server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
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

async fn verify_tool_suggestion_completed(
    session: &crate::codex::Session,
    turn: &crate::codex::TurnContext,
    tool: &DiscoverableTool,
    auth: Option<&crate::CodexAuth>,
) -> bool {
    match tool {
        DiscoverableTool::Connector(connector) => {
            let manager = session.services.mcp_connection_manager.read().await;
            match manager.hard_refresh_codex_apps_tools_cache().await {
                Ok(mcp_tools) => {
                    let accessible_connectors = connectors::with_app_enabled_state(
                        connectors::accessible_connectors_from_mcp_tools(&mcp_tools),
                        &turn.config,
                    );
                    connectors::refresh_accessible_connectors_cache_from_mcp_tools(
                        &turn.config,
                        auth,
                        &mcp_tools,
                    );
                    verified_connector_suggestion_completed(
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
        }
        DiscoverableTool::Plugin(plugin) => {
            session.reload_user_config_layer().await;
            let config = session.get_config().await;
            verified_plugin_suggestion_completed(
                plugin.id.as_str(),
                config.as_ref(),
                session.services.plugins_manager.as_ref(),
            )
        }
    }
}

fn verified_connector_suggestion_completed(
    tool_id: &str,
    accessible_connectors: &[AppInfo],
) -> bool {
    accessible_connectors
        .iter()
        .find(|connector| connector.id == tool_id)
        .is_some_and(|connector| connector.is_accessible)
}

fn verified_plugin_suggestion_completed(
    tool_id: &str,
    config: &crate::config::Config,
    plugins_manager: &crate::plugins::PluginsManager,
) -> bool {
    plugins_manager
        .list_marketplaces_for_config(config, &[])
        .ok()
        .into_iter()
        .flatten()
        .flat_map(|marketplace| marketplace.plugins.into_iter())
        .any(|plugin| plugin.id == tool_id && plugin.installed)
}

#[cfg(test)]
#[path = "tool_suggest_tests.rs"]
mod tests;
