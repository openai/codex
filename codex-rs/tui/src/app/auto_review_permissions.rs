//! Auto Review permission transitions and managed-network restoration.

use super::*;

const WORKSPACE_NETWORK_ACCESS_KEY: &str = "sandbox_workspace_write.network_access";
const WORKSPACE_NETWORK_ACCESS_POINTER: &str = "/sandbox_workspace_write/network_access";

fn user_network_restriction_overrides_managed_enabled(response: &ConfigReadResponse) -> bool {
    let user_override_is_effective = response
        .origins
        .get(WORKSPACE_NETWORK_ACCESS_KEY)
        .is_some_and(|metadata| matches!(metadata.name, ConfigLayerSource::User { .. }));
    user_override_is_effective
        && response.layers.as_ref().is_some_and(|layers| {
            layers.iter().any(|layer| {
                matches!(
                    layer.name,
                    ConfigLayerSource::Mdm { .. }
                        | ConfigLayerSource::System { .. }
                        | ConfigLayerSource::EnterpriseManaged { .. }
                        | ConfigLayerSource::LegacyManagedConfigTomlFromFile { .. }
                        | ConfigLayerSource::LegacyManagedConfigTomlFromMdm
                ) && layer
                    .config
                    .pointer(WORKSPACE_NETWORK_ACCESS_POINTER)
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
            })
        })
}

impl App {
    pub(super) fn current_auto_review_selection(&self) -> Option<AutoReviewPresetSelection> {
        (self.config.approvals_reviewer == ApprovalsReviewer::AutoReview).then(|| {
            AutoReviewPresetSelection {
                approval_policy: AskForApproval::from(
                    self.config.permissions.approval_policy.value(),
                ),
                profile_update: PermissionPresetProfileUpdate::Preserve,
                display_label: "Approve for me".to_string(),
            }
        })
    }

