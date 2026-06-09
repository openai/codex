use super::*;
use crate::mcp::ResolvedMcpConnections;
use codex_mcp::ElicitationReviewRequest;
use codex_mcp::ElicitationReviewer;
use codex_mcp::ElicitationReviewerHandle;
use codex_mcp::McpConnectionStartParams;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::mcp_approval_meta::APPROVAL_KIND_KEY as MCP_ELICITATION_APPROVAL_KIND_KEY;
use codex_protocol::mcp_approval_meta::APPROVAL_KIND_MCP_TOOL_CALL as MCP_ELICITATION_APPROVAL_KIND_MCP_TOOL_CALL;
use codex_protocol::mcp_approval_meta::APPROVAL_KIND_TOOL_SUGGESTION as MCP_ELICITATION_APPROVAL_KIND_TOOL_SUGGESTION;
use codex_protocol::mcp_approval_meta::APPROVALS_REVIEWER_KEY as MCP_ELICITATION_APPROVALS_REVIEWER_KEY;
use codex_protocol::mcp_approval_meta::CONNECTOR_DESCRIPTION_KEY as MCP_ELICITATION_CONNECTOR_DESCRIPTION_KEY;
use codex_protocol::mcp_approval_meta::CONNECTOR_ID_KEY as MCP_ELICITATION_CONNECTOR_ID_KEY;
use codex_protocol::mcp_approval_meta::CONNECTOR_NAME_KEY as MCP_ELICITATION_CONNECTOR_NAME_KEY;
use codex_protocol::mcp_approval_meta::REQUEST_TYPE_APPROVAL_REQUEST as MCP_ELICITATION_REQUEST_TYPE_APPROVAL_REQUEST;
use codex_protocol::mcp_approval_meta::REQUEST_TYPE_KEY as MCP_ELICITATION_REQUEST_TYPE_KEY;
use codex_protocol::mcp_approval_meta::TOOL_DESCRIPTION_KEY as MCP_ELICITATION_TOOL_DESCRIPTION_KEY;
use codex_protocol::mcp_approval_meta::TOOL_NAME_KEY as MCP_ELICITATION_TOOL_NAME_KEY;
use codex_protocol::mcp_approval_meta::TOOL_PARAMS_KEY as MCP_ELICITATION_TOOL_PARAMS_KEY;
use codex_protocol::mcp_approval_meta::TOOL_TITLE_KEY as MCP_ELICITATION_TOOL_TITLE_KEY;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::ElicitationAction;
use rmcp::model::Meta;
use serde_json::Map;

const MCP_ELICITATION_DECLINE_MESSAGE_KEY: &str = "message";
const TOOL_SUGGESTION_ACTION_INSTALL: &str = "install";
const TOOL_SUGGESTION_ACTION_KEY: &str = "suggest_type";
const TOOL_SUGGESTION_TOOL_ID_KEY: &str = "tool_id";
const TOOL_SUGGESTION_TOOL_TYPE_KEY: &str = "tool_type";

#[derive(Debug, PartialEq)]
enum GuardianElicitationReview {
    NotRequested,
    Decline(&'static str),
    ApprovalRequest(Box<crate::guardian::GuardianApprovalRequest>),
}

struct GuardianMcpElicitationReviewer {
    session: std::sync::Weak<Session>,
}

pub(crate) struct McpServerElicitationOutcome {
    pub(crate) response: Option<ElicitationResponse>,
    pub(crate) sent: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct PluginInstallElicitationTelemetryMetadata {
    tool_type: String,
    tool_id: String,
    tool_name: String,
}

impl GuardianMcpElicitationReviewer {
    fn new(session: &Arc<Session>) -> Self {
        Self {
            session: Arc::downgrade(session),
        }
    }
}

impl ElicitationReviewer for GuardianMcpElicitationReviewer {
    fn review(
        &self,
        request: ElicitationReviewRequest,
    ) -> BoxFuture<'static, anyhow::Result<Option<ElicitationResponse>>> {
        let session = self.session.clone();
        Box::pin(async move {
            let Some(session) = session.upgrade() else {
                return Ok(None);
            };
            review_guardian_mcp_elicitation(session, request).await
        })
    }
}

impl Session {
    pub(crate) fn mcp_elicitation_reviewer(self: &Arc<Self>) -> ElicitationReviewerHandle {
        Arc::new(GuardianMcpElicitationReviewer::new(self))
    }

