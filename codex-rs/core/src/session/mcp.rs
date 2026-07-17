use super::*;
use crate::mcp::McpRuntimeProjection;
use codex_exec_server::ExecutorCapabilityDiscoveryCache;
use codex_exec_server::ExecutorCapabilityDiscoverySnapshot;
use codex_exec_server::MAX_SELECTED_CAPABILITY_ROOTS;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_mcp::ElicitationReviewRequest;
use codex_mcp::ElicitationReviewer;
use codex_mcp::ElicitationReviewerHandle;
use codex_protocol::capabilities::CapabilityRootLocation;
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
        self.services
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
            .config
    }

    pub(crate) async fn runtime_mcp_servers(
        &self,
        config: &Config,
    ) -> HashMap<String, McpServerConfig> {
        codex_mcp::configured_mcp_servers(&self.runtime_mcp_config(config).await)
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP runtime comparison and publication must remain serialized"
    )]
    #[tracing::instrument(name = "mcp.runtime.resolve_for_step", skip_all)]
    pub(crate) async fn mcp_runtime_for_step(
        self: &Arc<Self>,
        turn_context: &TurnContext,
        environments: &TurnEnvironmentSnapshot,
        selected_capability_roots: &[ResolvedSelectedCapabilityRoot],
        executor_capability_discovery: Option<&ExecutorCapabilityDiscoverySnapshot>,
    ) -> Arc<McpRuntimeSnapshot> {
        let ready_selected_capability_roots =
            Self::ready_selected_capability_roots(selected_capability_roots);
        let available_environment_ids =
            Self::available_selected_environment_ids(selected_capability_roots);
        let current = self.services.latest_mcp_runtime();
        if current.ready_selected_capability_roots() == ready_selected_capability_roots {
            return current;
        }

        let _guard = self.services.mcp_projection_lock.lock().await;
        let current = self.services.latest_mcp_runtime();
        if current.ready_selected_capability_roots() == ready_selected_capability_roots {
            return current;
        }
        let mcp_projection = self
            .services
            .mcp_manager
            .runtime_config_for_step(
                &turn_context.config,
                &self.services.mcp_thread_init,
                &self.services.thread_extension_data,
                &turn_context.originator,
                &ready_selected_capability_roots,
                executor_capability_discovery,
            )
            .await;
        let mcp_config = &mcp_projection.config;
        let changed_environment_is_used_by_mcp = mcp_config
            .mcp_server_catalog
            .configured_servers()
            .values()
            .any(|server| {
                let was_available = current
                    .ready_selected_capability_roots()
                    .iter()
                    .any(|root| {
                        let CapabilityRootLocation::Environment { environment_id, .. } =
                            &root.location;
                        environment_id == &server.environment_id
                    });
                let is_available = available_environment_ids.contains(&server.environment_id);
                server.enabled && was_available != is_available
            });
        if !changed_environment_is_used_by_mcp
            && current
                .config()
                .mcp_server_catalog
                .has_same_servers(&mcp_config.mcp_server_catalog)
            && current.config().connector_snapshot == mcp_config.connector_snapshot
        {
            // Selected roots are only an input to the MCP projection. When they change but the
            // projected servers and connectors do not, advance the input key without
            // replacing the live manager and restarting its processes.
            let runtime = Arc::new(McpRuntimeSnapshot::new(
                Arc::new(current.config().clone()),
                mcp_projection.plugins_available,
                current.manager_arc(),
                current.runtime_context().clone(),
                ready_selected_capability_roots,
            ));
            self.services.mcp_runtime.store(Some(Arc::clone(&runtime)));
            return runtime;
        }
        self.refresh_mcp_servers_inner(
            turn_context,
            mcp_projection,
            environments,
            &ready_selected_capability_roots,
            Some(self.mcp_elicitation_reviewer()),
        )
        .await
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
        Arc::new(GuardianMcpElicitationReviewer::new(self))
    }

    pub(crate) fn mcp_elicitation_lifecycle(&self) -> codex_mcp::ElicitationLifecycle {
        let elicitations = self.services.elicitations.clone();
        codex_mcp::ElicitationLifecycle::new(move || elicitations.register())
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
        if self
            .services
            .latest_mcp_runtime()
            .manager()
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
            .latest_mcp_runtime()
            .manager_arc()
            .resolve_elicitation(server_name, id, response)
            .await
    }

    #[tracing::instrument(name = "mcp.runtime.refresh", skip_all)]
    async fn refresh_mcp_servers_inner(
        &self,
        turn_context: &TurnContext,
        mcp_projection: McpRuntimeProjection,
        environments: &TurnEnvironmentSnapshot,
        ready_selected_capability_roots: &[SelectedCapabilityRoot],
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) -> Arc<McpRuntimeSnapshot> {
        let auth = self.services.auth_manager.auth().await;
        let McpRuntimeProjection {
            config: mcp_config,
            plugins_available,
        } = mcp_projection;
        let mcp_config = Arc::new(mcp_config);
        let tool_plugin_provenance = codex_mcp::tool_plugin_provenance(&mcp_config);
        let mcp_servers = effective_mcp_servers(&mcp_config, auth.as_ref());
        let environment_manager = self.services.turn_environments.environment_manager();
        // TODO(anp): Migrate MCP runtime cwd plumbing to PathUri so foreign environment cwd
        // values can be used without falling back to the legacy host cwd.
        let cwd = environments
            .primary()
            .and_then(|turn_environment| turn_environment.cwd().to_abs_path().ok())
            .map(|cwd| cwd.to_path_buf())
            .unwrap_or_else(|| {
                #[allow(deprecated)]
                turn_context.cwd.to_path_buf()
            });
        let mcp_runtime_context = McpRuntimeContext::new(environment_manager, cwd);
        let mcp_startup_cancellation_token = {
            let mut guard = self.services.mcp_startup_cancellation_token.lock().await;
            // The previous runtime owns the old token and may still be serving an in-flight step.
            // Its manager cancels that token when the last runtime handle is dropped.
            let cancellation_token = CancellationToken::new();
            *guard = cancellation_token.clone();
            cancellation_token
        };
        let current_runtime = self.services.latest_mcp_runtime();
        let codex_apps_auth_manager =
            codex_mcp::host_owned_codex_apps_enabled(&mcp_config, auth.as_ref())
                .then(|| Arc::clone(&self.services.auth_manager));
        let refreshed_manager = McpConnectionManager::new(
            &mcp_servers,
            mcp_config.mcp_oauth_credentials_store_mode,
            mcp_config.auth_keyring_backend_kind,
            &turn_context.approval_policy,
            turn_context.sub_id.clone(),
            self.get_tx_event(),
            mcp_startup_cancellation_token,
            turn_context.permission_profile(),
            mcp_runtime_context.clone(),
            mcp_config.codex_home.clone(),
            self.services.mcp_manager.codex_apps_tools_cache(),
            self.services.mcp_manager.tool_catalog_cache(),
            connector_runtime_context_key(auth.as_ref()),
            mcp_config.prefix_mcp_tool_names,
            mcp_config.client_elicitation_capability.clone(),
            self.services
                .supports_openai_form_elicitation
                .load(std::sync::atomic::Ordering::Relaxed),
            tool_plugin_provenance,
            auth.as_ref(),
            codex_apps_auth_manager,
            elicitation_reviewer,
            Some(self.mcp_elicitation_lifecycle()),
            current_runtime.manager().elicitation_router(),
        )
        .await;
        refreshed_manager
            .set_elicitations_auto_deny(current_runtime.manager().elicitations_auto_deny());
        self.services.publish_mcp_runtime(
            mcp_config,
            plugins_available,
            mcp_runtime_context,
            ready_selected_capability_roots.to_vec(),
            refreshed_manager,
        )
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP runtime refresh and publication must remain serialized"
    )]
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
            auth_keyring_backend_kind,
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
        let keyring_backend_kind =
            match serde_json::from_value::<AuthKeyringBackendKind>(auth_keyring_backend_kind) {
                Ok(kind) => kind,
                Err(err) => {
                    warn!("failed to parse MCP auth keyring backend refresh config: {err}");
                    return;
                }
            };

        let mut refresh_config = self.get_config().await.as_ref().clone();
        refresh_config.mcp_oauth_credentials_store_mode = store_mode;
        let secret_auth_storage_enabled = match keyring_backend_kind {
            AuthKeyringBackendKind::Direct => false,
            AuthKeyringBackendKind::Secrets => true,
        };
        if let Err(err) = refresh_config
            .features
            .set_enabled(Feature::SecretAuthStorage, secret_auth_storage_enabled)
        {
            warn!("failed to apply MCP auth keyring backend refresh config: {err}");
            return;
        }

        let _guard = self.services.mcp_projection_lock.lock().await;
        let current_runtime = self.services.latest_mcp_runtime();
        let ready_selected_capability_roots =
            current_runtime.ready_selected_capability_roots().to_vec();
        let executor_capability_discovery = self
            .executor_capability_discovery_for_step(
                &refresh_config,
                &ready_selected_capability_roots,
            )
            .await;
        let mut mcp_projection = self
            .services
            .mcp_manager
            .runtime_config_for_step(
                &refresh_config,
                &self.services.mcp_thread_init,
                &self.services.thread_extension_data,
                &turn_context.originator,
                &ready_selected_capability_roots,
                executor_capability_discovery.as_deref(),
            )
            .await;
        mcp_projection.config.mcp_server_catalog = mcp_projection
            .config
            .mcp_server_catalog
            .with_materialized_servers(mcp_servers);
        self.refresh_mcp_servers_inner(
            turn_context,
            mcp_projection,
            &turn_context.environments,
            &ready_selected_capability_roots,
            elicitation_reviewer,
        )
        .await;
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

        let config = self.get_config().await;
        let refresh_config = McpServerRefreshConfig {
            mcp_servers: serde_json::to_value(config.mcp_servers.get())?,
            mcp_oauth_credentials_store_mode: serde_json::to_value(
                config.mcp_oauth_credentials_store_mode,
            )?,
            auth_keyring_backend_kind: serde_json::to_value(config.auth_keyring_backend_kind())?,
        };
        self.services
            .supports_openai_form_elicitation
            .store(supported, std::sync::atomic::Ordering::Relaxed);
        *self.pending_mcp_server_refresh_config.lock().await = Some(refresh_config);
        Ok(())
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP runtime refresh and publication must remain serialized"
    )]
    pub(crate) async fn refresh_mcp_servers_now(
        &self,
        turn_context: &TurnContext,
        refresh_config: &Config,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) {
        let _guard = self.services.mcp_projection_lock.lock().await;
        let current_runtime = self.services.latest_mcp_runtime();
        let ready_selected_capability_roots =
            current_runtime.ready_selected_capability_roots().to_vec();
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
        self.refresh_mcp_servers_inner(
            turn_context,
            mcp_projection,
            &turn_context.environments,
            &ready_selected_capability_roots,
            elicitation_reviewer,
        )
        .await;
    }

    fn available_selected_environment_ids(
        selected_capability_roots: &[ResolvedSelectedCapabilityRoot],
    ) -> Vec<String> {
        let mut available = Vec::new();
        for root in selected_capability_roots {
            let CapabilityRootLocation::Environment { environment_id, .. } =
                &root.selected_root().location;
            if !available.contains(environment_id) {
                available.push(environment_id.clone());
            }
        }
        available
    }

    pub(crate) fn ready_selected_capability_roots(
        selected_capability_roots: &[ResolvedSelectedCapabilityRoot],
    ) -> Vec<SelectedCapabilityRoot> {
        selected_capability_roots
            .iter()
            .map(|root| root.selected_root().clone())
            .collect()
    }

    #[cfg(test)]
    pub(crate) async fn mcp_startup_cancellation_token(&self) -> CancellationToken {
        self.services
            .mcp_startup_cancellation_token
            .lock()
            .await
            .clone()
    }

    pub(crate) async fn cancel_mcp_startup(&self) {
        self.services
            .mcp_startup_cancellation_token
            .lock()
            .await
            .cancel();
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
