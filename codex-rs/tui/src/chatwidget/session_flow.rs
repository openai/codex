//! Session configuration and thread-header orchestration for `ChatWidget`.

use super::*;

impl ChatWidget {
    fn on_session_configured_with_display_and_fork_parent_title(
        &mut self,
        session: ThreadSessionState,
        display: SessionConfiguredDisplay,
        fork_parent_title: Option<String>,
    ) {
        self.transcript.reset_copy_history();
        let history_metadata = session.message_history.unwrap_or_default();
        self.bottom_pane.set_history_metadata(
            session.thread_id,
            history_metadata.log_id,
            history_metadata.entry_count,
        );
        self.set_skills(/*skills*/ None);
        self.session_network_proxy = session.network_proxy.clone();
        let previous_thread_id = self.thread_id;
        self.thread_id = Some(session.thread_id);
        if previous_thread_id != self.thread_id {
            self.review.recent_auto_review_denials = RecentAutoReviewDenials::default();
        }
        self.refresh_plan_mode_nudge();
        self.turn_lifecycle.reset_thread();
        self.thread_name = session.thread_name.clone();
        self.current_goal_status_indicator = None;
        self.current_goal_status = None;
        self.update_collaboration_mode_indicator();
        self.forked_from = session.forked_from_id;
        self.current_rollout_path = session.rollout_path.clone();
        self.current_cwd = Some(session.cwd.to_path_buf());
        self.config.cwd = session.cwd.clone();
        let runtime_workspace_roots = session.runtime_workspace_roots.clone();
        self.config.workspace_roots = runtime_workspace_roots.clone();
        self.config
            .permissions
            .set_workspace_roots(runtime_workspace_roots);
        self.effective_service_tier = session.service_tier.clone();
        if let Err(err) = self
            .config
            .permissions
            .approval_policy
            .set(session.approval_policy.to_core())
        {
            tracing::warn!(%err, "failed to sync approval_policy from SessionConfigured");
            self.config.permissions.approval_policy =
                Constrained::allow_only(session.approval_policy.to_core());
        }
        let permission_snapshot = PermissionProfileSnapshot::from_session_snapshot(
            session.permission_profile.clone(),
            session.active_permission_profile.clone(),
        );
        let permission_sync = self
            .config
            .permissions
            .set_permission_profile_from_session_snapshot(permission_snapshot.clone());
        if let Err(err) = permission_sync {
            tracing::warn!(%err, "failed to sync permissions from SessionConfigured");
            if let Err(replace_err) = self
                .config
                .permissions
                .replace_permission_profile_from_session_snapshot(permission_snapshot)
            {
                tracing::error!(
                    %replace_err,
                    "failed to replace permissions from SessionConfigured after constraint fallback"
                );
            }
        }
        self.config.approvals_reviewer = session.approvals_reviewer;
        self.status_line_project_root_name_cache = None;
        let forked_from_id = session.forked_from_id;
        let model_for_header = session.model.clone();
        self.session_header.set_model(&model_for_header);
        self.current_collaboration_mode = self.current_collaboration_mode.with_updates(
            Some(model_for_header.clone()),
            Some(session.reasoning_effort),
            /*developer_instructions*/ None,
        );
        if let Some(mask) = self.active_collaboration_mask.as_mut() {
            mask.model = Some(model_for_header.clone());
            mask.reasoning_effort = Some(session.reasoning_effort);
        }
        self.refresh_model_display();
        self.refresh_status_surfaces();
        self.sync_service_tier_commands();
        self.sync_personality_command_enabled();
        self.sync_plugins_command_enabled();
        self.sync_goal_command_enabled();
        self.refresh_plugin_mentions();
        if display == SessionConfiguredDisplay::Normal {
            let startup_tooltip_override = self.startup_tooltip_override.take();
            let show_fast_status = self
                .should_show_fast_status(&model_for_header, self.effective_service_tier.as_deref());
            let session_info_cell = history_cell::new_session_info(
                &self.config,
                &model_for_header,
                &session,
                self.show_welcome_banner,
                startup_tooltip_override,
                self.plan_type,
                show_fast_status,
            );
            self.apply_session_info_cell(session_info_cell);
        } else if self
            .transcript
            .active_cell
            .as_ref()
            .is_some_and(|cell| cell.as_any().is::<history_cell::SessionHeaderHistoryCell>())
        {
            self.transcript.active_cell = None;
            self.bump_active_cell_revision();
        }
        self.transcript.saw_copy_source_this_turn = false;
        self.refresh_skills_for_current_cwd(/*force_reload*/ true);
        if self.connectors_enabled() {
            self.prefetch_connectors();
        }
        if let Some(user_message) = self.initial_user_message.take() {
            if self.suppress_initial_user_message_submit {
                self.initial_user_message = Some(user_message);
            } else {
                self.submit_user_message(user_message);
            }
        }
        if display == SessionConfiguredDisplay::Normal
            && let Some(forked_from_id) = forked_from_id
        {
            self.emit_forked_thread_event(forked_from_id, fork_parent_title);
        }
        if !self.suppress_session_configured_redraw {
            self.request_redraw();
        }
    }

    pub(crate) fn handle_thread_session(&mut self, session: ThreadSessionState) {
        self.instruction_source_paths = session.instruction_source_paths.clone();
        let fork_parent_title = session.fork_parent_title.clone();
        self.on_session_configured_with_display_and_fork_parent_title(
            session,
            SessionConfiguredDisplay::Normal,
            fork_parent_title,
        );
    }

    pub(crate) fn handle_thread_session_quiet(&mut self, session: ThreadSessionState) {
        self.instruction_source_paths = session.instruction_source_paths.clone();
        self.on_session_configured_with_display_and_fork_parent_title(
            session,
            SessionConfiguredDisplay::Quiet,
            /*fork_parent_title*/ None,
        );
    }

    pub(crate) fn handle_side_thread_session(&mut self, session: ThreadSessionState) {
        self.instruction_source_paths = session.instruction_source_paths.clone();
        let fork_parent_title = session.fork_parent_title.clone();
        self.on_session_configured_with_display_and_fork_parent_title(
            session,
            SessionConfiguredDisplay::SideConversation,
            fork_parent_title,
        );
    }

    pub(super) fn emit_forked_thread_event(
        &mut self,
        forked_from_id: ThreadId,
        fork_parent_title: Option<String>,
    ) {
        let forked_from_id_text = forked_from_id.to_string();
        let line: Line<'static> = if let Some(name) = fork_parent_title
            && !name.trim().is_empty()
        {
            vec![
                "• ".dim(),
                "Thread forked from ".into(),
                name.cyan(),
                " (".into(),
                forked_from_id_text.cyan(),
                ")".into(),
            ]
            .into()
        } else {
            vec![
                "• ".dim(),
                "Thread forked from ".into(),
                forked_from_id_text.cyan(),
            ]
            .into()
        };
        self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
            PlainHistoryCell::new(vec![line]),
        )));
    }

    pub(super) fn on_thread_name_updated(
        &mut self,
        thread_id: ThreadId,
        thread_name: Option<String>,
    ) {
        if self.thread_id == Some(thread_id) {
            if let Some(name) = thread_name.as_deref() {
                let cell = Self::rename_confirmation_cell(name, self.thread_id);
                self.add_boxed_history(Box::new(cell));
            }
            self.thread_name = thread_name;
            self.refresh_status_surfaces();
            self.request_redraw();
            self.maybe_send_next_queued_input();
        }
    }

    pub(super) fn set_skills(&mut self, skills: Option<Vec<SkillMetadata>>) {
        self.bottom_pane.set_skills(skills);
    }
}