    pub(crate) async fn initialize_mcp_connections(
        &self,
        resolved: ResolvedMcpConnections,
        auth: Option<CodexAuth>,
        session_configuration: &SessionConfiguration,
        runtime_context: McpRuntimeContext,
        codex_home: PathBuf,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) -> anyhow::Result<()> {
        let mut required_servers = resolved
            .servers
            .iter()
            .filter(|(_, server)| server.enabled() && server.required())
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        required_servers.sort();
        let required_server_count = required_servers.len();
        let params = mcp_connection_start_params(
            resolved,
            auth,
            session_configuration.approval_policy.value(),
            INITIAL_SUBMIT_ID.to_owned(),
            self.get_tx_event(),
            session_configuration.permission_profile(),
            runtime_context,
            codex_home,
            elicitation_reviewer,
        );
        self.services
            .mcp_connection_manager
            .initialize(params)
            .await;

        if required_servers.is_empty() {
            return Ok(());
        }
        let failures = self
            .services
            .mcp_connection_manager
            .required_startup_failures(&required_servers)
            .instrument(info_span!(
                "session_init.required_mcp_wait",
                otel.name = "session_init.required_mcp_wait",
                session_init.required_mcp_server_count = required_server_count,
            ))
            .await;
        if failures.is_empty() {
            return Ok(());
        }
        let details = failures
            .iter()
            .map(|failure| format!("{}: {}", failure.server, failure.error))
            .collect::<Vec<_>>()
            .join("; ");
        anyhow::bail!("required MCP servers failed to initialize: {details}")
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and turn state updates must remain atomic"
    )]
    pub async fn request_mcp_server_elicitation(
        &self,
        turn_context: &TurnContext,
        request_id: RequestId,
        params: McpServerElicitationRequestParams,
    ) -> McpServerElicitationOutcome {
        if self
            .services
            .mcp_connection_manager
            .elicitations_auto_deny()
        {
            return McpServerElicitationOutcome {
                response: Some(ElicitationResponse {
                    action: codex_rmcp_client::ElicitationAction::Accept,
                    content: Some(serde_json::json!({})),
                    meta: None,
                }),
                sent: false,
            };
        }

        let server_name = params.server_name.clone();
        let request = match params.request {
            McpServerElicitationRequest::Form {
                meta,
                message,
                requested_schema,
            } => {
                let requested_schema = match serde_json::to_value(requested_schema) {
                    Ok(requested_schema) => requested_schema,
                    Err(err) => {
                        warn!(
                            "failed to serialize MCP elicitation schema for server_name: {server_name}, request_id: {request_id}: {err:#}"
                        );
                        return McpServerElicitationOutcome {
                            response: None,
                            sent: false,
                        };
                    }
                };
                codex_protocol::approvals::ElicitationRequest::Form {
                    meta,
                    message,
                    requested_schema,
                }
            }
            McpServerElicitationRequest::Url {
                meta,
                message,
                url,
                elicitation_id,
            } => codex_protocol::approvals::ElicitationRequest::Url {
                meta,
                message,
                url,
                elicitation_id,
            },
        };

        let (tx_response, rx_response) = oneshot::channel();
        let prev_entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.insert_pending_elicitation(
                        server_name.clone(),
                        request_id.clone(),
                        tx_response,
                    )
                }
                None => None,
            }
        };
        if prev_entry.is_some() {
            warn!(
                "Overwriting existing pending elicitation for server_name: {server_name}, request_id: {request_id}"
            );
        }
        let id = match request_id {
            rmcp::model::NumberOrString::String(value) => {
                codex_protocol::mcp::RequestId::String(value.to_string())
            }
            rmcp::model::NumberOrString::Number(value) => {
                codex_protocol::mcp::RequestId::Integer(value)
            }
        };
        let event = EventMsg::ElicitationRequest(ElicitationRequestEvent {
            turn_id: params.turn_id,
            server_name,
            id,
            request,
        });
        let plugin_install_telemetry = plugin_install_elicitation_telemetry_metadata(&event);
        turn_context
            .turn_metadata_state
            .mark_user_input_requested_during_turn();
        self.send_event(turn_context, event).await;
        if let Some(plugin_install_telemetry) = plugin_install_telemetry {
            turn_context
                .session_telemetry
                .record_plugin_install_elicitation_sent(
                    plugin_install_telemetry.tool_type.as_str(),
                    plugin_install_telemetry.tool_id.as_str(),
                    plugin_install_telemetry.tool_name.as_str(),
                );
        }
        McpServerElicitationOutcome {
            response: rx_response.await.ok(),
            sent: true,
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and manager fallback must stay serialized"
    )]
    pub async fn resolve_elicitation(
        &self,
        server_name: String,
        id: RequestId,
        response: ElicitationResponse,
    ) -> anyhow::Result<()> {
        let entry = {
            let mut active = self.active_turn.lock().await;
            match active.as_mut() {
                Some(at) => {
                    let mut ts = at.turn_state.lock().await;
                    ts.remove_pending_elicitation(&server_name, &id)
                }
                None => None,
            }
        };
        if let Some(tx_response) = entry {
            tx_response
                .send(response)
                .map_err(|e| anyhow::anyhow!("failed to send elicitation response: {e:?}"))?;
            return Ok(());
        }

        self.services
            .mcp_connection_manager
            .resolve_elicitation(server_name, id, response)
            .await
    }

    pub async fn list_resources(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> anyhow::Result<ListResourcesResult> {
        self.services
            .mcp_connection_manager
            .list_resources(server, params)
            .await
    }

    pub async fn list_resource_templates(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> anyhow::Result<ListResourceTemplatesResult> {
        self.services
            .mcp_connection_manager
            .list_resource_templates(server, params)
            .await
    }

    pub async fn read_resource(
        &self,
        server: &str,
        params: ReadResourceRequestParams,
    ) -> anyhow::Result<ReadResourceResult> {
        self.services
            .mcp_connection_manager
            .read_resource(server, params)
            .await
    }

    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> anyhow::Result<CallToolResult> {
        self.services
            .mcp_connection_manager
            .call_tool(server, tool, arguments, meta)
            .await
    }

    async fn refresh_mcp_servers_inner(
        &self,
        turn_context: &TurnContext,
        mcp_servers: HashMap<String, McpServerConfig>,
        store_mode: OAuthCredentialsStoreMode,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) {
        let auth = self.services.auth_manager.auth().await;
        let config = self.get_config().await;
        let resolved = self
            .services
            .mcp_manager
            .resolve_refreshed_connections(config.as_ref(), mcp_servers, store_mode, auth.as_ref())
            .await;
        let mcp_runtime_context = match turn_context.environments.primary() {
            Some(turn_environment) => McpRuntimeContext::new(
                Arc::clone(&self.services.environment_manager),
                turn_environment.cwd.to_path_buf(),
            ),
            None => McpRuntimeContext::new(
                Arc::clone(&self.services.environment_manager),
                #[allow(deprecated)]
                turn_context.cwd.to_path_buf(),
            ),
        };
        let params = mcp_connection_start_params(
            resolved,
            auth,
            turn_context.approval_policy.value(),
            turn_context.sub_id.clone(),
            self.get_tx_event(),
            turn_context.permission_profile(),
            mcp_runtime_context,
            config.codex_home.to_path_buf(),
            elicitation_reviewer,
        );
        self.services
            .mcp_connection_manager
            .reconfigure(params)
            .await;
    }

    pub(crate) async fn refresh_mcp_servers_if_requested(
        &self,
        turn_context: &TurnContext,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) {
        let refresh_config = { self.pending_mcp_server_refresh_config.lock().await.take() };
        let Some(refresh_config) = refresh_config else {
            return;
        };

        let McpServerRefreshConfig {
            mcp_servers,
            mcp_oauth_credentials_store_mode,
        } = refresh_config;

        let mcp_servers =
            match serde_json::from_value::<HashMap<String, McpServerConfig>>(mcp_servers) {
                Ok(servers) => servers,
                Err(err) => {
                    warn!("failed to parse MCP server refresh config: {err}");
                    return;
                }
            };
        let store_mode = match serde_json::from_value::<OAuthCredentialsStoreMode>(
            mcp_oauth_credentials_store_mode,
        ) {
            Ok(mode) => mode,
            Err(err) => {
                warn!("failed to parse MCP OAuth refresh config: {err}");
                return;
            }
        };

        self.refresh_mcp_servers_inner(turn_context, mcp_servers, store_mode, elicitation_reviewer)
            .await;
    }

    pub(crate) async fn refresh_mcp_servers_now(
        &self,
        turn_context: &TurnContext,
        mcp_servers: HashMap<String, McpServerConfig>,
        store_mode: OAuthCredentialsStoreMode,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) {
        self.refresh_mcp_servers_inner(turn_context, mcp_servers, store_mode, elicitation_reviewer)
            .await;
    }

    pub(crate) async fn cancel_mcp_startup(&self) {
        self.services.mcp_connection_manager.cancel_startup();
    }
}

fn mcp_connection_start_params(
    resolved: ResolvedMcpConnections,
    auth: Option<CodexAuth>,
    approval_policy: AskForApproval,
    submit_id: String,
    tx_event: Sender<Event>,
    permission_profile: PermissionProfile,
    runtime_context: McpRuntimeContext,
    codex_home: PathBuf,
    elicitation_reviewer: Option<ElicitationReviewerHandle>,
) -> McpConnectionStartParams {
    McpConnectionStartParams {
        mcp_servers: resolved.servers,
        store_mode: resolved.store_mode,
        auth_entries: resolved.auth_statuses,
        approval_policy,
        submit_id,
        tx_event,
        permission_profile,
        runtime_context,
        codex_home,
        codex_apps_tools_cache_key: codex_apps_tools_cache_key(auth.as_ref()),
        host_owned_codex_apps_enabled: resolved.host_owned_codex_apps_enabled,
        prefix_mcp_tool_names: resolved.prefix_mcp_tool_names,
        client_elicitation_capability: resolved.client_elicitation_capability,
        tool_plugin_provenance: resolved.tool_plugin_provenance,
        auth,
        elicitation_reviewer,
    }
}

async fn review_guardian_mcp_elicitation(
    session: Arc<Session>,
    request: ElicitationReviewRequest,
) -> anyhow::Result<Option<ElicitationResponse>> {
    let Some((turn_context, _cancellation_token)) =
        session.active_turn_context_and_cancellation_token().await
    else {
        return Ok(None);
    };

    let approvals_reviewer = crate::connectors::mcp_approvals_reviewer(
        turn_context.config.as_ref(),
        request.server_name.as_str(),
        elicitation_connector_id(&request.elicitation),
    );
    if !crate::guardian::routes_approval_to_guardian_with_reviewer(
        turn_context.as_ref(),
        approvals_reviewer,
    ) {
        return Ok(None);
    }

    let guardian_request = match guardian_elicitation_review_request(&request) {
        GuardianElicitationReview::NotRequested => return Ok(None),
        GuardianElicitationReview::Decline(reason) => {
            warn!(
                server_name = %request.server_name,
                request_id = %mcp_elicitation_request_id(&request.request_id),
                reason,
                "declining Guardian MCP elicitation before review"
            );
            return Ok(Some(mcp_elicitation_decline_without_message()));
        }
        GuardianElicitationReview::ApprovalRequest(guardian_request) => *guardian_request,
    };

    let review_id = crate::guardian::new_guardian_review_id();
    let decision = crate::guardian::review_approval_request(
        &session,
        &turn_context,
        review_id.clone(),
        guardian_request,
        /*retry_reason*/ None,
    )
    .await;
    Ok(Some(
        mcp_elicitation_response_from_guardian_decision(session.as_ref(), &review_id, decision)
            .await,
    ))
}

fn guardian_elicitation_review_request(
    request: &ElicitationReviewRequest,
) -> GuardianElicitationReview {
    let (meta, requested_schema) = match &request.elicitation {
        CreateElicitationRequestParams::FormElicitationParams {
            meta,
            requested_schema,
            ..
        } => (meta, Some(requested_schema)),
        CreateElicitationRequestParams::UrlElicitationParams { meta, .. } => {
            return if meta_requests_approval_request(meta) {
                GuardianElicitationReview::Decline(
                    "guardian MCP elicitation review only supports form elicitations",
                )
            } else {
                GuardianElicitationReview::NotRequested
            };
        }
    };

    let Some(meta) = meta.as_ref().map(|meta| &meta.0) else {
        return GuardianElicitationReview::NotRequested;
    };
    if metadata_str(meta, MCP_ELICITATION_REQUEST_TYPE_KEY)
        != Some(MCP_ELICITATION_REQUEST_TYPE_APPROVAL_REQUEST)
    {
        return GuardianElicitationReview::NotRequested;
    }
    if metadata_str(meta, MCP_ELICITATION_APPROVAL_KIND_KEY)
        != Some(MCP_ELICITATION_APPROVAL_KIND_MCP_TOOL_CALL)
    {
        return GuardianElicitationReview::Decline(
            "guardian MCP elicitation metadata must declare mcp_tool_call approval kind",
        );
    }
    if requested_schema.is_some_and(|schema| !schema.properties.is_empty()) {
        return GuardianElicitationReview::Decline(
            "guardian MCP elicitation review only supports empty form schemas",
        );
    }

    let Some(tool_name) = metadata_owned_string(meta, MCP_ELICITATION_TOOL_NAME_KEY) else {
        return GuardianElicitationReview::Decline(
            "guardian MCP elicitation metadata must include a non-empty tool_name",
        );
    };
    let arguments = match meta.get(MCP_ELICITATION_TOOL_PARAMS_KEY) {
        Some(value @ Value::Object(_)) => Some(value.clone()),
        Some(_) => {
            return GuardianElicitationReview::Decline(
                "guardian MCP elicitation tool_params must be an object",
            );
        }
        None => Some(Value::Object(Map::new())),
    };

    GuardianElicitationReview::ApprovalRequest(Box::new(
        crate::guardian::GuardianApprovalRequest::McpToolCall {
            id: format!(
                "mcp_elicitation:{}:{}",
                request.server_name,
                mcp_elicitation_request_id(&request.request_id)
            ),
            server: request.server_name.clone(),
            tool_name,
            arguments,
            connector_id: metadata_owned_string(meta, MCP_ELICITATION_CONNECTOR_ID_KEY),
            connector_name: metadata_owned_string(meta, MCP_ELICITATION_CONNECTOR_NAME_KEY),
            connector_description: metadata_owned_string(
                meta,
                MCP_ELICITATION_CONNECTOR_DESCRIPTION_KEY,
            ),
            tool_title: metadata_owned_string(meta, MCP_ELICITATION_TOOL_TITLE_KEY),
            tool_description: metadata_owned_string(meta, MCP_ELICITATION_TOOL_DESCRIPTION_KEY),
            annotations: None,
        },
    ))
}

fn elicitation_connector_id(elicitation: &CreateElicitationRequestParams) -> Option<&str> {
    match elicitation {
        CreateElicitationRequestParams::FormElicitationParams { meta, .. }
        | CreateElicitationRequestParams::UrlElicitationParams { meta, .. } => meta
            .as_ref()
            .and_then(|meta| metadata_str(&meta.0, MCP_ELICITATION_CONNECTOR_ID_KEY)),
    }
}

fn meta_requests_approval_request(meta: &Option<Meta>) -> bool {
    meta.as_ref()
        .and_then(|meta| metadata_str(&meta.0, MCP_ELICITATION_REQUEST_TYPE_KEY))
        == Some(MCP_ELICITATION_REQUEST_TYPE_APPROVAL_REQUEST)
}

fn metadata_str<'a>(meta: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    meta.get(key).and_then(Value::as_str)
}

