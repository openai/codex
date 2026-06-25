use super::session::PendingMcpServerRefresh;
use super::*;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_mcp::ElicitationReviewRequest;
use codex_mcp::ElicitationReviewer;
use codex_mcp::ElicitationReviewerHandle;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::mcp_approval_meta::APPROVAL_KIND_KEY as MCP_ELICITATION_APPROVAL_KIND_KEY;
use codex_protocol::mcp_approval_meta::APPROVAL_KIND_MCP_TOOL_CALL as MCP_ELICITATION_APPROVAL_KIND_MCP_TOOL_CALL;
use codex_protocol::mcp_approval_meta::APPROVAL_KIND_TOOL_SUGGESTION as MCP_ELICITATION_APPROVAL_KIND_TOOL_SUGGESTION;
use codex_protocol::mcp_approval_meta::APPROVALS_REVIEWER_KEY as MCP_ELICITATION_APPROVALS_REVIEWER_KEY;
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

#[derive(Clone, Copy)]
enum McpRefreshMode {
    Restart,
    ReuseUnchanged,
}

enum McpRefreshSource {
    /// An intentional override, such as an ephemeral skill dependency snapshot.
    Independent {
        configured_base: McpConfiguredBase,
        store_mode: OAuthCredentialsStoreMode,
        keyring_backend_kind: AuthKeyringBackendKind,
    },
    /// A queued base that must be rebuilt if the session config advances before publication.
    SessionConfig {
        contributor_config: Arc<Config>,
        configured_base: McpConfiguredBase,
        store_mode: OAuthCredentialsStoreMode,
        keyring_backend_kind: AuthKeyringBackendKind,
    },
}

struct McpRuntimeReplacement {
    mcp_config: McpConfig,
    runtime_context: McpRuntimeContext,
    available_environment_ids: Vec<String>,
    inputs: McpRuntimeInputs,
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
    pub(super) fn mcp_runtime_context_for_environments(
        environment_manager: Arc<EnvironmentManager>,
        environments: &TurnEnvironmentSnapshot,
        fallback_cwd: PathBuf,
    ) -> McpRuntimeContext {
        // TODO(anp): Migrate MCP runtime cwd plumbing to PathUri so foreign environment cwd
        // values can be used without falling back to the legacy host cwd.
        let cwd = environments
            .primary()
            .and_then(|turn_environment| turn_environment.cwd().to_abs_path().ok())
            .map(|cwd| cwd.to_path_buf())
            .unwrap_or(fallback_cwd);
        McpRuntimeContext::new_with_environment_overrides(
            environment_manager,
            cwd,
            environments.turn_environments.iter().map(|environment| {
                (
                    environment.environment_id.clone(),
                    Arc::clone(&environment.environment),
                )
            }),
        )
    }

    pub(crate) async fn runtime_mcp_config(&self, config: &Config) -> McpConfig {
        let environments = self.services.turn_environments.snapshot().await;
        let selected_capability_roots = self
            .resolve_selected_capability_roots_for_step(&environments)
            .await;
        let available_environment_ids =
            Self::available_selected_environment_ids(&selected_capability_roots);
        self.services
            .mcp_manager
            .runtime_config_for_step(
                config,
                &self.services.mcp_thread_init,
                &self.services.thread_extension_data,
                &available_environment_ids,
            )
            .await
    }

    pub(crate) async fn runtime_mcp_servers(
        &self,
        config: &Config,
    ) -> HashMap<String, McpServerConfig> {
        codex_mcp::configured_mcp_servers(&self.runtime_mcp_config(config).await)
    }

    pub(crate) async fn configured_mcp_base(&self, config: &Config) -> McpConfiguredBase {
        self.services.mcp_manager.configured_base(config).await
    }

