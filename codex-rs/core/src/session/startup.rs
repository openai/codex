use super::*;
use codex_mcp::EffectiveMcpServer;
use codex_mcp::McpAuthStatusEntry;

/// Tracks whether a session may start model-driven turn work yet.
///
/// Async subagent startup intentionally creates the child thread before slow
/// MCP initialization finishes. The child therefore needs a small readiness
/// gate so external work can stay queued until the session has the correct
/// tool universe and reconstructed history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SessionStartupState {
    Ready,
    Initializing,
    Failed(String),
}

/// Captures the work deferred until after a session has been registered.
///
/// This keeps the async startup seam explicit: thread creation and
/// `SessionConfigured` happen first, while slow MCP initialization and initial
/// history reconstruction can either complete synchronously or continue in the
/// background for spawned subagents.
pub(super) struct PendingSessionStartup {
    pub(super) initial_history: InitialHistory,
    pub(super) session_start_source: codex_hooks::SessionStartSource,
    pub(super) auth: Option<CodexAuth>,
    pub(super) mcp_servers: HashMap<String, EffectiveMcpServer>,
    pub(super) auth_statuses: HashMap<String, McpAuthStatusEntry>,
    pub(super) base_instructions: String,
}

pub(super) fn async_subagent_startup_enabled(
    config: &Config,
    session_source: &SessionSource,
) -> bool {
    // Only spawned MultiAgentV2 children have the parent-notification and
    // queued-mailbox semantics needed to safely surface delayed startup
    // failures after the thread has been registered.
    config.features.enabled(Feature::MultiAgentV2)
        && config.multi_agent_v2.async_subagent_startup
        && matches!(
            session_source,
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                agent_path: Some(_),
                ..
            })
        )
}

impl Session {
    pub(crate) fn startup_state(&self) -> SessionStartupState {
        self.startup_state.borrow().clone()
    }

    pub(crate) fn startup_ready(&self) -> bool {
        matches!(self.startup_state(), SessionStartupState::Ready)
    }

    /// Starts any deferred startup work after the thread has been registered.
    pub(crate) async fn start_deferred_startup_if_needed(self: &Arc<Self>) {
        let startup = {
            let mut deferred_startup = self.deferred_startup.lock().await;
            deferred_startup.take()
        };
        if let Some(startup) = startup {
            // Only one caller should ever observe pending startup work, but the
            // explicit take keeps repeated calls harmless and makes the handoff
            // point easy to reason about.
            self.spawn_initial_startup(startup);
        }
    }

    fn spawn_initial_startup(self: &Arc<Self>, startup: PendingSessionStartup) {
        let session = Arc::clone(self);
        let thread_id = session.thread_id();
        tokio::spawn(
            async move {
                if let Err(err) = session.finish_initial_startup(startup).await {
                    session.handle_initial_startup_failure(err).await;
                }
            }
            .instrument(tracing::info_span!(
                "session_init.deferred_startup",
                otel.name = "session_init.deferred_startup",
                thread_id = %thread_id,
            )),
        );
    }

