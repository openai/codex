use super::*;
use codex_exec_server::ExecutorCapabilityDiscoveryCache;
use codex_exec_server::ExecutorCapabilityDiscoverySnapshot;
use codex_exec_server::MAX_SELECTED_CAPABILITY_ROOTS;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::ElicitationReviewRequest;
use codex_mcp::ElicitationReviewer;
use codex_mcp::ElicitationReviewerHandle;
use codex_protocol::capabilities::SelectedCapabilityRoot;
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
use codex_rmcp_client::Elicitation;
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

/// Restores a claimed refresh when its task is cancelled before publication.
struct McpRefreshInvalidationGuard<'a> {
    pending: &'a std::sync::atomic::AtomicBool,
    published: bool,
}

impl Drop for McpRefreshInvalidationGuard<'_> {
    fn drop(&mut self) {
        if !self.published {
            self.pending
                .store(true, std::sync::atomic::Ordering::Release);
        }
    }
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
    pub(crate) async fn runtime_mcp_config(&self, config: &Config) -> McpConfig {
        self.runtime_mcp_config_and_context(config).await.0
    }

    pub(crate) async fn runtime_mcp_config_and_context(
        &self,
        config: &Config,
    ) -> (McpConfig, McpRuntimeContext) {
        let originator = self.originator().await;
        let environments = self.services.turn_environments.snapshot().await;
        let selected_capability_roots = self
            .resolve_selected_capability_roots_for_step(&environments)
            .await;
        let ready_selected_capability_roots =
            Self::ready_selected_capability_roots(&selected_capability_roots);
        let executor_capability_discovery = self
            .executor_capability_discovery_for_step(config, &ready_selected_capability_roots)
            .await;
        let mcp_config = self
            .services
            .mcp_manager
            .runtime_config_for_step(
                config,
                &self.services.mcp_thread_init,
                &self.services.thread_extension_data,
                &originator,
                &ready_selected_capability_roots,
                executor_capability_discovery.as_deref(),
            )
            .await
            .config;
        let local_stdio_fallback_cwd = environments
            .primary()
            .and_then(|environment| environment.cwd().to_abs_path().ok())
            .map(|cwd| cwd.to_path_buf())
            .unwrap_or_else(|| config.cwd.to_path_buf());
        let runtime_context = McpRuntimeContext::new(
            self.services.turn_environments.environment_manager(),
            local_stdio_fallback_cwd,
        );
        (mcp_config, runtime_context)
    }

    pub(crate) async fn runtime_mcp_servers(
        &self,
        config: &Config,
    ) -> HashMap<String, McpServerConfig> {
        codex_mcp::configured_mcp_servers(&self.runtime_mcp_config(config).await)
    }

    /// Publishes changed MCP state, waiting for any refresh already in progress.
    pub(crate) async fn refresh_mcp_if_dirty(self: &Arc<Self>) {
        let Ok(_refresh) = self.mcp_refresh_lock.acquire().await else {
            error!("MCP runtime refresh semaphore closed");
            return;
        };
        loop {
            if !self
                .mcp_refresh_pending
                .swap(false, std::sync::atomic::Ordering::AcqRel)
            {
                return;
            }
            let mut refresh_invalidation = McpRefreshInvalidationGuard {
                pending: &self.mcp_refresh_pending,
                published: false,
            };
            let desired = self.latest_mcp_desired_state().await;
            let selected_capability_roots = self
                .resolve_selected_capability_roots_for_step(&desired.environments)
                .await;
            let ready_selected_capability_roots =
                Self::ready_selected_capability_roots(&selected_capability_roots);
            let executor_capability_discovery = self
                .executor_capability_discovery_for_step(
                    &desired.config,
                    &ready_selected_capability_roots,
                )
                .await;
            let mcp_projection = self
                .services
                .mcp_manager
                .runtime_config_for_step(
                    &desired.config,
                    &self.services.mcp_thread_init,
                    &self.services.thread_extension_data,
                    &desired.originator,
                    &ready_selected_capability_roots,
                    executor_capability_discovery.as_deref(),
                )
                .await;
            self.publish_mcp_runtime(
                &desired,
                mcp_projection,
                &ready_selected_capability_roots,
                Some(self.mcp_elicitation_reviewer()),
            )
            .await;
            refresh_invalidation.published = true;
            if !self
                .mcp_refresh_pending
                .load(std::sync::atomic::Ordering::Acquire)
            {
                return;
            }
        }
    }

    /// Reconnects the runtime so refreshed Apps tools belong to their new exact client.
    pub(crate) async fn hard_refresh_latest_codex_apps_tools(
        self: &Arc<Self>,
    ) -> anyhow::Result<Vec<codex_mcp::ToolInfo>> {
        self.refresh_mcp_if_dirty().await;
        let _refresh = self
            .mcp_refresh_lock
            .acquire()
            .await
            .map_err(|_| anyhow::anyhow!("MCP runtime refresh semaphore closed"))?;
        let desired = self.latest_mcp_desired_state().await;
        let selected_capability_roots = self
            .resolve_selected_capability_roots_for_step(&desired.environments)
            .await;
        let ready_selected_capability_roots =
            Self::ready_selected_capability_roots(&selected_capability_roots);
        let executor_capability_discovery = self
            .executor_capability_discovery_for_step(
                &desired.config,
                &ready_selected_capability_roots,
            )
            .await;
        let mcp_projection = self
            .services
            .mcp_manager
            .runtime_config_for_step(
                &desired.config,
                &self.services.mcp_thread_init,
                &self.services.thread_extension_data,
                &desired.originator,
                &ready_selected_capability_roots,
                executor_capability_discovery.as_deref(),
            )
            .await;
        let input = self
            .build_mcp_runtime_input(
                &desired,
                mcp_projection,
                &ready_selected_capability_roots,
                Some(self.mcp_elicitation_reviewer()),
            )
            .await;
        anyhow::ensure!(
            input.mcp_servers.contains_key(CODEX_APPS_MCP_SERVER_NAME),
            "unknown MCP server '{CODEX_APPS_MCP_SERVER_NAME}'"
        );
        self.services.mcp_runtime.replace_fresh(input).await
    }

    pub(super) fn mark_mcp_runtime_dirty(&self) {
        self.mcp_refresh_pending
            .store(true, std::sync::atomic::Ordering::Release);
    }

    #[tracing::instrument(name = "mcp.runtime.resolve_for_step", skip_all)]
    pub(crate) async fn mcp_runtime_for_step(
        self: &Arc<Self>,
        turn_context: &TurnContext,
        selected_capability_roots: &[ResolvedSelectedCapabilityRoot],
    ) -> Arc<codex_mcp::McpBinding> {
        let ready_selected_capability_roots =
            Self::ready_selected_capability_roots(selected_capability_roots);
        if self
            .services
            .mcp_runtime
            .current_ready_selected_capability_roots()
            != ready_selected_capability_roots
        {
            self.mark_mcp_runtime_dirty();
        }
        self.refresh_mcp_if_dirty().await;
        if let Some(binding) = self.services.mcp_runtime.current_binding().await {
            return binding;
        }
        let config = Arc::new(self.runtime_mcp_config(&turn_context.config).await);
        Arc::new(codex_mcp::McpBinding::empty(config))
    }

    #[tracing::instrument(
        name = "capability_roots.snapshot_for_step",
        skip_all,
        fields(root_count = ready_selected_capability_roots.len())
    )]
    pub(crate) async fn executor_capability_discovery_for_step(
        &self,
        config: &Config,
        ready_selected_capability_roots: &[SelectedCapabilityRoot],
    ) -> Option<Arc<ExecutorCapabilityDiscoverySnapshot>> {
        if !config
            .features
            .enabled(Feature::ExecutorCapabilityDiscovery)
        {
            return None;
        }
        let environment_manager = self.services.turn_environments.environment_manager();
        let cache = self
            .services
            .thread_extension_data
            .get_or_init(|| ExecutorCapabilityDiscoveryCache::new(environment_manager));
        Some(Arc::new(
            cache.snapshot(ready_selected_capability_roots).await,
        ))
    }

    pub(crate) async fn resolve_selected_capability_roots_for_step(
        &self,
        environments: &TurnEnvironmentSnapshot,
    ) -> Vec<ResolvedSelectedCapabilityRoot> {
        let thread_root_count = self.services.selected_capability_roots.len();
        let mut root_locations_by_id = HashMap::new();
        let mut selected_capability_roots = Vec::new();
        let mut ready_environment_root_count = 0;
        for (index, root) in self
            .services
            .selected_capability_roots
            .iter()
            .chain(
                environments
                    .turn_environments()
                    .flat_map(|environment| environment.environment.selected_capability_roots()),
            )
            .enumerate()
        {
            if let Some(kept_location) = root_locations_by_id.get(&root.id) {
                if kept_location != &root.location {
                    tracing::warn!(
                        root_id = root.id,
                        ?kept_location,
                        ignored_location = ?root.location,
                        "ignoring selected capability root with conflicting location"
                    );
                }
                continue;
            }
            if index >= thread_root_count {
                if ready_environment_root_count == MAX_SELECTED_CAPABILITY_ROOTS {
                    tracing::warn!(
                        max_root_count = MAX_SELECTED_CAPABILITY_ROOTS,
                        "ignoring excess selected capability roots from ready environments"
                    );
                    break;
                }
                ready_environment_root_count += 1;
            }
            root_locations_by_id.insert(root.id.clone(), root.location.clone());
            selected_capability_roots.push(root.clone());
        }
        self.services
            .turn_environments
            .environment_manager()
            .resolve_selected_capability_roots(
                &selected_capability_roots,
                &environments.captured_environments(),
            )
            .await
    }

    pub(crate) fn mcp_elicitation_reviewer(self: &Arc<Self>) -> ElicitationReviewerHandle {
        Arc::clone(
            self.mcp_elicitation_reviewer_handle
                .get_or_init(|| Arc::new(GuardianMcpElicitationReviewer::new(self))),
        )
    }

    pub(crate) fn mcp_elicitation_lifecycle(&self) -> codex_mcp::ElicitationLifecycle {
        self.mcp_elicitation_lifecycle_handle
            .get_or_init(|| {
                let elicitations = self.services.elicitations.clone();
                codex_mcp::ElicitationLifecycle::new(move || elicitations.register())
            })
            .clone()
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and turn state updates must remain atomic"
    )]
    pub async fn request_mcp_server_elicitation(
        &self,
        turn_context: &TurnContext,
        server_name: String,
        request_id: RequestId,
        request: ElicitationRequest,
    ) -> McpServerElicitationOutcome {
        if self.services.mcp_runtime.elicitations_auto_deny() {
            return McpServerElicitationOutcome {
                response: Some(ElicitationResponse {
                    action: codex_rmcp_client::ElicitationAction::Accept,
                    content: Some(serde_json::json!({})),
                    meta: None,
                }),
                sent: false,
            };
        }

        let _elicitation = self.services.elicitations.register();
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
            turn_id: Some(turn_context.sub_id.clone()),
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
            .mcp_runtime
            .resolve_elicitation(server_name, id, response)
            .await
    }

    pub(crate) async fn set_openai_form_elicitation_support(
        &self,
        supported: bool,
    ) -> anyhow::Result<()> {
        if self
            .services
            .supports_openai_form_elicitation
            .load(std::sync::atomic::Ordering::Relaxed)
            == supported
        {
            return Ok(());
        }

        self.services
            .supports_openai_form_elicitation
            .store(supported, std::sync::atomic::Ordering::Relaxed);
        self.mark_mcp_runtime_dirty();
        Ok(())
    }

    pub(crate) async fn refresh_mcp_servers_now(
        &self,
        turn_context: &TurnContext,
        refresh_config: &Config,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) {
        let Ok(_refresh) = self.mcp_refresh_lock.acquire().await else {
            error!("MCP runtime refresh semaphore closed");
            return;
        };
        {
            let mut state = self.state.lock().await;
            let mut config = (*state.session_configuration.original_config_do_not_use).clone();
            config.mcp_servers = refresh_config.mcp_servers.clone();
            state.session_configuration.original_config_do_not_use = Arc::new(config);
        }
        let ready_selected_capability_roots = self
            .services
            .mcp_runtime
            .current_ready_selected_capability_roots();
        let executor_capability_discovery = self
            .executor_capability_discovery_for_step(
                refresh_config,
                &ready_selected_capability_roots,
            )
            .await;
        let mcp_projection = self
            .services
            .mcp_manager
            .runtime_config_for_step(
                refresh_config,
                &self.services.mcp_thread_init,
                &self.services.thread_extension_data,
                &turn_context.originator,
                &ready_selected_capability_roots,
                executor_capability_discovery.as_deref(),
            )
            .await;
        let mut desired = self.latest_mcp_desired_state().await;
        desired.config = Arc::new(refresh_config.clone());
        self.publish_mcp_runtime(
            &desired,
            mcp_projection,
            &ready_selected_capability_roots,
            elicitation_reviewer,
        )
        .await;
    }

    pub(crate) fn ready_selected_capability_roots(
        selected_capability_roots: &[ResolvedSelectedCapabilityRoot],
    ) -> Vec<SelectedCapabilityRoot> {
        selected_capability_roots
            .iter()
            .map(|root| root.selected_root().clone())
            .collect()
    }

    pub(crate) fn cancel_mcp_startup(&self) {
        self.services.mcp_runtime.cancel_startup();
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
    Ok(Some(mcp_elicitation_response_from_guardian_decision(
        decision,
    )))
}

