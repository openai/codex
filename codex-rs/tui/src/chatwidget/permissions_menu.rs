use super::*;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS;

impl ChatWidget {
    pub(super) fn open_permission_profiles_popup(&mut self) {
        let active_profile_id = self
            .config
            .permissions
            .active_permission_profile()
            .map(|profile| profile.id);
        let presets = builtin_approval_presets();
        let Some(read_only) = presets.iter().find(|preset| preset.id == "read-only") else {
            self.add_error_message(
                "Internal error: missing the 'read-only' approval preset.".to_string(),
            );
            return;
        };
        let Some(default) = presets.iter().find(|preset| preset.id == "auto") else {
            self.add_error_message(
                "Internal error: missing the 'auto' approval preset.".to_string(),
            );
            return;
        };
        let Some(full_access) = presets.iter().find(|preset| preset.id == "full-access") else {
            self.add_error_message(
                "Internal error: missing the 'full-access' approval preset.".to_string(),
            );
            return;
        };
        let mut items = vec![
            self.builtin_permission_mode_selection_item(
                default,
                ":workspace",
                default
                    .description
                    .replace(" (Identical to Agent mode)", ""),
                AskForApproval::from(default.approval),
                ApprovalsReviewer::User,
            ),
        ];
        if self.config.features.enabled(Feature::GuardianApproval) {
            items.push(self.builtin_permission_mode_selection_item(
                default,
                ":workspace",
                AUTO_REVIEW_DESCRIPTION.to_string(),
                AskForApproval::OnRequest,
                ApprovalsReviewer::AutoReview,
            ));
        }
        items.push(self.builtin_permission_mode_selection_item(
            full_access,
            BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS,
            full_access.description.to_string(),
            AskForApproval::from(full_access.approval),
            ApprovalsReviewer::User,
        ));
        items.push(self.builtin_permission_mode_selection_item(
            read_only,
            ":read-only",
            read_only.description.to_string(),
            AskForApproval::from(read_only.approval),
            ApprovalsReviewer::User,
        ));
        items.extend(
            self.config
                .custom_permission_profiles
                .iter()
                .map(|profile| {
                    Self::permission_profile_selection_item(
                        &profile.id,
                        &profile.id,
                        profile
                            .description
                            .as_deref()
                            .unwrap_or("Configured permission profile."),
                        active_profile_id.as_deref(),
                        profile.allowed,
                    )
                }),
        );

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Update Model Permissions".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: Box::new(()),
            ..Default::default()
        });
    }

    fn builtin_permission_mode_selection_item(
        &self,
        preset: &ApprovalPreset,
        id: &str,
        description: String,
        approval_policy: AskForApproval,
        approvals_reviewer: ApprovalsReviewer,
    ) -> SelectionItem {
        let label = match (preset.id, approvals_reviewer) {
            ("auto", ApprovalsReviewer::AutoReview) => APPROVE_FOR_ME_LABEL,
            ("auto", ApprovalsReviewer::User) => ASK_FOR_APPROVAL_LABEL,
            _ => preset.label,
        };
        let active_profile_id = self
            .config
            .permissions
            .active_permission_profile()
            .map(|profile| profile.id);
        let current_approval =
            AskForApproval::from(self.config.permissions.approval_policy.value());
        let current_reviewer = self.config.approvals_reviewer;
        let profile_id = id.to_string();
        let selection = PermissionProfileSelection {
            profile_id,
            approval_policy: Some(approval_policy),
            approvals_reviewer: Some(approvals_reviewer),
            display_label: label.to_string(),
        };
        SelectionItem {
            name: label.to_string(),
            description: Some(description),
            is_current: active_profile_id.as_deref() == Some(id)
                && current_approval == approval_policy
                && current_reviewer == approvals_reviewer,
            actions: self.permission_mode_actions(
                preset,
                label.to_string(),
                approvals_reviewer,
                Some(selection),
                /*return_to_permissions*/ true,
            ),
            dismiss_on_select: true,
            disabled_reason: self
                .config
                .permissions
                .approval_policy
                .can_set(&approval_policy.to_core())
                .err()
                .map(|err| err.to_string())
                .or_else(|| {
                    self.config
                        .permissions
                        .can_set_permission_profile(&preset.permission_profile)
                        .err()
                        .map(|err| err.to_string())
                })
                .or_else(|| {
                    (!self
                        .config
                        .is_permission_profile_allowed(id, &preset.permission_profile))
                    .then(|| "Disabled by requirements.".to_string())
                }),
            ..Default::default()
        }
    }

    fn permission_profile_selection_item(
        label: &str,
        id: &str,
        description: &str,
        active_profile_id: Option<&str>,
        allowed: bool,
    ) -> SelectionItem {
        let id_for_action = id.to_string();
        let selection = PermissionProfileSelection {
            profile_id: id_for_action.clone(),
            approval_policy: None,
            approvals_reviewer: None,
            display_label: id_for_action,
        };
        SelectionItem {
            name: label.to_string(),
            description: Some(description.to_string()),
            is_current: active_profile_id == Some(id),
            actions: Self::permission_profile_selection_actions(selection),
            dismiss_on_select: true,
            disabled_reason: (!allowed).then(|| "Disabled by requirements.".to_string()),
            ..Default::default()
        }
    }
}
