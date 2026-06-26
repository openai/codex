use codex_analytics::PluginInstallRequestSource;
use codex_analytics::PluginInstallRequested;
use codex_analytics::build_track_events_context;
use codex_config::types::ToolSuggestDisabledTool;
use codex_core_plugins::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_tools::DiscoverablePluginInfo;
use codex_tools::DiscoverableToolAction;
use codex_tools::DiscoverableToolType;
use codex_tools::LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME;
use codex_tools::REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE;
use codex_tools::REQUEST_PLUGIN_INSTALL_PERSIST_KEY;
use codex_tools::REQUEST_PLUGIN_INSTALL_TOOL_NAME;
use codex_tools::RequestPluginInstallArgs;
use codex_tools::RequestPluginInstallResult;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_tools::build_request_plugin_install_elicitation_request;
use rmcp::model::RequestId;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tracing::warn;

use crate::config::edit::ConfigEdit;
use crate::config::edit::ConfigEditsBuilder;
use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::request_plugin_install_spec::create_request_plugin_install_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use crate::tools::router::ToolSuggestPresentation;

const PLUGIN_INSTALL_ELICITATION_SERVER_NAME: &str = "plugin_installer";

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct RecommendedPluginInstallArgs {
    #[serde(alias = "tool_id")]
    plugin_id: String,
    suggest_reason: String,
}

pub struct RequestPluginInstallHandler {
    plugins: Vec<DiscoverablePluginInfo>,
    presentation: ToolSuggestPresentation,
}

impl RequestPluginInstallHandler {
    pub(crate) fn new(
        plugins: Vec<DiscoverablePluginInfo>,
        presentation: ToolSuggestPresentation,
    ) -> Self {
        Self {
            plugins,
            presentation,
        }
    }
}

impl ToolExecutor<ToolInvocation> for RequestPluginInstallHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(REQUEST_PLUGIN_INSTALL_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        create_request_plugin_install_tool(self.presentation)
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(invocation))
    }
}

