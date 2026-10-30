//! Prepare saved conversations through their owner without mutating the UI on failure.
use super::backend_sessions::BackendSessions;
use super::resume_preparation::ResumeSource;
use super::*;
use codex_app_server_protocol::ThreadItemPayload;

impl App {
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
                    "Wait for the current turn to finish before switching backends.".into(),
                );
                return Ok(AppRunControl::Continue);
            }
            let Some(settings) = backends.settings.get(&owner).cloned() else {
                self.add_session_picker_error(
                    "This backend has no connection settings. Open the session picker again."
                        .into(),
                );
                return Ok(AppRunControl::Continue);
            };
            let Some(destination_slot) = backends.listening_slot(owner) else {
                self.add_session_picker_error(
                    "This session's backend is unavailable. Open the session picker again.".into(),
                );
                return Ok(AppRunControl::Continue);
            };
            let destination = backends.session_at_mut(destination_slot);
            let bootstrap = match destination.bootstrap(&settings.config).await {
                Ok(bootstrap) => bootstrap,
                Err(error) => {
                    let message = format!("Failed to prepare session backend: {error}");
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
            self.capture_session_inputs().await;
            for channel in self.thread_event_channels.values_mut() {
                channel.backend_id.get_or_insert(outgoing);
            }
            backends.settings.insert(outgoing, self.backend_settings());
            self.detach_current_thread_for_navigation(
                backends.session_at_mut(displayed_session),
                Some(target.thread_id),
            )
            .await;
            let destination = backends.session_at_mut(destination_slot);
            self.store_active_thread_receiver().await;
            self.clear_active_thread().await;
            self.agents_overview.pending_usage = None;
            self.agents_overview.usage_disabled = false;
            self.app_event_tx = AppEventSender::new(self.app_event_tx.app_event_tx.clone());
            self.app_server_target = settings.target;
            self.reconnect = Default::default();
            if let Ok(mut state) = self.agents_overview.view_state.lock() {
                state.connection_notice = None;
            }
            self.rate_limit_hard_stop_generation =
                self.rate_limit_hard_stop_generation.wrapping_add(1);
            self.rate_limit_refresh_state.invalidate_recovery();
            self.account_email_request_id = None;
            self.file_search.on_user_query(String::new());
            self.bind_backend_workspace(destination, prepared.config.cwd.to_path_buf());
            self.model_catalog = Arc::new(
                ModelCatalog::new(bootstrap.available_models)
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
            let thread_id = target.thread_id;
            if let Some(input) = self.agents_overview.input_states.get_mut(&thread_id) {
                input.provider_error = None;
                for item in prepared.thread.turns.iter().flat_map(Turn::item_payloads) {
                    if let ThreadItemPayload::UserMessage {
                        client_id: Some(id),
                        ..
                    } = item
                    {
                        input.pending_steers.retain(|steer| steer.client_id != *id);
                    }
                }
            }
            let control = self
                .activate_prepared_session(tui, destination, prepared)
                .await?;
            backends.displayed = destination_slot;
            if let Some(mut input) = self.agents_overview.input_states.remove(&thread_id) {
                let blank = self.agents_overview.blank_sessions.contains_key(&thread_id);
                let settings = blank
                    .then(|| input.pending_thread_settings.take())
                    .flatten();
                self.restore_session_draft(thread_id, input, settings).await;
            }
            tui.frame_requester().schedule_frame();
            Ok(control)
        })
    }
}