    pub(super) async fn managed_network_restore_available(
        &self,
        app_server: &AppServerSession,
    ) -> bool {
        if self
            .config
            .permissions
            .network_sandbox_policy()
            .is_enabled()
        {
            return false;
        }
        let cwd = self.chat_widget.config_ref().cwd.display().to_string();
        match crate::config_update::read_effective_config_with_layers(
            app_server.request_handle(),
            cwd,
        )
        .await
        {
            Ok(response) => user_network_restriction_overrides_managed_enabled(&response),
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "failed to inspect managed network config for terminal browser"
                );
                false
            }
        }
    }

    pub(super) async fn apply_auto_review_preset(
        &mut self,
        app_server: &mut AppServerSession,
        selection: AutoReviewPresetSelection,
        network_choice: ManagedNetworkChoice,
    ) -> bool {
        let terminal_browser_enabled = self.config.features.enabled(Feature::TerminalBrowser);
        let network_is_restricted = !self
            .config
            .permissions
            .network_sandbox_policy()
            .is_enabled();
        if network_choice == ManagedNetworkChoice::Detect
            && terminal_browser_enabled
            && network_is_restricted
            && self.managed_network_restore_available(app_server).await
        {
            self.chat_widget
                .open_managed_network_restore_confirmation(selection);
            return false;
        }

        let restore_managed_network = network_choice == ManagedNetworkChoice::RestoreManaged;
        let mut next_config = self.config.clone();
        if !self.try_set_approval_policy_on_config(
            &mut next_config,
            selection.approval_policy,
            "Failed to enable Approve for me",
            "failed to set Auto Review approval policy",
        ) {
            return false;
        }
        next_config.approvals_reviewer = ApprovalsReviewer::AutoReview;

        let mut edits = vec![crate::config_update::replace_config_value(
            "approvals_reviewer",
            serde_json::json!(ApprovalsReviewer::AutoReview.to_string()),
        )];
        if restore_managed_network {
            edits.push(crate::config_update::clear_config_value(
                WORKSPACE_NETWORK_ACCESS_KEY,
            ));
        }
        if let Err(err) =
            crate::config_update::write_config_batch(app_server.request_handle(), edits).await
        {
            tracing::error!(error = %err, "failed to persist Auto Review selection");
            self.chat_widget.add_error_message(format!(
                "Failed to update Auto Review permissions: {}",
                crate::config_update::format_config_error(&err)
            ));
            return false;
        }

        if restore_managed_network {
            next_config = match self
                .rebuild_config_for_cwd(self.chat_widget.config_ref().cwd.to_path_buf())
                .await
            {
                Ok(config) => config,
                Err(err) => {
                    tracing::error!(
                        error = %err,
                        "failed to rebuild config after restoring managed network access"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Managed network access was restored on disk, but the current session could not refresh it: {err}"
                    ));
                    return false;
                }
            };
            if !self.try_set_approval_policy_on_config(
                &mut next_config,
                selection.approval_policy,
                "Failed to enable Approve for me",
                "failed to restore Auto Review approval policy",
            ) {
                return false;
            }
            next_config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        }

        let (permission_profile, active_permission_profile) = if restore_managed_network {
            if !next_config
                .permissions
                .network_sandbox_policy()
                .is_enabled()
            {
                self.chat_widget.add_error_message(
                    "The user network override was removed, but managed network access is still disabled."
                        .to_string(),
                );
                return false;
            }
            (
                Some(next_config.permissions.permission_profile().clone()),
                next_config.permissions.active_permission_profile(),
            )
        } else {
            match &selection.profile_update {
                PermissionPresetProfileUpdate::Preserve => (None, None),
                PermissionPresetProfileUpdate::Replace {
                    permission_profile,
                    active_permission_profile,
                } => (
                    Some(permission_profile.clone()),
                    Some(active_permission_profile.clone()),
                ),
            }
        };

        if let Some(permission_profile) = permission_profile.as_ref()
            && let Err(err) = next_config
                .permissions
                .set_permission_profile_from_session_snapshot(
                    PermissionProfileSnapshot::from_session_snapshot(
                        permission_profile.clone(),
                        active_permission_profile.clone(),
                    ),
                )
        {
            tracing::warn!(error = %err, "failed to apply Auto Review permission profile");
            self.chat_widget
                .add_error_message(format!("Failed to enable Approve for me: {err}"));
            return false;
        }

        let permission_network = next_config.permissions.network.clone();
        self.config = next_config;
        self.runtime_approval_policy_override = Some(selection.approval_policy);
        self.chat_widget
            .set_approval_policy(selection.approval_policy);
        self.chat_widget
            .set_approvals_reviewer(ApprovalsReviewer::AutoReview);
        if let Some(permission_profile) = permission_profile.as_ref()
            && let Err(err) = self
                .chat_widget
                .set_permission_profile_from_session_snapshot(
                    PermissionProfileSnapshot::from_session_snapshot(
                        permission_profile.clone(),
                        active_permission_profile.clone(),
                    ),
                )
        {
            tracing::warn!(error = %err, "failed to apply Auto Review profile to chat config");
            self.chat_widget
                .add_error_message(format!("Failed to enable Approve for me: {err}"));
            return false;
        }
        self.chat_widget.set_permission_network(permission_network);
        if permission_profile.is_some() {
            self.runtime_permission_profile_override =
                Some(RuntimePermissionProfileOverride::from_config(&self.config));
        }
        self.sync_active_thread_permission_settings_to_cached_session()
            .await;

        let op = AppCommand::override_turn_context(
            /*cwd*/ None,
            Some(selection.approval_policy),
            Some(ApprovalsReviewer::AutoReview),
            permission_profile,
            active_permission_profile,
            /*windows_sandbox_level*/ None,
            /*model*/ None,
            /*effort*/ None,
            /*summary*/ None,
            /*service_tier*/ None,
            /*collaboration_mode*/ None,
            /*personality*/ None,
        );
        let replay_state_op =
            ThreadEventStore::op_can_change_pending_replay_state(&op).then(|| op.clone());
        let submitted = self.chat_widget.submit_op(op);
        if submitted && let Some(op) = replay_state_op.as_ref() {
            self.note_active_thread_outbound_op(op).await;
            self.refresh_pending_thread_approvals().await;
        }

        let network_enabled = self
            .config
            .permissions
            .network_sandbox_policy()
            .is_enabled();
        let message = if restore_managed_network {
            format!(
                "Permissions updated to {}; managed network access enabled",
                selection.display_label
            )
        } else if matches!(
            selection.profile_update,
            PermissionPresetProfileUpdate::Replace { .. }
        ) {
            format!("Permissions updated to {}", selection.display_label)
        } else {
            format!("Review mode updated to {}", selection.display_label)
        };
        let hint = (terminal_browser_enabled && !network_enabled).then(|| {
            "Network access remains disabled; the terminal browser is unavailable.".to_string()
        });
        self.chat_widget.add_info_message(message, hint);
        self.chat_widget.submit_initial_user_message_if_pending();
        true
    }
}

#[cfg(test)]
#[path = "auto_review_permissions_tests.rs"]
mod tests;
