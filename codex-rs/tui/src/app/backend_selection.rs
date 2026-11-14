//! Prepare conversations through their owning backend before replacing the UI.

use super::backend_sessions::BackendSessions;
use super::backend_sessions::SessionSlot;
use super::cloud_switch_wait::ClipboardHandoff;
use super::cloud_switch_wait::finish_failed_cloud_switch;
use super::cloud_switch_wait::wait_for_cloud_switch;
use super::resume_preparation::ResumeSource;
use super::session_lifecycle::PreparedFreshSession;
use super::session_lifecycle::ThreadAttachPresentation;
use super::*;
use crate::startup_draft::StartupDraftPump;
use futures::TryFutureExt;

pub(super) enum NewBackendSession {
    Fresh { name: Option<String> },
    CarryDraft,
}

/// Host inputs for the new-thread preference. A cloud thread's cwd and credentials
/// belong to the executor and must never determine where this preference is saved.
pub(crate) struct BackendPreferenceContext {
    pub(crate) local_cwd: Option<PathBuf>,
    auth: codex_login::AuthConfig,
    project_root_markers: Vec<String>,
}

impl BackendPreferenceContext {
    pub(crate) fn new(config: &Config, local_cwd: Option<PathBuf>) -> Self {
        let project_root_markers = config
            .config_layer_stack
            .layers_high_to_low()
            .filter(|layer| !matches!(layer.name, codex_config::ConfigLayerSource::Project { .. }))
            .find_map(|layer| {
                codex_config::project_root_markers_from_config(&layer.config)
                    .ok()
                    .flatten()
            })
            .unwrap_or_else(codex_config::default_project_root_markers);
        Self {
            local_cwd,
            auth: config.auth_config(),
            project_root_markers,
        }
    }

    async fn scope(&self) -> Option<(String, String)> {
        let cwd = AbsolutePathBuf::from_absolute_path(self.local_cwd.as_deref()?).ok()?;
        let project = codex_config::loader::find_project_root(
            codex_exec_server::LOCAL_FS.as_ref(),
            &cwd,
            &self.project_root_markers,
        )
        .await
        .ok()?;
        let auth = self
            .auth
            .load_auth(/*enable_codex_api_key_env*/ false)
            .await
            .ok()?;
        let user = if let Some(id) = auth
            .as_ref()
            .and_then(codex_login::CodexAuth::get_chatgpt_user_id)
        {
            format!("chatgpt:{id}")
        } else if auth
            .as_ref()
            .is_some_and(codex_login::CodexAuth::is_chatgpt_auth)
        {
            return None;
        } else {
            // A local profile without an account is already private to its CODEX_HOME.
            "local".to_string()
        };
        Some((
            codex_config::loader::project_trust_key(project.as_path()),
            user,
        ))
    }
}

impl App {
    pub(super) fn remember_attached_host_project(&mut self, app_server: &AppServerSession) {
        if !app_server.uses_remote_workspace()
            && !crate::uses_remote_workspace_or_environment(
                &self.app_server_target,
                &self.environment_manager,
            )
        {
            self.backend_preference_context.local_cwd =
                Some(self.chat_widget.config_ref().cwd.to_path_buf());
        }
    }

    pub(super) async fn new_thread_host_preference(&self) -> Option<&str> {
        let defaults = &self.local_settings.tui.default_hosts;
        if defaults.is_empty() {
            return None;
        }
        let (project, user) = self.backend_preference_context.scope().await?;
        defaults.get(&project)?.get(&user).map(String::as_str)
    }

    pub(super) async fn other_new_session_backend(
        &self,
        backends: &BackendSessions,
    ) -> Option<crate::backend_id::BackendId> {
        backends
            .preferred_listening_backend(self.new_thread_host_preference().await)
            .filter(|owner| *owner != backends.displayed_owner())
    }

    pub(super) fn can_cycle_conversation_backend(&self) -> bool {
        self.voice_owner_thread_id().is_none()
            && self.chat_widget.can_choose_new_backend()
            && self
                .current_displayed_thread_id()
                .and_then(|id| self.thread_event_channels.get(&id))
                .and_then(|channel| channel.store.try_lock().ok())
                .is_some_and(|store| store.session.is_some() && store.latest_turn_id.is_none())
    }