    pub(super) async fn finish_initial_startup(
        self: &Arc<Self>,
        startup: PendingSessionStartup,
    ) -> anyhow::Result<()> {
        let PendingSessionStartup {
            initial_history,
            session_start_source,
            auth,
            mcp_servers,
            auth_statuses,
            base_instructions,
        } = startup;
        let config = self.get_config().await;
        let session_configuration = {
            let state = self.state.lock().await;
            state.session_configuration.clone()
        };

        let mut required_mcp_servers: Vec<String> = mcp_servers
            .iter()
            .filter(|(_, server)| server.enabled() && server.required())
            .map(|(name, _)| name.clone())
            .collect();
        required_mcp_servers.sort();
        let enabled_mcp_server_count = mcp_servers
            .values()
            .filter(|server| server.enabled())
            .count();
        let required_mcp_server_count = required_mcp_servers.len();
        let tool_plugin_provenance = self
            .services
            .mcp_manager
            .tool_plugin_provenance(config.as_ref())
            .await;
        let host_owned_codex_apps_enabled = config.features.apps_enabled_for_auth(
            auth.as_ref()
                .is_some_and(codex_login::CodexAuth::uses_codex_backend),
        );
        let client_elicitation_capability = if config.features.enabled(Feature::AuthElicitation) {
            ElicitationCapability {
                form: Some(FormElicitationCapability::default()),
                url: Some(UrlElicitationCapability::default()),
            }
        } else {
            ElicitationCapability::default()
        };
        {
            let mut cancel_guard = self.services.mcp_startup_cancellation_token.lock().await;
            cancel_guard.cancel();
            *cancel_guard = CancellationToken::new();
        }
        let turn_environment = crate::environment_selection::resolve_environment_selections(
            self.services.environment_manager.as_ref(),
            &session_configuration.environments,
        )
        .map_err(|err| {
            CodexErr::InvalidRequest(err.to_string().replace(
                "unknown turn environment id",
                "unknown stored MCP environment id",
            ))
        })?
        .primary()
        .cloned();
        let mcp_runtime_environment = match turn_environment {
            Some(turn_environment) => McpRuntimeEnvironment::new(
                Some(Arc::clone(&turn_environment.environment)),
                self.services.environment_manager.try_local_environment(),
                turn_environment.cwd.to_path_buf(),
            ),
            None => McpRuntimeEnvironment::new(
                self.services.environment_manager.default_or_local_environment(),
                self.services.environment_manager.try_local_environment(),
                session_configuration.cwd.to_path_buf(),
            ),
        };
        let (mcp_connection_manager, cancel_token) = McpConnectionManager::new(
            &mcp_servers,
            config.mcp_oauth_credentials_store_mode,
            auth_statuses,
            &session_configuration.approval_policy,
            INITIAL_SUBMIT_ID.to_owned(),
            self.get_tx_event(),
            session_configuration.permission_profile(),
            mcp_runtime_environment,
            config.codex_home.to_path_buf(),
            codex_apps_tools_cache_key(auth.as_ref()),
            host_owned_codex_apps_enabled,
            client_elicitation_capability,
            tool_plugin_provenance,
            auth.as_ref(),
            Some(self.mcp_elicitation_reviewer()),
        )
        .instrument(tracing::info_span!(
            "session_init.mcp_manager_init",
            otel.name = "session_init.mcp_manager_init",
            session_init.enabled_mcp_server_count = enabled_mcp_server_count,
            session_init.required_mcp_server_count = required_mcp_server_count,
        ))
        .await;
        {
            let mut cancel_guard = self.services.mcp_startup_cancellation_token.lock().await;
            if cancel_guard.is_cancelled() {
                cancel_token.cancel();
            }
            *cancel_guard = cancel_token;
        }
        if !required_mcp_servers.is_empty() {
            let failures = mcp_connection_manager
                .required_startup_failures(&required_mcp_servers)
                .instrument(tracing::info_span!(
                    "session_init.required_mcp_wait",
                    otel.name = "session_init.required_mcp_wait",
                    session_init.required_mcp_server_count = required_mcp_server_count,
                ))
                .await;
            if !failures.is_empty() {
                let details = failures
                    .iter()
                    .map(|failure| format!("{}: {}", failure.server, failure.error))
                    .collect::<Vec<_>>()
                    .join("; ");
                anyhow::bail!("required MCP servers failed to initialize: {details}");
            }
        }
        {
            // Publish the live manager only after the required-server gate has
            // settled so the rest of the session never observes a half-ready
            // manager through the shared RwLock.
            let mut manager_guard = self.services.mcp_connection_manager.write().await;
            *manager_guard = mcp_connection_manager;
        }

        self.schedule_startup_prewarm(base_instructions).await;
        // `record_initial_history` can emit events, so keep it after
        // `SessionConfigured` regardless of whether startup is synchronous or
        // deferred.
        Box::pin(self.record_initial_history(initial_history)).await;
        {
            let mut state = self.state.lock().await;
            state.set_pending_session_start_source(Some(session_start_source));
        }

        if matches!(self.agent_status.borrow().clone(), AgentStatus::Shutdown) {
            return Ok(());
        }

        self.startup_state.send_replace(SessionStartupState::Ready);
        self.maybe_start_turn_for_pending_work().await;
        Ok(())
    }

    async fn handle_initial_startup_failure(self: &Arc<Self>, error: anyhow::Error) {
        if matches!(self.agent_status.borrow().clone(), AgentStatus::Shutdown) {
            return;
        }

        let message = error.to_string();
        self.startup_state
            .send_replace(SessionStartupState::Failed(message.clone()));
        self.send_event_raw(Event {
            id: INITIAL_SUBMIT_ID.to_owned(),
            msg: EventMsg::Error(ErrorEvent {
                message: message.clone(),
                codex_error_info: Some(CodexErrorInfo::Other),
            }),
        })
        .await;
        self.maybe_notify_parent_of_startup_failure(AgentStatus::Errored(message))
            .await;
    }

    #[cfg(test)]
    pub(crate) fn set_startup_state_for_tests(&self, state: SessionStartupState) {
        self.startup_state.send_replace(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigBuilder;
    use codex_protocol::AgentPath;

    #[tokio::test]
    async fn async_subagent_startup_requires_a_multi_agent_v2_pathful_child() {
        let mut config = ConfigBuilder::default()
            .build()
            .await
            .expect("default test config should load");
        let _ = config.features.enable(Feature::MultiAgentV2);
        config.multi_agent_v2.async_subagent_startup = true;

        assert!(async_subagent_startup_enabled(
            &config,
            &SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: ThreadId::new(),
                depth: 1,
                agent_path: Some(AgentPath::root().join("worker").expect("worker path")),
                agent_nickname: None,
                agent_role: Some("explorer".to_string()),
            })
        ));

        assert!(!async_subagent_startup_enabled(
            &config,
            &SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: ThreadId::new(),
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: Some("explorer".to_string()),
            })
        ));
    }
}
