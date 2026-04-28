use std::collections::HashSet;

use codex_app_server_protocol::AppInfo;
use codex_config::types::ToolSuggestDisabledTool;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_tools::DiscoverableTool;
use codex_tools::DiscoverableToolAction;
use codex_tools::DiscoverableToolType;
use codex_tools::TOOL_SUGGEST_PERSIST_ALWAYS_VALUE;
use codex_tools::TOOL_SUGGEST_PERSIST_KEY;
use codex_tools::TOOL_SUGGEST_TOOL_NAME;
use codex_tools::ToolSuggestArgs;
use codex_tools::ToolSuggestResult;
use codex_tools::all_suggested_connectors_picked_up;
use codex_tools::build_tool_suggestion_elicitation_request;
use codex_tools::filter_tool_suggest_discoverable_tools_for_client;
use codex_tools::verified_connector_suggestion_completed;
use rmcp::model::RequestId;
use serde_json::Value;
use tracing::warn;

use crate::config::edit::ConfigEdit;
use crate::config::edit::ConfigEditsBuilder;
use crate::connectors;
use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct ToolSuggestHandler;

impl ToolHandler for ToolSuggestHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "tool suggestion discovery reads through the session-owned manager guard"
    )]
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
        if args.tool_type == DiscoverableToolType::Plugin
            && turn.app_server_client_name.as_deref() == Some("codex-tui")
        {
            return Err(FunctionCallError::RespondToModel(
                "plugin tool suggestions are not available in codex-tui yet".to_string(),
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
        .map(|discoverable_tools| {
            filter_tool_suggest_discoverable_tools_for_client(
                discoverable_tools,
                turn.app_server_client_name.as_deref(),
            )
        })
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
            CODEX_APPS_MCP_SERVER_NAME,
            session.conversation_id.to_string(),
            turn.sub_id.clone(),
            &args,
            suggest_reason,
            &tool,
        );
        let response = session
            .request_mcp_server_elicitation(turn.as_ref(), request_id, params)
            .await;
        if let Some(response) = response.as_ref() {
            maybe_persist_tool_suggest_disable(&session, &turn, &tool, response).await;
        }
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

async fn maybe_persist_tool_suggest_disable(
    session: &crate::session::session::Session,
    turn: &crate::session::turn_context::TurnContext,
    tool: &DiscoverableTool,
    response: &ElicitationResponse,
) {
    if !tool_suggest_response_requests_persistent_disable(response) {
        return;
    }

    if let Err(err) = persist_tool_suggest_disable(&turn.config.codex_home, tool).await {
        warn!(
            error = %err,
            tool_id = tool.id(),
            "failed to persist disabled tool suggestion"
        );
        return;
    }

    session.reload_user_config_layer().await;
}

fn tool_suggest_response_requests_persistent_disable(response: &ElicitationResponse) -> bool {
    if response.action != ElicitationAction::Decline {
        return false;
    }

    response
        .meta
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|meta| meta.get(TOOL_SUGGEST_PERSIST_KEY))
        .and_then(Value::as_str)
        == Some(TOOL_SUGGEST_PERSIST_ALWAYS_VALUE)
}

async fn persist_tool_suggest_disable(
    codex_home: &codex_utils_absolute_path::AbsolutePathBuf,
    tool: &DiscoverableTool,
) -> anyhow::Result<()> {
    ConfigEditsBuilder::new(codex_home)
        .with_edits([ConfigEdit::AddToolSuggestDisabledTool(
            disabled_tool_suggestion(tool),
        )])
        .apply()
        .await
}

fn disabled_tool_suggestion(tool: &DiscoverableTool) -> ToolSuggestDisabledTool {
    match tool {
        DiscoverableTool::Connector(connector) => {
            ToolSuggestDisabledTool::connector(connector.id.as_str())
        }
        DiscoverableTool::Plugin(plugin) => ToolSuggestDisabledTool::plugin(plugin.id.as_str()),
    }
}

async fn verify_tool_suggestion_completed(
    session: &crate::session::session::Session,
    turn: &crate::session::turn_context::TurnContext,
    tool: &DiscoverableTool,
    auth: Option<&codex_login::CodexAuth>,
) -> bool {
    match tool {
        DiscoverableTool::Connector(connector) => refresh_missing_suggested_connectors(
            session,
            turn,
            auth,
            std::slice::from_ref(&connector.id),
            connector.id.as_str(),
        )
        .await
        .is_some_and(|accessible_connectors| {
            verified_connector_suggestion_completed(connector.id.as_str(), &accessible_connectors)
        }),
        DiscoverableTool::Plugin(plugin) => {
            session.reload_user_config_layer().await;
            let config = session.get_config().await;
            let completed = verified_plugin_suggestion_completed(
                plugin.id.as_str(),
                config.as_ref(),
                session.services.plugins_manager.as_ref(),
            );
            let _ = refresh_missing_suggested_connectors(
                session,
                turn,
                auth,
                &plugin.app_connector_ids,
                plugin.id.as_str(),
            )
            .await;
            completed
        }
    }
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "connector cache refresh reads through the session-owned manager guard"
)]
async fn refresh_missing_suggested_connectors(
    session: &crate::session::session::Session,
    turn: &crate::session::turn_context::TurnContext,
    auth: Option<&codex_login::CodexAuth>,
    expected_connector_ids: &[String],
    tool_id: &str,
) -> Option<Vec<AppInfo>> {
    if expected_connector_ids.is_empty() {
        return Some(Vec::new());
    }

    let manager = session.services.mcp_connection_manager.read().await;
    let mcp_tools = manager.list_all_tools().await;
    let accessible_connectors = connectors::with_app_enabled_state(
        connectors::accessible_connectors_from_mcp_tools(&mcp_tools),
        &turn.config,
    );
    if all_suggested_connectors_picked_up(expected_connector_ids, &accessible_connectors) {
        return Some(accessible_connectors);
    }

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
            Some(accessible_connectors)
        }
        Err(err) => {
            warn!(
                "failed to refresh codex apps tools cache after tool suggestion for {tool_id}: {err:#}"
            );
            None
        }
    }
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
        .flat_map(|outcome| outcome.marketplaces)
        .flat_map(|marketplace| marketplace.plugins.into_iter())
        .any(|plugin| plugin.id == tool_id && plugin.installed)
}

#[cfg(test)]
#[path = "tool_suggest_tests.rs"]
mod tests;
