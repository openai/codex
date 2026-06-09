use super::*;
use crate::environment_selection::ResolvedTurnEnvironments;
use crate::session::session::BaseInstructionsOrigin;
use std::sync::atomic::Ordering;

impl Session {
    pub(crate) async fn ensure_model_catalog_ready(&self) {
        if !self.model_catalog_refresh_enabled {
            return;
        }

        self.model_catalog_ready
            .get_or_init(|| async {
                self.services
                    .models_manager
                    .list_models(codex_models_manager::manager::RefreshStrategy::OnlineIfUncached)
                    .await;

                let refreshed_default_model = self.initial_default_model.as_ref().map(|_| {
                    self.services
                        .models_manager
                        .get_default_model_from_current_catalog(/*configured_model*/ None)
                });

                let model_changed = {
                    let mut state = self.state.lock().await;
                    if let (Some(initial_model), Some(refreshed_model)) =
                        (&self.initial_default_model, refreshed_default_model)
                        && state.session_configuration.collaboration_mode.model() == initial_model
                        && refreshed_model != *initial_model
                    {
                        state
                            .session_configuration
                            .collaboration_mode
                            .settings
                            .model = refreshed_model;
                        true
                    } else {
                        false
                    }
                };
                if model_changed {
                    self.send_event_raw(Event {
                        id: INITIAL_SUBMIT_ID.to_owned(),
                        msg: super::handlers::thread_settings_applied_event(self).await,
                    })
                    .await;
                }
                loop {
                    let (model, personality, config) = {
                        let state = self.state.lock().await;
                        (
                            state
                                .session_configuration
                                .collaboration_mode
                                .model()
                                .to_string(),
                            state.session_configuration.personality,
                            Self::build_effective_session_config(&state.session_configuration),
                        )
                    };
                    let model_info = self
                        .services
                        .models_manager
                        .get_model_info(&model, &config.to_models_manager_config())
                        .await;
                    let base_instructions = model_info.get_model_instructions(personality);
                    let mut state = self.state.lock().await;
                    if state.session_configuration.collaboration_mode.model() != model
                        || state.session_configuration.personality != personality
                    {
                        continue;
                    }
                    state.session_configuration.service_tier = crate::session::get_service_tier(
                        state.session_configuration.service_tier.clone(),
                        config.features.enabled(codex_features::Feature::FastMode),
                        &model_info,
                    );
                    if matches!(
                        self.base_instructions_origin,
                        BaseInstructionsOrigin::ModelDerived
                    ) {
                        state.session_configuration.base_instructions = base_instructions;
                    }
                    break;
                }
                let session_configuration = self.state.lock().await.session_configuration.clone();
                self.spawn_config_lock_export(session_configuration).await;
            })
            .await;
    }

    pub(super) async fn ensure_deferred_initialization_ready(&self) {
        self.deferred_initialization_ready
            .get_or_init(|| async {
                self.ensure_model_catalog_ready().await;
                self.initialize_plugins_and_skills().await;
                let initial_history = self.initial_history.lock().await.take();
                if let Some(initial_history) = initial_history {
                    self.record_initial_history(initial_history).await;
                }
            })
            .await;
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the session-owned MCP manager must remain exclusively locked while adding servers"
    )]
    async fn initialize_plugins_and_skills(&self) {
        let config = self.get_config().await;
        let plugin_outcome = self
            .services
            .plugins_manager
            .plugins_for_config(&config.plugins_config_input())
            .await;
        let effective_skill_roots = plugin_outcome.effective_plugin_skill_roots();
        let skills_input = skills_load_input_from_config(config.as_ref(), effective_skill_roots);
        let environments = {
            let state = self.state.lock().await;
            state
                .session_configuration
                .environment_selections()
                .to_vec()
        };
        let resolved_environments = crate::environment_selection::resolve_environment_selections(
            self.services.environment_manager.as_ref(),
            &environments,
        )
        .ok();
        let fs = resolved_environments
            .as_ref()
            .and_then(ResolvedTurnEnvironments::primary_filesystem);
        let skills = self
            .services
            .skills_manager
            .skills_for_config(&skills_input, fs)
            .await;
        for err in skills.errors {
            error!(
                "failed to load skill {}: {}",
                err.path.display(),
                err.message
            );
        }

        let hooks = super::build_hooks_with_plugins(
            config.as_ref(),
            self.services.user_shell.as_ref(),
            &plugin_outcome,
        );
        for warning in hooks.startup_warnings() {
            self.send_event_raw(Event {
                id: INITIAL_SUBMIT_ID.to_owned(),
                msg: EventMsg::Warning(WarningEvent {
                    message: warning.clone(),
                }),
            })
            .await;
        }
        self.services.hooks.store(Arc::new(hooks));

        let mcp_config = config.to_mcp_config_with_plugins(&plugin_outcome);
        let auth = self.services.auth_manager.auth().await;
        let mcp_servers = codex_mcp::effective_mcp_servers(&mcp_config, auth.as_ref());
        let tool_plugin_provenance = codex_mcp::tool_plugin_provenance(&mcp_config);
        let runtime_context =
            match resolved_environments.and_then(|resolved| resolved.primary().cloned()) {
                Some(turn_environment) => McpRuntimeContext::new(
                    Arc::clone(&self.services.environment_manager),
                    turn_environment.cwd.to_path_buf(),
                ),
                None => McpRuntimeContext::new(
                    Arc::clone(&self.services.environment_manager),
                    config.cwd.to_path_buf(),
                ),
            };
        self.services
            .mcp_connection_manager
            .write()
            .await
            .add_servers(
                &mcp_servers,
                mcp_config.mcp_oauth_credentials_store_mode,
                INITIAL_SUBMIT_ID.to_owned(),
                self.get_tx_event(),
                runtime_context,
                config.codex_home.to_path_buf(),
                codex_apps_tools_cache_key(auth.as_ref()),
                mcp_config.client_elicitation_capability,
                tool_plugin_provenance,
                auth.as_ref(),
            )
            .await;
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "the prewarm gate must stay locked until the handle is installed"
    )]
    pub(crate) async fn finish_deferred_initialization(self: &Arc<Self>) {
        self.ensure_deferred_initialization_ready().await;
        let mut prewarm_pending = self.model_catalog_prewarm_pending.lock().await;
        if self.shutdown_requested.load(Ordering::SeqCst) || !*prewarm_pending {
            return;
        }
        *prewarm_pending = false;
        let base_instructions = {
            let state = self.state.lock().await;
            state.session_configuration.base_instructions.clone()
        };
        self.schedule_startup_prewarm(base_instructions).await;
        drop(prewarm_pending);
    }

    pub(super) async fn claim_model_catalog_prewarm(&self) {
        *self.model_catalog_prewarm_pending.lock().await = false;
    }

    pub(crate) async fn shutdown_deferred_initialization(&self) -> anyhow::Result<()> {
        self.shutdown_requested.store(true, Ordering::SeqCst);
        *self.model_catalog_prewarm_pending.lock().await = false;
        let deferred_initialization_task = self.deferred_initialization_task.lock().await.take();
        let task_result = match deferred_initialization_task {
            Some(task) => task
                .await
                .map_err(|err| anyhow::anyhow!("deferred initialization task failed: {err}")),
            None => Ok(()),
        };
        if let Some(prewarm) = self.take_session_startup_prewarm().await {
            prewarm.abort().await;
        }
        let export_result = self.wait_for_config_lock_export().await;
        task_result?;
        export_result
    }
}