    pub(super) async fn start_new_session_on_other_backend(
        &mut self,
        tui: &mut tui::Tui,
        backends: &mut BackendSessions,
        owner: crate::backend_id::BackendId,
        session: NewBackendSession,
    ) -> Result<(), String> {
        let previous_thread = self.current_displayed_thread_id();
        let mut fresh_draft = None;
        let result = Box::pin(self.switch_to_new_backend_session(
            tui,
            backends,
            owner,
            session,
            &mut fresh_draft,
        ))
        .await;
        if let Some(draft) = fresh_draft {
            let opened_new_session = self.current_displayed_thread_id() != previous_thread;
            draft
                .handoff_after_navigation(tui, &mut self.chat_widget, opened_new_session)
                .await;
        }
        result
    }

    async fn switch_to_new_backend_session(
        &mut self,
        tui: &mut tui::Tui,
        backends: &mut BackendSessions,
        owner: crate::backend_id::BackendId,
        session: NewBackendSession,
        fresh_draft: &mut Option<StartupDraftPump>,
    ) -> Result<(), String> {
        let slot = backends
            .listening_slot(owner)
            .ok_or("That connection is unavailable. Choose another and try again.")?;
        if self.windows_sandbox_blocks_thread_switch()
            || self.reject_pending_permission_root_switch()
        {
            return Ok(());
        }
        if self.voice_owner_thread_id().is_some() {
            return Err("End the voice call before switching.".into());
        }
        let displayed_thread = self.current_displayed_thread_id();
        let outgoing_slot = backends.displayed;
        if let Some(thread_id) = displayed_thread
            && self.active_turn_id_for_thread(thread_id).await.is_some()
        {
            return Err("Wait for the current turn to finish before switching.".into());
        }
        let mut settings = backends
            .settings
            .get(&owner)
            .cloned()
            .ok_or("Connection settings are unavailable.")?;
        if settings.target == AppServerTarget::Cloud && self.chat_widget.thread_id().is_some() {
            self.remember_attached_host_project(backends.session_at(outgoing_slot));
        }
        let preference = if matches!(session, NewBackendSession::CarryDraft)
            && let Some(key) = backends.preference_key(slot)
        {
            self.backend_preference_context
                .scope()
                .await
                .map(|(project, user)| (project, user, key.to_owned()))
        } else {
            None
        };
        let user_config_path = self.local_settings.user_config_path.clone();
        let responsive_cloud_switch = settings.target == AppServerTarget::Cloud
            && matches!(session, NewBackendSession::CarryDraft);
        let (name, mut draft) = match session {
            NewBackendSession::Fresh { name } => {
                *fresh_draft = Some(StartupDraftPump::for_new_session(
                    tui,
                    &settings.local_settings,
                    settings.config.cwd.as_path(),
                ));
                (name, None)
            }
            NewBackendSession::CarryDraft => (
                self.chat_widget.thread_name(),
                (!responsive_cloud_switch).then(|| self.chat_widget.composer_draft_snapshot()),
            ),
        };
        let destination = backends.session_at_mut(slot);
        let clipboard_after_bootstrap = if destination.remote_cwd_override().is_some() {
            // An explicit remote directory can open the trust prompt after bootstrap.
            ClipboardHandoff::SettleBeforeNextView
        } else {
            ClipboardHandoff::ContinueEditing
        };
        let bootstrap = destination
            .bootstrap_for_new_thread(&settings.config)
            .map_err(|error| format!("Failed to prepare the new conversation: {error}"));
        let Some(bootstrap) = if responsive_cloud_switch {
            let events = tui.event_stream();
            wait_for_cloud_switch(self, tui, bootstrap, events, clipboard_after_bootstrap).await
        } else {
            StartupDraftPump::run_during_navigation(fresh_draft.as_mut(), tui, bootstrap)
                .await
                .map(Some)
        }?
        else {
            return Ok(());
        };
        let destination_cwd = settings.config.cwd.to_path_buf();
        if let Err(control) = self
            .confirm_directory_trust_for_target(
                tui,
                destination,
                &settings.target,
                &settings.local_settings,
                &mut settings.config,
                &destination_cwd,
                crate::onboarding::DirectoryTrustOptions {
                    cancel: displayed_thread
                        .map(|_| crate::onboarding::TrustCancelAction::CurrentTask),
                    ..Default::default()
                },
                fresh_draft.as_mut(),
            )
            .await
        {
            if responsive_cloud_switch {
                let events = tui.event_stream();
                if finish_failed_cloud_switch(self, tui, events)
                    .await
                    .is_break()
                {
                    return Ok(());
                }
            }
            if matches!(control, AppRunControl::Exit(_)) {
                self.app_event_tx.send(AppEvent::Exit(ExitMode::Immediate));
            }
            return Ok(());
        }
        let loaded = if fresh_draft.is_some() {
            StartupDraftPump::run_during_navigation(
                fresh_draft.as_mut(),
                tui,
                Box::pin(self.load_new_session_config_from(
                    destination,
                    &settings,
                    destination_cwd,
                    /*cloud_switch*/ None,
                )),
            )
            .await
        } else {
            self.load_new_session_config_from(
                destination,
                &settings,
                destination_cwd,
                responsive_cloud_switch.then_some(&mut *tui),
            )
            .await
        };
        let Some((config, local_settings)) =
            loaded.map_err(|error| format!("Failed to read new session defaults: {error}"))?
        else {
            return Ok(());
        };
        if let Some(draft) = fresh_draft.as_mut() {
            draft.apply_settings(&local_settings, config.cwd.as_path());
        }
        let host_project_cwd = self.backend_preference_context.local_cwd.clone();
        let start = destination
            .start_thread_with_session_start_source(
                &local_settings,
                &config,
                /*session_start_source*/ None,
                /*remote_cwd_override*/ None,
                /*selected_profile*/ None,
                host_project_cwd.as_deref(),
            )
            .map_err(|error| format!("Failed to start the new conversation: {error}"));
        let Some(started) = if responsive_cloud_switch {
            let events = tui.event_stream();
            wait_for_cloud_switch(
                self,
                tui,
                start,
                events,
                ClipboardHandoff::SettleBeforeNextView,
            )
            .await
        } else {
            StartupDraftPump::run_during_navigation(fresh_draft.as_mut(), tui, start)
                .await
                .map(Some)
        }?
        else {
            return Ok(());
        };
        if responsive_cloud_switch {
            draft = Some(self.chat_widget.composer_draft_snapshot());
        }
        // Clear before switching so a terminal error leaves the outgoing conversation usable.
        if let Err(error) = tui.clear_for_thread_switch() {
            if responsive_cloud_switch {
                let events = tui.event_stream();
                if finish_failed_cloud_switch(self, tui, events)
                    .await
                    .is_break()
                {
                    return Ok(());
                }
            }
            return Err(format!("Failed to open the new conversation: {error}"));
        }
        let destination_thread = started.session.thread_id;
        let outgoing_has_queue = self.chat_widget.has_queued_follow_up_messages();
        self.switch_backend_context(
            backends,
            slot,
            started.session.thread_id,
            settings.target,
            config.cwd.to_path_buf(),
            bootstrap,
        )
        .await;
        let destination = backends.session_at_mut(slot);
        self.adopt_fresh_session(
            tui,
            destination,
            PreparedFreshSession {
                started,
                config,
                local_settings,
            },
            ThreadAttachPresentation::FreshAfterClear,
            /*initial_user_message*/ None,
            name,
        )
        .await;
        backends.switch_displayed(slot, &mut self.agents_overview.input_states);
        if !backends.session_at_mut(slot).uses_remote_workspace()
            && self.current_displayed_thread_id() == Some(destination_thread)
        {
            let cwd = self.config.cwd.to_path_buf();
            self.review_folder_hooks_after_attach(backends.session_at_mut(slot), cwd)
                .await;
        }
        tui.set_notification_settings(
            self.local_settings.tui.notification_settings.method,
            self.local_settings.tui.notification_settings.condition,
        );
        if let Some(draft) = draft {
            if responsive_cloud_switch {
                self.chat_widget.restore_cloud_switch_draft(draft);
            } else {
                self.chat_widget.restore_startup_draft(draft);
            }
            if let Some(thread_id) = displayed_thread
                && !outgoing_has_queue
                && self.current_displayed_thread_id() == Some(destination_thread)
                && self
                    .agents_overview
                    .blank_sessions
                    .remove(&thread_id)
                    .is_some()
            {
                backends.parked_inputs_mut(outgoing_slot).remove(&thread_id);
                self.remove_agents_overview_thread(thread_id);
                if let Err(error) = backends
                    .session_at_mut(outgoing_slot)
                    .thread_unsubscribe(thread_id)
                    .await
                {
                    tracing::warn!(%error, "failed to unsubscribe replaced blank thread {thread_id}");
                }
                self.discard_thread_local_state(thread_id).await;
            }
        }
        if self.current_displayed_thread_id() == Some(destination_thread)
            && let Some((project, user, key)) = preference
        {
            let result = ConfigEditsBuilder::for_config_path(user_config_path.as_path())
                .with_edits([crate::legacy_core::config::edit::ConfigEdit::SetPath {
                    segments: vec![
                        "tui".into(),
                        "default_hosts".into(),
                        project.clone(),
                        user.clone(),
                    ],
                    value: toml_edit::value(key.clone()),
                }])
                .apply()
                .await;
            match result {
                Ok(()) => {
                    for settings in std::iter::once(&mut self.local_settings).chain(
                        backends
                            .settings
                            .values_mut()
                            .map(|settings| &mut settings.local_settings),
                    ) {
                        settings
                            .tui
                            .default_hosts
                            .entry(project.clone())
                            .or_default()
                            .insert(user.clone(), key.clone());
                    }
                }
                Err(error) => self.chat_widget.add_error_message(format!(
                    "Switched, but could not save the default for this project: {error}"
                )),
            }
        }
        tui.frame_requester().schedule_frame();
        Ok(())
    }

