use super::*;

const MCP_ELICITATION_CONNECTOR_ID_KEY: &str = "connector_id";
const MCP_ELICITATION_CONNECTOR_NAME_KEY: &str = "connector_name";
const MCP_ELICITATION_CONNECTOR_DISPLAY_NAME_KEY: &str = "connector_display_name";

impl Session {
    pub(crate) fn mcp_elicitation_reviewer(self: &Arc<Self>) -> McpElicitationReviewer {
        let session = Arc::downgrade(self);
        Arc::new(move |request| {
            let session = session.clone();
            async move {
                let session = session.upgrade()?;
                session.review_mcp_elicitation(request).await
            }
            .boxed()
        })
    }

    async fn review_mcp_elicitation(
        self: Arc<Self>,
        request: McpElicitationReviewRequest,
    ) -> Option<ElicitationResponse> {
        if !guardian_can_review_mcp_elicitation(&request.request) {
            return None;
        }

        let (turn_context, cancellation_token) =
            self.active_turn_context_and_cancellation_token().await?;
        if !crate::guardian::routes_approval_to_guardian(turn_context.as_ref()) {
            return None;
        }

        let review_rx = crate::guardian::spawn_approval_request_review(
            Arc::clone(&self),
            Arc::clone(&turn_context),
            crate::guardian::new_guardian_review_id(),
            build_guardian_mcp_elicitation_review_request(request, &turn_context.sub_id),
            /*retry_reason*/ None,
            codex_analytics::GuardianApprovalRequestSource::MainTurn,
            cancellation_token.clone(),
        );
        let decision = tokio::select! {
            biased;
            _ = cancellation_token.cancelled() => ReviewDecision::Abort,
            decision = review_rx => decision.unwrap_or(ReviewDecision::Denied),
        };

        Some(mcp_elicitation_response_from_guardian_decision(decision))
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
    ) -> Option<ElicitationResponse> {
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
                        return None;
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
        let id = protocol_request_id(request_id);
        let event = EventMsg::ElicitationRequest(ElicitationRequestEvent {
            turn_id: params.turn_id,
            server_name,
            id,
            request,
        });
        self.send_event(turn_context, event).await;
        rx_response.await.ok()
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
            .read()
            .await
            .resolve_elicitation(server_name, id, response)
            .await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP resource calls are serialized through the session-owned manager guard"
    )]
    pub async fn list_resources(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> anyhow::Result<ListResourcesResult> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .list_resources(server, params)
            .await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP resource calls are serialized through the session-owned manager guard"
    )]
    pub async fn list_resource_templates(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> anyhow::Result<ListResourceTemplatesResult> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .list_resource_templates(server, params)
            .await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP resource calls are serialized through the session-owned manager guard"
    )]
    pub async fn read_resource(
        &self,
        server: &str,
        params: ReadResourceRequestParams,
    ) -> anyhow::Result<ReadResourceResult> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .read_resource(server, params)
            .await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP tool calls are serialized through the session-owned manager guard"
    )]
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> anyhow::Result<CallToolResult> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .call_tool(server, tool, arguments, meta)
            .await
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP tool metadata reads through the session-owned manager guard"
    )]
    pub(crate) async fn resolve_mcp_tool_info(&self, tool_name: &ToolName) -> Option<ToolInfo> {
        self.services
            .mcp_connection_manager
            .read()
            .await
            .resolve_tool_info(tool_name)
            .await
    }

    async fn refresh_mcp_servers_inner(
        self: &Arc<Self>,
        turn_context: &TurnContext,
        mcp_servers: HashMap<String, McpServerConfig>,
        store_mode: OAuthCredentialsStoreMode,
    ) {
        let auth = self.services.auth_manager.auth().await;
        let config = self.get_config().await;
        let mcp_config = config
            .to_mcp_config(self.services.plugins_manager.as_ref())
            .await;
        let tool_plugin_provenance = self
            .services
            .mcp_manager
            .tool_plugin_provenance(config.as_ref())
            .await;
        let mcp_servers = with_codex_apps_mcp(mcp_servers, auth.as_ref(), &mcp_config);
        let auth_statuses =
            compute_auth_statuses(mcp_servers.iter(), store_mode, auth.as_ref()).await;
        {
            let mut guard = self.services.mcp_startup_cancellation_token.lock().await;
            guard.cancel();
            *guard = CancellationToken::new();
        }
        let (refreshed_manager, cancel_token) = McpConnectionManager::new(
            &mcp_servers,
            store_mode,
            auth_statuses,
            &turn_context.approval_policy,
            turn_context.sub_id.clone(),
            self.get_tx_event(),
            turn_context.sandbox_policy.get().clone(),
            McpRuntimeEnvironment::new(
                turn_context
                    .environment
                    .clone()
                    .unwrap_or_else(|| self.services.environment_manager.local_environment()),
                turn_context.cwd.to_path_buf(),
            ),
            config.codex_home.to_path_buf(),
            codex_apps_tools_cache_key(auth.as_ref()),
            tool_plugin_provenance,
            Some(self.mcp_elicitation_reviewer()),
            auth.as_ref(),
        )
        .await;
        {
            let mut guard = self.services.mcp_startup_cancellation_token.lock().await;
            if guard.is_cancelled() {
                cancel_token.cancel();
            }
            *guard = cancel_token;
        }

        let mut manager = self.services.mcp_connection_manager.write().await;
        *manager = refreshed_manager;
    }

    pub(crate) async fn refresh_mcp_servers_if_requested(
        self: &Arc<Self>,
        turn_context: &TurnContext,
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

        self.refresh_mcp_servers_inner(turn_context, mcp_servers, store_mode)
            .await;
    }

    pub(crate) async fn refresh_mcp_servers_now(
        self: &Arc<Self>,
        turn_context: &TurnContext,
        mcp_servers: HashMap<String, McpServerConfig>,
        store_mode: OAuthCredentialsStoreMode,
    ) {
        self.refresh_mcp_servers_inner(turn_context, mcp_servers, store_mode)
            .await;
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

fn guardian_can_review_mcp_elicitation(
    request: &codex_protocol::approvals::ElicitationRequest,
) -> bool {
    match request {
        codex_protocol::approvals::ElicitationRequest::Form {
            requested_schema, ..
        } => mcp_elicitation_form_has_empty_schema(requested_schema),
        codex_protocol::approvals::ElicitationRequest::Url { .. } => false,
    }
}

fn mcp_elicitation_form_has_empty_schema(requested_schema: &serde_json::Value) -> bool {
    let properties_empty = requested_schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .is_none_or(serde_json::Map::is_empty);
    let required_empty = requested_schema
        .get("required")
        .and_then(serde_json::Value::as_array)
        .is_none_or(Vec::is_empty);
    properties_empty && required_empty
}

fn build_guardian_mcp_elicitation_review_request(
    request: McpElicitationReviewRequest,
    turn_id: &str,
) -> crate::guardian::GuardianApprovalRequest {
    let McpElicitationReviewRequest {
        server_name,
        request_id,
        request,
    } = request;
    let meta = mcp_elicitation_meta(&request);
    let connector_id = mcp_elicitation_meta_string(meta, MCP_ELICITATION_CONNECTOR_ID_KEY);
    let connector_name = mcp_elicitation_meta_string(meta, MCP_ELICITATION_CONNECTOR_NAME_KEY)
        .or_else(|| mcp_elicitation_meta_string(meta, MCP_ELICITATION_CONNECTOR_DISPLAY_NAME_KEY));
    crate::guardian::GuardianApprovalRequest::McpElicitation {
        id: format!("mcp_elicitation:{server_name}:{request_id}"),
        turn_id: turn_id.to_string(),
        server_name,
        request,
        connector_id,
        connector_name,
    }
}

fn mcp_elicitation_meta(
    request: &codex_protocol::approvals::ElicitationRequest,
) -> Option<&serde_json::Value> {
    match request {
        codex_protocol::approvals::ElicitationRequest::Form { meta, .. }
        | codex_protocol::approvals::ElicitationRequest::Url { meta, .. } => meta.as_ref(),
    }
}

fn mcp_elicitation_meta_string(meta: Option<&serde_json::Value>, key: &str) -> Option<String> {
    meta.and_then(serde_json::Value::as_object)
        .and_then(|meta| meta.get(key))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn mcp_elicitation_response_from_guardian_decision(
    decision: ReviewDecision,
) -> ElicitationResponse {
    let action = match decision {
        ReviewDecision::Approved | ReviewDecision::ApprovedForSession => ElicitationAction::Accept,
        ReviewDecision::Denied
        | ReviewDecision::TimedOut
        | ReviewDecision::ApprovedExecpolicyAmendment { .. }
        | ReviewDecision::NetworkPolicyAmendment { .. } => ElicitationAction::Decline,
        ReviewDecision::Abort => ElicitationAction::Cancel,
    };
    let should_accept = action == ElicitationAction::Accept;
    ElicitationResponse {
        action,
        content: should_accept.then(|| serde_json::json!({})),
        meta: None,
    }
}
