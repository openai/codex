use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Weak;

use codex_app_server_protocol::AppInfo;
use codex_config::types::ToolSuggestDisabledTool;
use codex_core_plugins::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_tools::DiscoverableTool;
use codex_tools::DiscoverableToolType;
use rmcp::model::RequestId;
use serde::Deserialize;
use serde_json::Value;
use tracing::warn;

use crate::config::edit::ConfigEdit;
use crate::config::edit::ConfigEditsBuilder;
use crate::connectors;
use crate::function_tool::FunctionCallError;
use codex_plugin_installs_extension::REQUEST_PLUGIN_INSTALL_PERSIST_ALWAYS_VALUE;
use codex_plugin_installs_extension::REQUEST_PLUGIN_INSTALL_PERSIST_KEY;
use codex_plugin_installs_extension::REQUEST_PLUGIN_INSTALLS_TOOL_NAME;
use codex_plugin_installs_extension::RequestPluginInstallEntryResult;
use codex_plugin_installs_extension::RequestPluginInstallInstalledEntry;
use codex_plugin_installs_extension::RequestPluginInstallResolvedPickerEntry;
use codex_plugin_installs_extension::RequestPluginInstallsBackend;
use codex_plugin_installs_extension::RequestPluginInstallsBackendFuture;
use codex_plugin_installs_extension::RequestPluginInstallsRequest;
use codex_plugin_installs_extension::RequestPluginInstallsResult;
use codex_plugin_installs_extension::all_requested_connectors_picked_up;
use codex_plugin_installs_extension::build_request_plugin_installs_elicitation_request;
use codex_plugin_installs_extension::request_plugin_install_picker_completed;
use codex_plugin_installs_extension::verified_connector_install_completed;

pub(crate) struct CoreRequestPluginInstallsBackend {
    session: Weak<crate::session::session::Session>,
}

impl CoreRequestPluginInstallsBackend {
    pub(crate) fn new(session: &Arc<crate::session::session::Session>) -> Self {
        Self {
            session: Arc::downgrade(session),
        }
    }

    async fn handle_request(
        &self,
        request: RequestPluginInstallsRequest,
    ) -> Result<RequestPluginInstallsResult, FunctionCallError> {
        let RequestPluginInstallsRequest {
            call_id,
            turn_id,
            args,
            resolved_entries,
        } = request;
        let session = self.session.upgrade().ok_or_else(|| {
            FunctionCallError::Fatal("plugin install session is no longer available".to_string())
        })?;
        let turn = session
            .turn_context_for_sub_id(&turn_id)
            .await
            .ok_or_else(|| {
                FunctionCallError::Fatal("plugin install turn is no longer active".to_string())
            })?;
        let action_type = args.action_type;
        let requested_entries = requested_picker_install_entries(&resolved_entries);

        let request_id = RequestId::String(format!("request_plugin_installs_{call_id}").into());
        let params = build_request_plugin_installs_elicitation_request(
            CODEX_APPS_MCP_SERVER_NAME,
            session.thread_id.to_string(),
            turn.sub_id.clone(),
            &args,
            &resolved_entries,
        );
        drop(resolved_entries);

        let elicitation = session
            .request_mcp_server_elicitation(turn.as_ref(), request_id, params)
            .await;
        let response = elicitation.response;
        if let Some(response) = response.as_ref() {
            maybe_persist_disabled_install_requests(&session, &turn, &requested_entries, response)
                .await;
        }
        let user_confirmed = response
            .as_ref()
            .is_some_and(|response| response.action == ElicitationAction::Accept);
        let response_installed_entries =
            request_plugin_install_picker_response_entries(response.as_ref());

        let auth = session.services.auth_manager.auth().await;
        let entries = if user_confirmed {
            verify_request_plugin_install_picker_completed(
                &session,
                &turn,
                &requested_entries,
                &response_installed_entries,
                auth.as_ref(),
            )
            .await
        } else {
            requested_entries
                .iter()
                .map(|entry| entry.result(/*completed*/ false))
                .collect()
        };
        let completed_connector_ids = requested_entries
            .iter()
            .zip(entries.iter())
            .filter_map(|(requested_entry, entry)| {
                if !entry.completed {
                    return None;
                }
                match &requested_entry.tool {
                    DiscoverableTool::Connector(connector) => Some(connector.id.clone()),
                    DiscoverableTool::Plugin(_) => None,
                }
            })
            .collect::<HashSet<_>>();
        if !completed_connector_ids.is_empty() {
            session
                .merge_connector_selection(completed_connector_ids)
                .await;
        }

        if elicitation.sent {
            let response_action = match response.as_ref().map(|response| &response.action) {
                Some(ElicitationAction::Accept) => "accept",
                Some(ElicitationAction::Decline) => "decline",
                Some(ElicitationAction::Cancel) => "cancel",
                None => "unavailable",
            };
            for entry in &entries {
                turn.session_telemetry.record_plugin_install_suggestion(
                    tool_type_str(entry.tool_type),
                    entry.tool_id.as_str(),
                    entry.tool_name.as_str(),
                    response_action,
                    user_confirmed,
                    entry.completed,
                );
            }
        }

        let completed = user_confirmed && request_plugin_install_picker_completed(&entries);
        Ok(RequestPluginInstallsResult {
            completed,
            user_confirmed,
            action_type,
            entries,
        })
    }
}