    // Both create and resume call this after the destination thread is ready.
    async fn switch_backend_context(
        &mut self,
        backends: &mut BackendSessions,
        destination_slot: SessionSlot,
        thread_id: ThreadId,
        target: AppServerTarget,
        cwd: PathBuf,
        bootstrap: AppServerBootstrap,
    ) {
        let outgoing = backends.displayed_owner();
        self.saved_openings.abandon();
        self.capture_session_inputs(outgoing).await;
        for channel in self.thread_event_channels.values_mut() {
            channel.backend_id.get_or_insert(outgoing);
        }
        backends.settings.insert(outgoing, self.backend_settings());
        self.detach_current_thread_for_navigation(
            backends.session_at_mut(backends.displayed),
            Some(thread_id),
        )
        .await;
        let destination = backends.session_at_mut(destination_slot);
        self.store_active_thread_receiver().await;
        self.clear_active_thread().await;
        self.agents_overview.search_matches.clear();
        self.agents_overview.pending_usage = None;
        self.reset_agents_overview_usage_for_backend_switch();
        self.agents_overview.usage_disabled = false;
        self.agents_overview.reset_visible_pagination();
        self.app_event_tx = AppEventSender::new(self.app_event_tx.app_event_tx.clone());
        self.app_server_backend_id = Some(destination.backend_id());
        self.app_server_target = target;
        self.reconnect = Default::default();
        if let Ok(mut state) = self.agents_overview.view_state.lock() {
            state.connection_notice = None;
            let destination_id = destination.backend_id();
            state.has_more = self.agents_overview.initialized.contains(&destination_id)
                && self.agents_overview.has_more(destination_id);
            state.restart_history_search();
        }
        self.rate_limit_hard_stop_generation = self.rate_limit_hard_stop_generation.wrapping_add(1);
        self.rate_limit_refresh_state.invalidate_recovery();
        self.account_email_request_id = None;
        self.file_search.on_user_query(String::new());
        self.bind_backend_workspace(destination, cwd);
        self.model_catalog = Arc::new(
            ModelCatalog::new(bootstrap.available_models)
                .for_backend(destination.backend_id(), &mut self.provider_writes)
                .with_collaboration_modes(bootstrap.collaboration_modes),
        );
        self.feedback_audience = bootstrap.feedback_audience;
        self.chat_widget.requires_openai_auth = bootstrap.requires_openai_auth;
        self.chat_widget.invalidate_security_setup();
        self.chat_widget.update_account_state(
            bootstrap.status_account_display,
            bootstrap.plan_type,
            bootstrap.has_chatgpt_account,
            matches!(bootstrap.auth_mode, Some(TelemetryAuthMode::Chatgpt)),
        );
    }

