//! In-app session picker transitions, preserving per-thread input and the view on cancellation.

use super::*;
use crate::chatwidget::ThreadInputStateRestoreMode;

impl App {
    pub(super) async fn open_resume_picker(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
    ) -> Result<AppRunControl> {
        // Keep embedded-server initialization on its own runtime task as well as opening the
        // picker on a fresh event-loop iteration; its auth manager needs more stack headroom.
        let picker_config = self.config.clone();
        let picker_target = self.app_server_target.clone();
        let picker_cli_kv_overrides = self.cli_kv_overrides.clone();
        let picker_loader_overrides = self.loader_overrides.clone();
        let picker_state_db = self.state_db.clone();
        let picker_environment_manager = Arc::clone(&self.environment_manager);
        let picker_app_server = tokio::spawn(async move {
            crate::start_app_server_for_picker(
                &picker_config,
                &picker_target,
                picker_cli_kv_overrides,
                picker_loader_overrides,
                picker_state_db,
                picker_environment_manager,
            )
            .await
        })
        .await;
        let picker_app_server = match picker_app_server {
            Ok(result) => result,
            Err(err) => Err(err.into()),
        };
        let picker_app_server = match picker_app_server {
            Ok(app_server) => app_server,
            Err(err) => {
                self.add_session_picker_error(format!("Failed to start TUI session picker: {err}"));
                self.chat_widget.maybe_send_next_queued_input();
                return Ok(AppRunControl::Continue);
            }
        };
        let selection =
            crate::resume_picker::run_resume_picker_from_existing_session_with_app_server(
                crate::uses_remote_workspace_or_environment(
                    &self.app_server_target,
                    &self.environment_manager,
                ),
                tui,
                &self.config,
                &self.local_settings,
                /*show_all*/ false,
                /*include_non_interactive*/ false,
                picker_app_server,
                app_server.request_handle(),
                self.primary_thread_id
                    .or(self.current_displayed_thread_id()),
            )
            .await;
        match selection {
            Ok(selection) => {
                self.apply_resume_picker_selection(tui, app_server, selection)
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
                let thread_id = target_session.thread_id;
                let switching_threads = self.current_displayed_thread_id() != Some(thread_id);
                if switching_threads {
                    for (id, channel) in &self.thread_event_channels {
                        if let Some(input_state) = channel.store.lock().await.input_state.clone() {
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
                if let AppRunControl::Exit(reason) = self
                    .resume_target_session(tui, app_server, target_session)
                    .await?
                {
                    return Ok(AppRunControl::Exit(reason));
                }
                if switching_threads
                    && self.current_displayed_thread_id() == Some(thread_id)
                    && let Some(input_state) = self.agents_overview.input_states.remove(&thread_id)
                {
                    let preserve_in_flight_turn =
                        self.active_turn_id_for_thread(thread_id).await.is_some();
                    self.chat_widget.restore_thread_input_state(
                        Some(input_state),
                        ThreadInputStateRestoreMode {
                            preserve_in_flight_turn,
                        },
                    );
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
