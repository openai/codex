//! Cycle ordinary built-in modes using the last permission catalog received from the server.

use super::*;

impl ChatWidget {
    pub(super) fn handle_permission_shortcut(&mut self, key_event: KeyEvent) -> bool {
        let forward = if self.chat_keymap.next_permission_mode.is_pressed(key_event) {
            true
        } else if self
            .chat_keymap
            .previous_permission_mode
            .is_pressed(key_event)
        {
            false
        } else {
            return false;
        };
        if !self.bottom_pane.no_modal_or_popup_active() {
            return false;
        }
        if self.permission_shortcut_pending {
            return true;
        }
        if self.blocks_direct_input {
            self.add_error_message(PARENT_OWNED_INPUT_MESSAGE.to_string());
            return true;
        }
        let Some(thread_id) = self.thread_id else {
            return true;
        };

        let Some(discovery) = self.permission_discovery.as_ref() else {
            self.request_permission_profiles();
            return true;
        };
        let server_requirements = codex_config::ConfigRequirements {
            auto_review_required_models: discovery
                .requirements
                .as_ref()
                .and_then(|requirements| requirements.auto_review.as_ref())
                .and_then(|auto_review| auto_review.required_on_models.as_ref())
                .map(|models| {
                    codex_config::Sourced::new(
                        models.iter().cloned().collect(),
                        codex_config::RequirementSource::Unknown,
                    )
                }),
            ..Default::default()
        };
        let current_approval =
            AskForApproval::from(self.config.permissions.approval_policy.value());
        let active_profile = self.config.permissions.active_permission_profile();
        let mut choices = Vec::new();
        for preset in builtin_approval_presets() {
            if !matches!(preset.id, "read-only" | "auto") {
                continue;
            }
            for reviewer in [ApprovalsReviewer::User, ApprovalsReviewer::AutoReview] {
                if reviewer == ApprovalsReviewer::AutoReview
                    && (preset.id != "auto"
                        || !self.config.features.enabled(Feature::GuardianApproval))
                {
                    continue;
                }
                let approval = AskForApproval::from(preset.approval);
                if discovery
                    .disabled_reason(
                        &preset.active_permission_profile.id,
                        Some(approval),
                        Some(reviewer.into()),
                    )
                    .is_some()
                    || (server_requirements.auto_review_required_for_model(self.current_model())
                        && reviewer != ApprovalsReviewer::AutoReview)
                {
                    continue;
                }
                // These modes still need explicit Windows setup.
                #[cfg(target_os = "windows")]
                if preset.id == "auto"
                    && (matches!(
                        self.windows_sandbox_host,
                        crate::app::WindowsSandboxHost::Mixed
                            | crate::app::WindowsSandboxHost::Unknown
                    ) || (self.windows_sandbox_host != crate::app::WindowsSandboxHost::Remote
                        && reviewer == ApprovalsReviewer::User
                        && !self.windows_sandbox_config.is_enabled()))
                {
                    continue;
                }
                let is_current = current_approval == approval
                    && self.config.approvals_reviewer == reviewer
                    && active_profile.as_ref().map_or_else(
                        || {
                            Self::preset_matches_current(
                                current_approval,
                                self.config.permissions.permission_profile(),
                                self.config.cwd.as_path(),
                                &preset,
                            )
                        },
                        |active| active.id == preset.active_permission_profile.id,
                    );
                let label = match (preset.id, reviewer) {
                    ("auto", ApprovalsReviewer::User) => ASK_FOR_APPROVAL_LABEL,
                    ("auto", ApprovalsReviewer::AutoReview) => APPROVE_FOR_ME_LABEL,
                    _ => preset.label,
                };
                choices.push((
                    is_current,
                    PermissionProfileSelection {
                        profile_id: preset.active_permission_profile.id.clone(),
                        approval_policy: Some(approval),
                        approvals_reviewer: Some(reviewer),
                        display_label: label.to_string(),
                    },
                ));
            }
        }
        if !forward {
            choices.reverse();
        }
        let start = choices
            .iter()
            .position(|(current, _)| *current)
            .map_or(0, |index| index + 1);
        if let Some((_, selection)) = choices
            .iter()
            .cycle()
            .skip(start)
            .take(choices.len())
            .find(|(current, _)| !current)
        {
            self.permission_shortcut_pending = true;
            self.app_event_tx.send(AppEvent::ApplyPermissionShortcut {
                thread_id,
                selection: selection.clone(),
            });
        } else {
            self.add_info_message(
                "No other permission modes are available.".to_string(),
                /*hint*/ None,
            );
        }
        true
    }

    pub(crate) fn complete_permission_shortcut(&mut self, thread_id: ThreadId) {
        if self.thread_id == Some(thread_id) {
            self.permission_shortcut_pending = false;
        }
    }
}