fn guardian_elicitation_review_request(
    request: &ElicitationReviewRequest,
) -> GuardianElicitationReview {
    let (meta, requested_schema) = match &request.elicitation {
        Elicitation::Mcp(rmcp::model::CreateElicitationRequestParams::FormElicitationParams {
            meta,
            requested_schema,
            ..
        }) => (meta, Some(requested_schema)),
        Elicitation::Mcp(rmcp::model::CreateElicitationRequestParams::UrlElicitationParams {
            meta,
            ..
        }) => {
            return if meta_requests_approval_request(meta) {
                GuardianElicitationReview::Decline(
                    "guardian MCP elicitation review only supports form elicitations",
                )
            } else {
                GuardianElicitationReview::NotRequested
            };
        }
        Elicitation::OpenAiForm { .. } => return GuardianElicitationReview::NotRequested,
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
            connected_account_email: None,
            tool_title: metadata_owned_string(meta, MCP_ELICITATION_TOOL_TITLE_KEY),
            tool_description: metadata_owned_string(meta, MCP_ELICITATION_TOOL_DESCRIPTION_KEY),
            annotations: None,
        },
    ))
}

fn elicitation_connector_id(elicitation: &Elicitation) -> Option<&str> {
    elicitation
        .meta()
        .and_then(|meta| metadata_str(meta, MCP_ELICITATION_CONNECTOR_ID_KEY))
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

fn mcp_elicitation_response_from_guardian_decision(
    decision: ReviewDecision,
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
        ReviewDecision::Denied { rejection } => mcp_elicitation_decline_with_message(rejection),
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