    #[cfg(test)]
    pub(crate) async fn queue_mcp_server_refresh_from_config(&self, config: &Config) {
        let contributor_config = self.mcp_source_config().await;
        let configured_base = self.configured_mcp_base(config).await;
        *self.pending_mcp_server_refresh.lock().await = Some(PendingMcpServerRefresh::Sourceful {
            contributor_config,
            configured_base,
            store_mode: config.mcp_oauth_credentials_store_mode,
            keyring_backend_kind: config.auth_keyring_backend_kind(),
        });
    }

    pub(crate) async fn queue_mcp_server_refresh_from_current_config(&self) {
        let config = self.mcp_source_config().await;
        let configured_base = self.configured_mcp_base(config.as_ref()).await;
        *self.pending_mcp_server_refresh.lock().await = Some(PendingMcpServerRefresh::Sourceful {
            contributor_config: Arc::clone(&config),
            configured_base,
            store_mode: config.mcp_oauth_credentials_store_mode,
            keyring_backend_kind: config.auth_keyring_backend_kind(),
        });
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP runtime comparison and publication must remain serialized"
    )]
    pub(crate) async fn mcp_runtime_for_step(
        &self,
        turn_context: &TurnContext,
        environments: &TurnEnvironmentSnapshot,
        selected_capability_roots: &[ResolvedSelectedCapabilityRoot],
    ) -> Arc<McpRuntimeSnapshot> {
        let available_environment_ids =
            Self::available_selected_environment_ids(selected_capability_roots);
        let contributor_config = self.mcp_source_config().await;
        let contributors_revision = self.services.mcp_manager.contributors_revision();
        let mcp_runtime_context = Self::mcp_runtime_context_for_environments(
            self.services.turn_environments.environment_manager(),
            environments,
            {
                #[allow(deprecated)]
                turn_context.cwd.to_path_buf()
            },
        );
        {
            let current = self.services.latest_mcp_runtime();
            if current.matches(
                &contributor_config,
                &contributors_revision,
                &mcp_runtime_context,
                &available_environment_ids,
            ) {
                return current;
            }
        }

        let _guard = self.services.mcp_refresh_lock.lock().await;
        let contributor_config = self.mcp_source_config().await;
        let contributors_revision = self.services.mcp_manager.contributors_revision();
        let mcp_runtime_context = Self::mcp_runtime_context_for_environments(
            self.services.turn_environments.environment_manager(),
            environments,
            {
                #[allow(deprecated)]
                turn_context.cwd.to_path_buf()
            },
        );
        let current = self.services.latest_mcp_runtime();
        if current.matches(
            &contributor_config,
            &contributors_revision,
            &mcp_runtime_context,
            &available_environment_ids,
        ) {
            return current;
        }
        let inputs = current.inputs();
        let published_inputs =
            Arc::ptr_eq(&inputs.contributor_config, &contributor_config).then(|| {
                (
                    inputs.configured_base.clone(),
                    inputs.store_mode,
                    inputs.keyring_backend_kind,
                )
            });
        let (mcp_config, configured_base, contributors_revision, store_mode, keyring_backend_kind) =
            match published_inputs {
                Some((configured_base, store_mode, keyring_backend_kind)) => {
                    let (mcp_config, contributors_revision) = self
                        .services
                        .mcp_manager
                        .runtime_config_for_step_from_base_with_revision(
                            contributor_config.as_ref(),
                            &self.services.mcp_thread_init,
                            &self.services.thread_extension_data,
                            &available_environment_ids,
                            &configured_base,
                        )
                        .await;
                    (
                        mcp_config,
                        configured_base,
                        contributors_revision,
                        store_mode,
                        keyring_backend_kind,
                    )
                }
                None => {
                    let (mcp_config, configured_base, contributors_revision) = self
                        .services
                        .mcp_manager
                        .runtime_config_for_step_with_base_and_revision(
                            contributor_config.as_ref(),
                            &self.services.mcp_thread_init,
                            &self.services.thread_extension_data,
                            &available_environment_ids,
                        )
                        .await;
                    (
                        mcp_config,
                        configured_base,
                        contributors_revision,
                        contributor_config.mcp_oauth_credentials_store_mode,
                        contributor_config.auth_keyring_backend_kind(),
                    )
                }
            };
        let elicitation_reviewer = current.manager().elicitation_reviewer();
        self.replace_mcp_servers(
            turn_context,
            McpRuntimeReplacement {
                mcp_config,
                runtime_context: mcp_runtime_context,
                available_environment_ids,
                inputs: McpRuntimeInputs::new(
                    contributor_config,
                    configured_base,
                    store_mode,
                    keyring_backend_kind,
                    contributors_revision,
                ),
            },
            elicitation_reviewer,
            McpRefreshMode::ReuseUnchanged,
        )
        .await
    }

    pub(crate) async fn resolve_selected_capability_roots_for_step(
        &self,
        environments: &TurnEnvironmentSnapshot,
    ) -> Vec<ResolvedSelectedCapabilityRoot> {
        self.services
            .turn_environments
            .environment_manager()
            .resolve_selected_capability_roots(
                &self.services.selected_capability_roots,
                &environments.captured_environments(),
            )
            .await
    }

    pub(crate) fn mcp_elicitation_reviewer(self: &Arc<Self>) -> ElicitationReviewerHandle {
        Arc::new(GuardianMcpElicitationReviewer::new(self))
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

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP runtime refresh and publication must remain serialized"
    )]
    async fn refresh_mcp_servers_inner(
        &self,
        turn_context: &TurnContext,
        source: McpRefreshSource,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) {
        let _refresh_guard = self.services.mcp_refresh_lock.lock().await;
        let contributor_config = self.mcp_source_config().await;
        let (configured_base, store_mode, keyring_backend_kind) = match source {
            McpRefreshSource::Independent {
                configured_base,
                store_mode,
                keyring_backend_kind,
            } => (configured_base, store_mode, keyring_backend_kind),
            McpRefreshSource::SessionConfig {
                contributor_config: expected,
                configured_base,
                store_mode,
                keyring_backend_kind,
            } if Arc::ptr_eq(&expected, &contributor_config) => {
                (configured_base, store_mode, keyring_backend_kind)
            }
            McpRefreshSource::SessionConfig { .. } => (
                self.configured_mcp_base(contributor_config.as_ref()).await,
                contributor_config.mcp_oauth_credentials_store_mode,
                contributor_config.auth_keyring_backend_kind(),
            ),
        };
        let available_environment_ids = self
            .services
            .latest_mcp_runtime()
            .available_environment_ids()
            .to_vec();
        let (mcp_config, contributors_revision) = self
            .services
            .mcp_manager
            .refresh_runtime_config_for_step_with_revision(
                contributor_config.as_ref(),
                &self.services.mcp_thread_init,
                &self.services.thread_extension_data,
                &available_environment_ids,
                &configured_base,
            )
            .await;
        let mcp_runtime_context = Self::mcp_runtime_context_for_environments(
            self.services.turn_environments.environment_manager(),
            &turn_context.environments,
            {
                #[allow(deprecated)]
                turn_context.cwd.to_path_buf()
            },
        );
        self.replace_mcp_servers(
            turn_context,
            McpRuntimeReplacement {
                mcp_config,
                runtime_context: mcp_runtime_context,
                available_environment_ids,
                inputs: McpRuntimeInputs::new(
                    contributor_config,
                    configured_base,
                    store_mode,
                    keyring_backend_kind,
                    contributors_revision,
                ),
            },
            elicitation_reviewer,
            McpRefreshMode::Restart,
        )
        .await;
    }

    async fn replace_mcp_servers(
        &self,
        turn_context: &TurnContext,
        replacement: McpRuntimeReplacement,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
        refresh_mode: McpRefreshMode,
    ) -> Arc<McpRuntimeSnapshot> {
        let McpRuntimeReplacement {
            mcp_config,
            runtime_context,
            available_environment_ids,
            inputs,
        } = replacement;
        let (auth, auth_revision) = self.services.auth_manager.auth_with_revision().await;
        let tool_plugin_provenance = codex_mcp::tool_plugin_provenance(&mcp_config);
        let mcp_servers = codex_mcp::effective_mcp_servers(&mcp_config);
        let auth_statuses = compute_auth_statuses(
            mcp_servers.iter(),
            inputs.store_mode,
            inputs.keyring_backend_kind,
            auth.as_ref(),
            &runtime_context,
        )
        .await;
        let current_runtime = self.services.latest_mcp_runtime();
        let current_manager = current_runtime.manager();
        let mcp_startup_cancellation_token = {
            let mut guard = self.services.mcp_startup_cancellation_token.lock().await;
            // The previous runtime owns the old token and may still be serving an in-flight step.
            // Its manager cancels that token when the last runtime handle is dropped.
            let cancellation_token = CancellationToken::new();
            *guard = cancellation_token.clone();
            cancellation_token
        };
        let refresh = match refresh_mode {
            McpRefreshMode::Restart => {
                McpConnectionRefresh::RestartPreservingState(current_manager)
            }
            McpRefreshMode::ReuseUnchanged => McpConnectionRefresh::ReuseUnchanged(current_manager),
        };
        let refreshed_manager = McpConnectionManager::new_with_refresh(
            &mcp_servers,
            McpConnectionManagerInput {
                store_mode: inputs.store_mode,
                keyring_backend_kind: inputs.keyring_backend_kind,
                auth_entries: auth_statuses,
                approval_policy: &turn_context.approval_policy,
                submit_id: turn_context.sub_id.clone(),
                tx_event: self.get_tx_event(),
                startup_cancellation_token: mcp_startup_cancellation_token,
                initial_permission_profile: turn_context.permission_profile(),
                runtime_context: runtime_context.clone(),
                prefix_mcp_tool_names: mcp_config.prefix_mcp_tool_names,
                client_elicitation_capability: mcp_config.client_elicitation_capability.clone(),
                supports_openai_form_elicitation: self
                    .services
                    .supports_openai_form_elicitation
                    .load(std::sync::atomic::Ordering::Relaxed),
                tool_plugin_provenance,
                auth_snapshot: McpAuthSnapshot::new(auth.as_ref(), auth_revision),
                elicitation_reviewer,
            },
            refresh,
        )
        .await;
        refreshed_manager.set_elicitations_auto_deny(current_manager.elicitations_auto_deny());
        self.services.publish_mcp_runtime(
            Arc::new(mcp_config),
            runtime_context,
            available_environment_ids,
            inputs,
            refreshed_manager,
        )
    }

    #[cfg(test)]
    pub(crate) async fn refresh_mcp_servers_if_contributions_changed(
        &self,
        turn_context: &TurnContext,
    ) {
        let environments = if turn_context
            .config
            .features
            .enabled(Feature::DeferredExecutor)
        {
            self.services.turn_environments.snapshot().await
        } else {
            turn_context.environments.clone()
        };
        let selected_capability_roots = self
            .resolve_selected_capability_roots_for_step(&environments)
            .await;
        self.mcp_runtime_for_step(turn_context, &environments, &selected_capability_roots)
            .await;
    }

    pub(crate) async fn refresh_mcp_servers_if_requested(
        &self,
        turn_context: &TurnContext,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) {
        let refresh = { self.pending_mcp_server_refresh.lock().await.take() };
        let Some(refresh) = refresh else {
            return;
        };

        let source = match refresh {
            PendingMcpServerRefresh::Sourceful {
                contributor_config,
                configured_base,
                store_mode,
                keyring_backend_kind,
            } => McpRefreshSource::SessionConfig {
                contributor_config,
                configured_base,
                store_mode,
                keyring_backend_kind,
            },
            PendingMcpServerRefresh::SourceLess(refresh_config) => {
                let McpServerRefreshConfig {
                    mcp_servers,
                    mcp_oauth_credentials_store_mode,
                    auth_keyring_backend_kind,
                } = refresh_config;
                let configured_servers =
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
                let keyring_backend_kind = match serde_json::from_value::<AuthKeyringBackendKind>(
                    auth_keyring_backend_kind,
                ) {
                    Ok(kind) => kind,
                    Err(err) => {
                        warn!("failed to parse MCP auth keyring backend refresh config: {err}");
                        return;
                    }
                };
                McpRefreshSource::Independent {
                    configured_base: McpConfiguredBase::from_servers(configured_servers),
                    store_mode,
                    keyring_backend_kind,
                }
            }
        };

        self.refresh_mcp_servers_inner(turn_context, source, elicitation_reviewer)
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

        let refresh = {
            let runtime = self.services.latest_mcp_runtime();
            let inputs = runtime.inputs();
            PendingMcpServerRefresh::Sourceful {
                contributor_config: Arc::clone(&inputs.contributor_config),
                configured_base: inputs.configured_base.clone(),
                store_mode: inputs.store_mode,
                keyring_backend_kind: inputs.keyring_backend_kind,
            }
        };
        self.services
            .supports_openai_form_elicitation
            .store(supported, std::sync::atomic::Ordering::Relaxed);
        *self.pending_mcp_server_refresh.lock().await = Some(refresh);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) async fn refresh_mcp_servers_now(
        &self,
        turn_context: &TurnContext,
        configured_base: McpConfiguredBase,
        store_mode: OAuthCredentialsStoreMode,
        keyring_backend_kind: AuthKeyringBackendKind,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) {
        self.refresh_mcp_servers_inner(
            turn_context,
            McpRefreshSource::Independent {
                configured_base,
                store_mode,
                keyring_backend_kind,
            },
            elicitation_reviewer,
        )
        .await;
    }

    pub(crate) async fn refresh_mcp_servers_now_from_supplied_config(
        &self,
        turn_context: &TurnContext,
        config: Arc<Config>,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) {
        let configured_base = self.configured_mcp_base(config.as_ref()).await;
        let store_mode = config.mcp_oauth_credentials_store_mode;
        let keyring_backend_kind = config.auth_keyring_backend_kind();
        self.refresh_mcp_servers_inner(
            turn_context,
            McpRefreshSource::Independent {
                configured_base,
                store_mode,
                keyring_backend_kind,
            },
            elicitation_reviewer,
        )
        .await;
    }

    pub(crate) async fn refresh_mcp_servers_now_from_current_config(
        &self,
        turn_context: &TurnContext,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) {
        let contributor_config = self.mcp_source_config().await;
        let configured_base = self.configured_mcp_base(contributor_config.as_ref()).await;
        let store_mode = contributor_config.mcp_oauth_credentials_store_mode;
        let keyring_backend_kind = contributor_config.auth_keyring_backend_kind();
        self.refresh_mcp_servers_inner(
            turn_context,
            McpRefreshSource::SessionConfig {
                contributor_config,
                configured_base,
                store_mode,
                keyring_backend_kind,
            },
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
            .latest_mcp_runtime()
            .manager()
            .cancel_startup();
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

    let approvals_reviewer = request
        .server_runtime_metadata
        .approvals_reviewer()
        .unwrap_or(turn_context.config.approvals_reviewer);
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
    let approval_source = request
        .server_runtime_metadata
        .approval_source_by_name_or_alias(&tool_name)
        .cloned();
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
            approval_source,
            connected_account_email: None,
            tool_title: metadata_owned_string(meta, MCP_ELICITATION_TOOL_TITLE_KEY),
            tool_description: metadata_owned_string(meta, MCP_ELICITATION_TOOL_DESCRIPTION_KEY),
            annotations: None,
        },
    ))
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
