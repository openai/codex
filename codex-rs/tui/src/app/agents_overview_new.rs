//! Creates empty sessions from the command center without interrupting other agents.

use super::*;

impl App {
    pub(in crate::app) async fn new_agents_overview_session(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        cwd: Option<AbsolutePathBuf>,
    ) -> Result<AppRunControl> {
        if self.reconnect.offline || self.windows_sandbox_blocks_thread_switch() {
            return Ok(AppRunControl::Continue);
        }
        let Some((config, remote_cwd)) = self
            .agents_overview_session_config(tui, app_server, cwd)
            .await
        else {
            return Ok(AppRunControl::Continue);
        };
        let selected_profile = self.chat_widget.thread_id().and_then(|thread_id| {
            let source = self.chat_widget.config_ref();
            let active = source.permissions.active_permission_profile()?;
            (self
                .agents_overview
                .selected_permission_profiles
                .get(&thread_id)
                == Some(&active.id))
            .then(|| PermissionProfileSelection {
                profile_id: active.id.clone(),
                approval_policy: Some(source.permissions.approval_policy.value().into()),
                approvals_reviewer: Some(source.approvals_reviewer),
                display_label: active.id,
            })
        });
        let started = match app_server
            .start_thread_with_session_start_source(
                &crate::local_settings::LocalSettings::from(&config),
                &config,
                /*session_start_source*/ None,
                remote_cwd.as_deref(),
                selected_profile.as_ref(),
            )
            .await
        {
            Ok(started) => started,
            Err(error) => {
                self.add_agents_overview_error(format!("Failed to start session: {error}"));
                return Ok(AppRunControl::Continue);
            }
        };
        let thread_id = started.session.thread_id;
        if let Some(selected) = selected_profile {
            self.agents_overview
                .selected_permission_profiles
                .insert(thread_id, selected.profile_id);
        }
        self.agents_overview
            .blank_sessions
            .insert(thread_id, started.clone());
        // Preserve running agents and unsent input without sending an initial turn.
        let control = self
            .attach_agents_overview_thread(tui, app_server, thread_id, Some((config, started)))
            .await?;
        if self.current_displayed_thread_id() != Some(thread_id) {
            self.agents_overview.blank_sessions.remove(&thread_id);
            let _ = app_server.thread_unsubscribe(thread_id).await;
        }
        Ok(control)
    }
}