impl RequestPluginInstallsBackend for CoreRequestPluginInstallsBackend {
    fn execute(
        &self,
        request: RequestPluginInstallsRequest,
    ) -> RequestPluginInstallsBackendFuture<'_> {
        Box::pin(self.handle_request(request))
    }
}

#[derive(Clone)]
struct RequestedPickerInstallEntry {
    tool: DiscoverableTool,
}

impl RequestedPickerInstallEntry {
    fn result(&self, completed: bool) -> RequestPluginInstallEntryResult {
        RequestPluginInstallEntryResult {
            tool_type: self.tool.tool_type(),
            tool_id: self.tool.id().to_string(),
            tool_name: self.tool.name().to_string(),
            completed,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RequestPluginInstallPickerResponseContent {
    #[serde(default)]
    installed_entries: Vec<RequestPluginInstallInstalledEntry>,
}

fn requested_picker_install_entries(
    resolved_entries: &[RequestPluginInstallResolvedPickerEntry],
) -> Vec<RequestedPickerInstallEntry> {
    resolved_entries
        .iter()
        .map(|entry| RequestedPickerInstallEntry {
            tool: entry.tool.clone(),
        })
        .collect()
}

fn request_plugin_install_picker_response_entries(
    response: Option<&ElicitationResponse>,
) -> Vec<RequestPluginInstallInstalledEntry> {
    let Some(content) = response.and_then(|response| response.content.as_ref()) else {
        return Vec::new();
    };

    match serde_json::from_value::<RequestPluginInstallPickerResponseContent>(content.clone()) {
        Ok(content) => content.installed_entries,
        Err(err) => {
            warn!("failed to parse request_plugin_installs picker response content: {err:#}");
            Vec::new()
        }
    }
}

async fn maybe_persist_disabled_install_requests(
    session: &crate::session::session::Session,
    turn: &crate::session::turn_context::TurnContext,
    requested_entries: &[RequestedPickerInstallEntry],
    response: &ElicitationResponse,
) {
    if !request_plugin_install_response_requests_persistent_disable(response) {
        return;
    }

    for entry in requested_entries {
        if let Err(err) =
            persist_disabled_install_request(&turn.config.codex_home, &entry.tool).await
        {
            warn!(
                error = %err,
                tool_id = entry.tool.id(),
                "failed to persist disabled tool suggestion"
            );
            return;
        }
    }

    session.reload_user_config_layer().await;
}

pub(super) fn request_plugin_install_response_requests_persistent_disable(
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

pub(super) async fn persist_disabled_install_request(
    codex_home: &codex_utils_absolute_path::AbsolutePathBuf,
    tool: &DiscoverableTool,
) -> anyhow::Result<()> {
    ConfigEditsBuilder::new(codex_home)
        .with_edits([ConfigEdit::AddToolSuggestDisabledTool(
            disabled_install_request(tool),
        )])
        .apply()
        .await
}

fn disabled_install_request(tool: &DiscoverableTool) -> ToolSuggestDisabledTool {
    match tool {
        DiscoverableTool::Connector(connector) => {
            ToolSuggestDisabledTool::connector(connector.id.as_str())
        }
        DiscoverableTool::Plugin(plugin) => ToolSuggestDisabledTool::plugin(plugin.id.as_str()),
    }
}

async fn verify_request_plugin_install_picker_completed(
    session: &crate::session::session::Session,
    turn: &crate::session::turn_context::TurnContext,
    requested_entries: &[RequestedPickerInstallEntry],
    response_installed_entries: &[RequestPluginInstallInstalledEntry],
    auth: Option<&codex_login::CodexAuth>,
) -> Vec<RequestPluginInstallEntryResult> {
    let mut expected_connector_ids = HashSet::new();
    let mut has_local_plugin_entry = false;
    for entry in requested_entries {
        match &entry.tool {
            DiscoverableTool::Connector(connector) => {
                expected_connector_ids.insert(connector.id.clone());
            }
            DiscoverableTool::Plugin(plugin) => {
                expected_connector_ids.extend(plugin.app_connector_ids.iter().cloned());
                if !is_remote_plugin_install_suggestion(&plugin.id) {
                    has_local_plugin_entry = true;
                }
            }
        }
    }
    let expected_connector_ids = expected_connector_ids.into_iter().collect::<Vec<_>>();
    let accessible_connectors = if expected_connector_ids.is_empty() {
        Some(Vec::new())
    } else {
        refresh_missing_requested_connectors(
            session,
            turn,
            auth,
            &expected_connector_ids,
            REQUEST_PLUGIN_INSTALLS_TOOL_NAME,
        )
        .await
    };

    let config = if has_local_plugin_entry {
        session.reload_user_config_layer().await;
        Some(session.get_config().await)
    } else {
        None
    };

    requested_entries
        .iter()
        .map(|entry| {
            let app_reported_completed =
                response_reports_picker_entry_completed(response_installed_entries, entry);
            let locally_verified_completed = match &entry.tool {
                DiscoverableTool::Connector(connector) => accessible_connectors
                    .as_ref()
                    .is_some_and(|accessible_connectors| {
                        verified_connector_install_completed(
                            connector.id.as_str(),
                            accessible_connectors,
                        )
                    }),
                DiscoverableTool::Plugin(plugin) => {
                    if is_remote_plugin_install_suggestion(&plugin.id) {
                        false
                    } else {
                        config.as_ref().is_some_and(|config| {
                            verified_plugin_install_completed(
                                plugin.id.as_str(),
                                config.as_ref(),
                                session.services.plugins_manager.as_ref(),
                            )
                        })
                    }
                }
            };
            entry.result(app_reported_completed || locally_verified_completed)
        })
        .collect()
}

fn response_reports_picker_entry_completed(
    response_installed_entries: &[RequestPluginInstallInstalledEntry],
    requested_entry: &RequestedPickerInstallEntry,
) -> bool {
    match &requested_entry.tool {
        DiscoverableTool::Connector(_) => false,
        DiscoverableTool::Plugin(_) => response_installed_entries.iter().any(|installed_entry| {
            installed_entry.tool_id == requested_entry.tool.id()
                && installed_entry.tool_type == requested_entry.tool.tool_type()
        }),
    }
}

pub(super) fn tool_type_str(tool_type: DiscoverableToolType) -> &'static str {
    match tool_type {
        DiscoverableToolType::Connector => "connector",
        DiscoverableToolType::Plugin => "plugin",
    }
}

pub(super) fn is_remote_plugin_install_suggestion(plugin_id: &str) -> bool {
    plugin_id
        .rsplit_once('@')
        .is_some_and(|(_, marketplace_name)| marketplace_name == REMOTE_GLOBAL_MARKETPLACE_NAME)
}

pub(super) async fn refresh_missing_requested_connectors(
    session: &crate::session::session::Session,
    turn: &crate::session::turn_context::TurnContext,
    auth: Option<&codex_login::CodexAuth>,
    expected_connector_ids: &[String],
    tool_id: &str,
) -> Option<Vec<AppInfo>> {
    if expected_connector_ids.is_empty() {
        return Some(Vec::new());
    }

    let manager = session.services.mcp_connection_manager.load_full();
    let mcp_tools = manager.list_all_tools().await;
    let accessible_connectors = connectors::with_app_enabled_state(
        connectors::accessible_connectors_from_mcp_tools(&mcp_tools),
        &turn.config,
    );
    if all_requested_connectors_picked_up(expected_connector_ids, &accessible_connectors) {
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
                "failed to refresh codex apps tools cache after plugin install request for {tool_id}: {err:#}"
            );
            None
        }
    }
}

pub(super) fn verified_plugin_install_completed(
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
#[path = "request_plugin_installs_tests.rs"]
mod tests;
