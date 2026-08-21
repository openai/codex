//! Confirm session-only permission changes before updating local state.

use super::*;
use codex_app_server_protocol::ThreadSettingsUpdateParams;

impl App {
    pub(super) async fn apply_permission_shortcut(
        &mut self,
        app_server: &mut AppServerSession,
        tui: &mut tui::Tui,
        thread_id: ThreadId,
        selection: PermissionProfileSelection,
    ) {
        if self.current_displayed_thread_id() != Some(thread_id)
            || self.chat_widget.thread_id() != Some(thread_id)
        {
            self.chat_widget.complete_permission_shortcut(thread_id);
            return;
        }
        let result: Result<()> = async {
            let active_profile = ActivePermissionProfile::new(selection.profile_id.clone());
            let profile = builtin_permission_profile_for_active_permission_profile(&active_profile)
                .ok_or_else(|| color_eyre::eyre::eyre!("unknown built-in permission profile"))?;
            let mut config = self.chat_widget.config_ref().clone();
            if let Some(policy) = selection.approval_policy {
                config.permissions.approval_policy.set(policy.to_core())?;
            }
            if let Some(reviewer) = selection.approvals_reviewer {
                config.config_layer_stack.requirements().approvals_reviewer.can_set(&reviewer)?;
                config.approvals_reviewer = reviewer;
            }
            config.permissions.set_permission_profile_from_session_snapshot(
                PermissionProfileSnapshot::active(profile, active_profile),
            )?;

            if !app_server.thread_settings_update(ThreadSettingsUpdateParams {
                thread_id: thread_id.to_string(),
                permissions: Some(selection.profile_id),
                approval_policy: selection.approval_policy,
                approvals_reviewer: selection.approvals_reviewer.map(Into::into),
                ..Default::default()
            }).await? {
                color_eyre::eyre::bail!("this app server does not support confirmed permission changes; use /permissions");
            }

            // This exact widget configuration was validated before the request.
            self.chat_widget.set_permission_profile_with_active_profile(
                config.permissions.permission_profile().clone(),
                config.permissions.active_permission_profile(),
            )?;
            self.chat_widget.set_approval_policy(AskForApproval::from(config.permissions.approval_policy.value()));
            self.config.permissions = config.permissions.clone();
            self.set_approvals_reviewer_in_app_and_widget(config.approvals_reviewer);
            self.runtime_approval_policy_override = selection.approval_policy;
            self.runtime_permission_profile_override = Some(RuntimePermissionProfileOverride::from_config(&config));
            self.sync_active_thread_permission_settings_to_cached_session().await;
            self.insert_history_cell(tui, Box::new(history_cell::new_info_event(format!("Permissions updated to {}", selection.display_label), /*hint*/ None)));
            Ok(())
        }.await;
        self.chat_widget.complete_permission_shortcut(thread_id);
        if let Err(err) = result {
            let error = crate::config_update::format_config_error(&err);
            self.insert_history_cell(
                tui,
                Box::new(history_cell::new_error_event(format!(
                    "Failed to update permissions: {error}"
                ))),
            );
        }
    }
}