fn metadata_owned_string(meta: &Map<String, Value>, key: &str) -> Option<String> {
    metadata_str(meta, key)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn plugin_install_elicitation_telemetry_metadata(
    event: &EventMsg,
) -> Option<PluginInstallElicitationTelemetryMetadata> {
    let EventMsg::ElicitationRequest(ElicitationRequestEvent { request, .. }) = event else {
        return None;
    };
    let codex_protocol::approvals::ElicitationRequest::Form {
        meta: Some(Value::Object(meta)),
        ..
    } = request
    else {
        return None;
    };
    if metadata_str(meta, MCP_ELICITATION_APPROVAL_KIND_KEY)
        != Some(MCP_ELICITATION_APPROVAL_KIND_TOOL_SUGGESTION)
        || metadata_str(meta, TOOL_SUGGESTION_ACTION_KEY) != Some(TOOL_SUGGESTION_ACTION_INSTALL)
    {
        return None;
    }

    Some(PluginInstallElicitationTelemetryMetadata {
        tool_type: metadata_owned_string(meta, TOOL_SUGGESTION_TOOL_TYPE_KEY)?,
        tool_id: metadata_owned_string(meta, TOOL_SUGGESTION_TOOL_ID_KEY)?,
        tool_name: metadata_owned_string(meta, MCP_ELICITATION_TOOL_NAME_KEY)?,
    })
}

fn mcp_elicitation_request_id(id: &RequestId) -> String {
    match id {
        rmcp::model::NumberOrString::String(value) => value.to_string(),
        rmcp::model::NumberOrString::Number(value) => value.to_string(),
    }
}

async fn mcp_elicitation_response_from_guardian_decision(
    session: &Session,
    review_id: &str,
    decision: ReviewDecision,
) -> ElicitationResponse {
    let denial_message = match decision {
        ReviewDecision::Denied => {
            Some(crate::guardian::guardian_rejection_message(session, review_id).await)
        }
        _ => None,
    };
    mcp_elicitation_response_from_guardian_decision_parts(decision, denial_message)
}

fn mcp_elicitation_response_from_guardian_decision_parts(
    decision: ReviewDecision,
    denial_message: Option<String>,
) -> ElicitationResponse {
    match decision {
        ReviewDecision::Approved
        | ReviewDecision::ApprovedForSession
        | ReviewDecision::ApprovedExecpolicyAmendment { .. }
        | ReviewDecision::NetworkPolicyAmendment { .. } => ElicitationResponse {
            action: ElicitationAction::Accept,
            content: Some(serde_json::json!({})),
            meta: Some(mcp_elicitation_auto_meta()),
        },
        ReviewDecision::Denied => mcp_elicitation_decline_with_message(
            denial_message.unwrap_or_else(|| "Guardian denied this request.".to_string()),
        ),
        ReviewDecision::TimedOut => {
            mcp_elicitation_decline_with_message(crate::guardian::guardian_timeout_message())
        }
        ReviewDecision::Abort => ElicitationResponse {
            action: ElicitationAction::Cancel,
            content: None,
            meta: Some(mcp_elicitation_auto_meta()),
        },
    }
}

fn mcp_elicitation_decline_with_message(message: String) -> ElicitationResponse {
    ElicitationResponse {
        action: ElicitationAction::Decline,
        content: None,
        meta: Some(serde_json::json!({
            MCP_ELICITATION_DECLINE_MESSAGE_KEY: message,
            MCP_ELICITATION_APPROVALS_REVIEWER_KEY: ApprovalsReviewer::AutoReview,
        })),
    }
}

fn mcp_elicitation_decline_without_message() -> ElicitationResponse {
    ElicitationResponse {
        action: ElicitationAction::Decline,
        content: None,
        meta: Some(mcp_elicitation_auto_meta()),
    }
}

fn mcp_elicitation_auto_meta() -> serde_json::Value {
    serde_json::json!({
        MCP_ELICITATION_APPROVALS_REVIEWER_KEY: ApprovalsReviewer::AutoReview,
    })
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