    pub(super) fn resume_backend_session<'a>(
        &'a mut self,
        tui: &'a mut tui::Tui,
        backends: &'a mut BackendSessions,
        target: SessionTarget,
    ) -> futures::future::LocalBoxFuture<'a, Result<AppRunControl>> {
        Box::pin(async move {
            let displayed_session = backends.displayed;
            let owner = target.backend_id;
            let outgoing = backends.displayed_owner();
            if let Some(registered_owner) = self.backend_registry.owner(target.thread_id)
                && let Err(error) = target.ensure_backend(registered_owner)
            {
                self.add_session_picker_error(error.to_string());
                return Ok(AppRunControl::Continue);
            }
            if owner == outgoing {
                return self
                    .apply_resume_picker_selection(
                        tui,
                        backends.session_at_mut(displayed_session),
                        SessionSelection::Resume(target),
                    )
                    .await;
            }
            if self.windows_sandbox_blocks_thread_switch()
                || self.reject_pending_permission_root_switch()
            {
                return Ok(AppRunControl::Continue);
            }
            if let Some(thread_id) = self.current_displayed_thread_id()
                && self.active_turn_id_for_thread(thread_id).await.is_some()
            {
                self.add_session_picker_error(
                    "Wait for the current turn to finish before switching.".into(),
                );
                return Ok(AppRunControl::Continue);
            }
            let Some(settings) = backends.settings.get(&owner).cloned() else {
                self.add_session_picker_error(
                    "Connection settings are unavailable. Open the session picker again.".into(),
                );
                return Ok(AppRunControl::Continue);
            };
            let Some(destination_slot) = backends.listening_slot(owner) else {
                self.add_session_picker_error(
                    "The connection for this conversation is unavailable. Open the session picker again.".into(),
                );
                return Ok(AppRunControl::Continue);
            };
            let destination = backends.session_at_mut(destination_slot);
            let bootstrap = match destination.bootstrap(&settings.config).await {
                Ok(bootstrap) => bootstrap,
                Err(error) => {
                    let message = format!("Failed to prepare the conversation: {error}");
                    self.add_session_picker_error(message);
                    return Ok(AppRunControl::Continue);
                }
            };
            let prepared = match self
                .prepare_target_session(tui, destination, &target, ResumeSource::Backend(&settings))
                .await
            {
                Ok(prepared) => prepared,
                Err(control) => return Ok(control),
            };
            let handoff = self.chat_widget.pause_input_for_resume_handoff();
            let _handoff = self.prepare_resume_handoff(tui, handoff).await?;
            self.switch_backend_context(
                backends,
                destination_slot,
                target.thread_id,
                settings.target,
                prepared.config.cwd.to_path_buf(),
                bootstrap,
            )
            .await;
            backends.switch_displayed(destination_slot, &mut self.agents_overview.input_states);
            let destination = backends.session_at_mut(destination_slot);
            let control = self
                .activate_prepared_session_after_clear(
                    tui,
                    destination,
                    prepared,
                    /*switched_backend*/ true,
                )
                .await?;
            tui.frame_requester().schedule_frame();
            Ok(control)
        })
    }
}