impl RequestPluginInstallHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            payload,
            session,
            step_context,
            call_id,
            ..
        } = invocation;
        let turn = Arc::clone(&step_context.turn);
        let manager = step_context.mcp.manager();

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::Fatal(format!(
                    "{REQUEST_PLUGIN_INSTALL_TOOL_NAME} handler received unsupported payload"
                )));
            }
        };

        let (requested_plugin_id, suggest_reason) = match self.presentation {
            ToolSuggestPresentation::ListTool => {
                let args: RequestPluginInstallArgs = parse_arguments(&arguments)?;
                (args.tool_id, args.suggest_reason)
            }
            ToolSuggestPresentation::RecommendationContext => {
                let args: RecommendedPluginInstallArgs = parse_arguments(&arguments)?;
                (args.plugin_id, args.suggest_reason)
            }
        };
        let suggest_reason = suggest_reason.trim();
        if suggest_reason.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "suggest_reason must not be empty".to_string(),
            ));
        }

        let plugin = self
            .plugins
            .iter()
            .find(|plugin| plugin.id == requested_plugin_id)
            .ok_or_else(|| {
                let (argument_name, source) = match self.presentation {
                    ToolSuggestPresentation::ListTool => (
                        "tool_id",
                        format!(
                            "the plugins returned by {LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME}"
                        ),
                    ),
                    ToolSuggestPresentation::RecommendationContext => (
                        "plugin_id",
                        "the entries in the <recommended_plugins> list".to_string(),
                    ),
                };
                FunctionCallError::RespondToModel(format!(
                    "{argument_name} must match one of {source}"
                ))
            })?;

        let suggestion_id = format!("request_plugin_install_{call_id}");
        let source = match self.presentation {
            ToolSuggestPresentation::ListTool => PluginInstallRequestSource::LegacyDiscovery,
            ToolSuggestPresentation::RecommendationContext => {
                PluginInstallRequestSource::EndpointRecommendation
            }
        };
        session
            .services
            .analytics_events_client
            .track_plugin_install_requested(
                build_track_events_context(
                    turn.model_info.slug.clone(),
                    session.thread_id.to_string(),
                    turn.sub_id.clone(),
                    turn.originator.clone(),
                ),
                PluginInstallRequested {
                    suggestion_id: suggestion_id.clone(),
                    plugins: vec![codex_core_plugins::plugin_install_requested_metadata(
                        plugin,
                    )],
                    source,
                },
            );

        let request_id = RequestId::String(suggestion_id.into());
        let request = build_request_plugin_install_elicitation_request(suggest_reason, plugin);
        let elicitation = session
            .request_mcp_server_elicitation(
                turn.as_ref(),
                PLUGIN_INSTALL_ELICITATION_SERVER_NAME.to_string(),
                request_id,
                request,
            )
            .await;
        let response = elicitation.response;
        if let Some(response) = response.as_ref() {
            maybe_persist_disabled_install_request(&session, &turn, plugin, response).await;
        }
        let user_confirmed = response
            .as_ref()
            .is_some_and(|response| response.action == ElicitationAction::Accept);

        let auth = session.services.auth_manager.auth().await;
        let completed = if user_confirmed {
            verify_plugin_install_completed(&session, &turn, manager, plugin, auth.as_ref()).await
        } else {
            false
        };

        if elicitation.sent {
            let response_action = match response.as_ref().map(|response| &response.action) {
                Some(ElicitationAction::Accept) => "accept",
                Some(ElicitationAction::Decline) => "decline",
                Some(ElicitationAction::Cancel) => "cancel",
                None => "unavailable",
            };
            turn.session_telemetry.record_plugin_install_suggestion(
                "plugin",
                &plugin.id,
                &plugin.name,
                response_action,
                user_confirmed,
                completed,
            );
        }

        let content = serde_json::to_string(&RequestPluginInstallResult {
            completed,
            user_confirmed,
            tool_type: DiscoverableToolType::Plugin,
            action_type: DiscoverableToolAction::Install,
            tool_id: plugin.id.clone(),
            tool_name: plugin.name.clone(),
            suggest_reason: suggest_reason.to_string(),
        })
        .map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize {REQUEST_PLUGIN_INSTALL_TOOL_NAME} response: {err}"
            ))
        })?;

        Ok(boxed_tool_output(FunctionToolOutput::from_text(
            content,
            Some(true),
        )))
    }
}

impl CoreToolRuntime for RequestPluginInstallHandler {}

async fn maybe_persist_disabled_install_request(
    session: &crate::session::session::Session,
    turn: &crate::session::turn_context::TurnContext,
    plugin: &DiscoverablePluginInfo,
    response: &ElicitationResponse,
) {
    if !request_plugin_install_response_requests_persistent_disable(response) {
        return;
    }

    if let Err(err) = persist_disabled_install_request(&turn.config.codex_home, plugin).await {
        warn!(
            error = %err,
            plugin_id = %plugin.id,
            "failed to persist disabled plugin suggestion"
        );
        return;
    }

    session.reload_user_config_layer().await;
}

fn request_plugin_install_response_requests_persistent_disable(
    response: &ElicitationResponse,
) -> bool {
    if response.action != ElicitationAction::Decline {
        return false;
    }

    response
        .meta
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|meta| meta.get(REQUEST_PLUGIN_INSTALL_PERSIST_KEY))
        .and_then(Value::as_str)
        == Some(REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE)
}

async fn persist_disabled_install_request(
    codex_home: &codex_utils_absolute_path::AbsolutePathBuf,
    plugin: &DiscoverablePluginInfo,
) -> anyhow::Result<()> {
    ConfigEditsBuilder::new(codex_home)
        .with_edits([ConfigEdit::AddToolSuggestDisabledTool(
            ToolSuggestDisabledTool::plugin(&plugin.id),
        )])
        .apply()
        .await
}

