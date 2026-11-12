//! In-app session picker transitions, preserving per-thread input and the view on cancellation.

use super::*;

impl App {
    pub(super) async fn capture_session_inputs(&mut self, owner: crate::backend_id::BackendId) {
        for (id, channel) in &self.thread_event_channels {
            if channel.backend_id.is_none_or(|backend| backend == owner)
                && let Some(input_state) = channel.store.lock().await.input_state.clone()
            {
                self.agents_overview.input_states.insert(*id, input_state);
            }
        }
        if let Some(current_thread_id) = self.current_displayed_thread_id()
            && let Some(input_state) = self.chat_widget.capture_thread_input_state()
        {
            self.agents_overview
                .input_states
                .insert(current_thread_id, input_state);
        }
    }

    pub(super) async fn reconcile_saved_session_input(
        &self,
        thread_id: ThreadId,
        input: &mut ThreadInputState,
    ) {
        if (!input.pending_steers.is_empty() || input.has_unconfirmed_messages())
            && let Some(channel) = self.thread_event_channels.get(&thread_id)
        {
            let store = channel.store.lock().await;
            let confirmed =
                super::thread_events::confirmed_user_message_ids(&store.turns, store.buffer.iter());
            input.reconcile_confirmed_messages(&confirmed);
        }
    }

    pub(super) async fn open_resume_picker_for_backends(
        &mut self,
        tui: &mut tui::Tui,
        backends: &mut super::backend_sessions::BackendSessions,
    ) -> Result<AppRunControl> {
        let displayed_session = backends.displayed;
        let displayed_owner = backends.displayed_owner();
        let selection =
            crate::resume_picker::run_resume_picker_from_existing_session_with_app_servers(
                crate::uses_remote_workspace_or_environment(
                    &self.app_server_target,
                    &self.environment_manager,
                ),
                tui,
                &self.config,
                &self.local_settings,
                /*show_all*/ false,
                /*include_non_interactive*/ false,
                crate::resume_picker::ResumePickerAppServers::new(
                    backends.picker_views(),
                    displayed_owner,
                ),
                self.primary_thread_id
                    .or(self.current_displayed_thread_id()),
            )
            .await;
        match selection {
            Ok(selection) => {
                self.apply_resume_picker_selection(
                    tui,
                    backends.session_at_mut(displayed_session),
                    selection,
                )
                .await
            }
            Err(err) => {
                self.add_session_picker_error(format!("Failed to open session picker: {err}"));
                self.chat_widget.maybe_send_next_queued_input();
                Ok(AppRunControl::Continue)
            }
        }
    }

    pub(super) async fn apply_resume_picker_selection(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        selection: SessionSelection,
    ) -> Result<AppRunControl> {
        match selection {
            SessionSelection::Resume(target_session) => {
                if target_session.backend_id != app_server.backend_id() {
                    self.app_event_tx
                        .send(AppEvent::ResumeBackendSession(target_session));
                    return Ok(AppRunControl::Continue);
                }
                let thread_id = target_session.thread_id;
                if let AppRunControl::Exit(reason) = self
                    .resume_target_session(tui, app_server, target_session)
                    .await?
                {
                    return Ok(AppRunControl::Exit(reason));
                }
                if self.active_thread_id == Some(thread_id)
                    && self
                        .chat_widget
                        .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
                        .is_some()
                {
                    if let Ok(mut state) = self.agents_overview.view_state.lock() {
                        state.completion = Some(crate::bottom_pane::ViewCompletion::Accepted);
                    }
                    self.chat_widget.pre_draw_tick();
                }
            }
            SessionSelection::Exit
            | SessionSelection::StartFresh
            | SessionSelection::AgentsOverview => {
                self.refresh_in_memory_config_from_disk_best_effort("closing the session picker")
                    .await;
            }
            SessionSelection::Fork(_) => {}
        }

        self.chat_widget.maybe_send_next_queued_input();
        // Leaving alt-screen may blank the inline viewport; force a redraw either way.
        tui.frame_requester().schedule_frame();
        Ok(AppRunControl::Continue)
    }

    pub(super) fn add_session_picker_error(&mut self, message: String) {
        if self
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .is_some()
        {
            self.chat_widget.show_selection_view(SelectionViewParams {
                title: Some("Unable to resume session".to_string()),
                subtitle: Some(message.clone()),
                items: vec![SelectionItem {
                    name: "Return to command center".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                }],
                ..SelectionViewParams::picker()
            });
        }
        self.chat_widget.add_error_message(message);
    }
}