async fn verify_plugin_install_completed(
    session: &crate::session::session::Session,
    turn: &crate::session::turn_context::TurnContext,
    manager: &codex_mcp::McpConnectionManager,
    plugin: &DiscoverablePluginInfo,
    auth: Option<&codex_login::CodexAuth>,
) -> bool {
    let remote = is_remote_plugin_install_suggestion(&plugin.id);
    let base_completed = if remote {
        Some(
            refresh_remote_installed_plugins_cache_after_install(
                session,
                turn,
                auth,
                plugin.id.as_str(),
            )
            .await,
        )
    } else {
        session.reload_user_config_layer().await;
        None
    };

    let config = session.get_config().await;
    refresh_runtime_mcp_servers(session, turn, manager).await;
    let base_completed = base_completed.unwrap_or_else(|| {
        verified_plugin_install_completed(
            plugin.id.as_str(),
            config.as_ref(),
            session.services.plugins_manager.as_ref(),
        )
    });
    let extension_completed = session
        .services
        .extensions
        .verify_plugin_install(codex_extension_api::PluginInstallVerificationContext::new(
            plugin,
            config.as_ref(),
        ))
        .await;
    plugin_install_completed_with_extensions(base_completed, extension_completed)
}

fn plugin_install_completed_with_extensions(
    base_completed: bool,
    extension_completed: Option<bool>,
) -> bool {
    base_completed && extension_completed.unwrap_or(true)
}

async fn refresh_remote_installed_plugins_cache_after_install(
    session: &crate::session::session::Session,
    turn: &crate::session::turn_context::TurnContext,
    auth: Option<&codex_login::CodexAuth>,
    plugin_id: &str,
) -> bool {
    let plugins_manager = &session.services.plugins_manager;
    let plugins_config = turn.config.plugins_config_input();
    match plugins_manager
        .build_and_cache_remote_installed_plugin_marketplaces(
            &plugins_config,
            auth,
            &[REMOTE_GLOBAL_MARKETPLACE_NAME],
            /*on_effective_plugins_changed*/ None,
        )
        .await
    {
        Ok(marketplaces) => marketplaces.into_iter().any(|marketplace| {
            marketplace
                .plugins
                .into_iter()
                .any(|plugin| plugin.id == plugin_id && plugin.installed)
        }),
        Err(err) => {
            warn!(
                "failed to refresh remote installed plugins cache after plugin install request for {plugin_id}: {err:#}"
            );
            false
        }
    }
}

fn is_remote_plugin_install_suggestion(plugin_id: &str) -> bool {
    plugin_id
        .rsplit_once('@')
        .is_some_and(|(_, marketplace_name)| marketplace_name == REMOTE_GLOBAL_MARKETPLACE_NAME)
}

async fn refresh_runtime_mcp_servers(
    session: &crate::session::session::Session,
    turn: &crate::session::turn_context::TurnContext,
    manager: &codex_mcp::McpConnectionManager,
) {
    let elicitation_reviewer = manager.elicitation_reviewer();
    session
        .refresh_mcp_servers_now_from_current_config(turn, elicitation_reviewer)
        .await;
}

fn verified_plugin_install_completed(
    tool_id: &str,
    config: &crate::config::Config,
    plugins_manager: &codex_core_plugins::PluginsManager,
) -> bool {
    let plugins_input = config.plugins_config_input();
    plugins_manager
        .list_marketplaces_for_config(&plugins_input, &[], /*include_openai_curated*/ true)
        .ok()
        .into_iter()
        .flat_map(|outcome| outcome.marketplaces)
        .flat_map(|marketplace| marketplace.plugins.into_iter())
        .any(|plugin| plugin.id == tool_id && plugin.installed)
}

#[cfg(test)]
#[path = "request_plugin_install_tests.rs"]
mod tests;
