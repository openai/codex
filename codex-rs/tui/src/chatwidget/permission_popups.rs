//! Permission and approval popup flows for `ChatWidget`.
//!
//! This module owns the generic permission pickers and confirmation surfaces;
//! Windows-specific sandbox prompting lives beside it in
//! `windows_sandbox_prompts`.

use super::*;

impl ChatWidget {
    /// Open the permissions popup.
    pub(crate) fn open_approvals_popup(&mut self) {
        self.open_permissions_popup();
    }

    /// Open a popup to choose the permissions mode.
    pub(crate) fn open_permissions_popup(&mut self) {
        let include_read_only = cfg!(target_os = "windows");
        let current_approval =
            AskForApproval::from(self.config.permissions.approval_policy.value());
        let current_permission_profile = self.config.permissions.permission_profile().clone();
        let guardian_approval_enabled = self.config.features.enabled(Feature::GuardianApproval);
        let current_review_policy = self.config.approvals_reviewer;
        let mut items: Vec<SelectionItem> = Vec::new();
        let presets: Vec<ApprovalPreset> = builtin_approval_presets();

        #[cfg(target_os = "windows")]
        let windows_sandbox_level = WindowsSandboxLevel::from_config(&self.config);
        #[cfg(target_os = "windows")]
        let windows_degraded_sandbox_enabled =
            matches!(windows_sandbox_level, WindowsSandboxLevel::RestrictedToken);
        #[cfg(not(target_os = "windows"))]
        let windows_degraded_sandbox_enabled = false;

        let show_elevate_sandbox_hint =
            crate::legacy_core::windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED
                && windows_degraded_sandbox_enabled
                && presets.iter().any(|preset| preset.id == "auto");

        let guardian_disabled_reason = |enabled: bool| {
            let mut next_features = self.config.features.get().clone();
            next_features.set_enabled(Feature::GuardianApproval, enabled);
            self.config
                .features
                .can_set(&next_features)
                .err()
                .map(|err| err.to_string())
        };

        for preset in presets.into_iter() {
            if !include_read_only && preset.id == "read-only" {
                continue;
            }
            let base_name = if preset.id == "auto" && windows_degraded_sandbox_enabled {
                "Default (non-admin sandbox)".to_string()
            } else {
                preset.label.to_string()
            };
            let preset_approval = AskForApproval::from(preset.approval);
            let base_description =
                Some(preset.description.replace(" (Identical to Agent mode)", ""));
            let approval_disabled_reason = match self
                .config
                .permissions
                .approval_policy
                .can_set(&preset.approval)
            {
                Ok(()) => None,
                Err(err) => Some(err.to_string()),
            };
            let default_disabled_reason = approval_disabled_reason
                .clone()
                .or_else(|| guardian_disabled_reason(false));
            let requires_confirmation = preset.id == "full-access"
                && !self
                    .config
                    .notices
                    .hide_full_access_warning
                    .unwrap_or(false);
            let default_actions: Vec<SelectionAction> = if requires_confirmation {
                let preset_clone = preset.clone();
                vec![Box::new(move |tx| {
                    tx.send(AppEvent::OpenFullAccessConfirmation {
                        preset: preset_clone.clone(),
                        return_to_permissions: !include_read_only,
                    });
                })]
            } else if preset.id == "auto" {
                #[cfg(target_os = "windows")]
                {
                    if WindowsSandboxLevel::from_config(&self.config)
                        == WindowsSandboxLevel::Disabled
                    {
                        let preset_clone = preset.clone();
                        if crate::legacy_core::windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED
                            && crate::legacy_core::windows_sandbox::sandbox_setup_is_complete(
                                self.config.codex_home.as_path(),
                            )
                        {
                            vec![Box::new(move |tx| {
                                tx.send(AppEvent::EnableWindowsSandboxForAgentMode {
                                    preset: preset_clone.clone(),
                                    mode: WindowsSandboxEnableMode::Elevated,
                                });
                            })]
                        } else {
                            vec![Box::new(move |tx| {
                                tx.send(AppEvent::OpenWindowsSandboxEnablePrompt {
                                    preset: preset_clone.clone(),
                                });
                            })]
                        }
                    } else if let Some((sample_paths, extra_count, failed_scan)) =
                        self.world_writable_warning_details()
                    {
                        let preset_clone = preset.clone();
                        vec![Box::new(move |tx| {
                            tx.send(AppEvent::OpenWorldWritableWarningConfirmation {
                                preset: Some(preset_clone.clone()),
                                sample_paths: sample_paths.clone(),
                                extra_count,
                                failed_scan,
                            });
                        })]
                    } else {
                        Self::approval_preset_actions(
                            preset_approval,
                            preset.permission_profile.clone(),
                            base_name.clone(),
                            ApprovalsReviewer::User,
                        )
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    Self::approval_preset_actions(
                        preset_approval,
                        preset.permission_profile.clone(),
                        base_name.clone(),
                        ApprovalsReviewer::User,
                    )
                }
            } else {
                Self::approval_preset_actions(
                    preset_approval,
                    preset.permission_profile.clone(),
                    base_name.clone(),
                    ApprovalsReviewer::User,
                )
            };
            if preset.id == "auto" {
                items.push(SelectionItem {
                    name: base_name.clone(),
                    description: base_description.clone(),
                    is_current: current_review_policy == ApprovalsReviewer::User
                        && Self::preset_matches_current(
                            current_approval,
                            &current_permission_profile,
                            self.config.cwd.as_path(),
                            &preset,
                        ),
                    actions: default_actions,
                    dismiss_on_select: true,
                    disabled_reason: default_disabled_reason,
                    ..Default::default()
                });

                if guardian_approval_enabled {
                    items.push(SelectionItem {
                        name: "Auto-review".to_string(),
                        description: Some(
                            "Same workspace-write permissions as Default, but eligible `on-request` approvals are routed through the auto-reviewer subagent."
                                .to_string(),
                        ),
                        is_current: current_review_policy == ApprovalsReviewer::AutoReview
                            && Self::preset_matches_current(
                                current_approval,
                                &current_permission_profile,
                                self.config.cwd.as_path(),
                                &preset,
                            ),
                        actions: Self::approval_preset_actions(
                            preset_approval,
                            preset.permission_profile.clone(),
                            "Auto-review".to_string(),
                            ApprovalsReviewer::AutoReview,
                        ),
                        dismiss_on_select: true,
                        disabled_reason: approval_disabled_reason
                            .or_else(|| guardian_disabled_reason(true)),
                        ..Default::default()
                    });
                }
            } else {
                items.push(SelectionItem {
                    name: base_name,
                    description: base_description,
                    is_current: Self::preset_matches_current(
                        current_approval,
                        &current_permission_profile,
                        self.config.cwd.as_path(),
                        &preset,
                    ),
                    actions: default_actions,
                    dismiss_on_select: true,
                    disabled_reason: default_disabled_reason,
                    ..Default::default()
                });
            }
        }

        let footer_note = show_elevate_sandbox_hint.then(|| {
            vec![
                "The non-admin sandbox protects your files and prevents network access under most circumstances. However, it carries greater risk if prompt injected. To upgrade to the default sandbox, run ".dim(),
                "/setup-default-sandbox".cyan(),
                ".".dim(),
            ]
            .into()
        });

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Update Model Permissions".to_string()),
            footer_note,
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: Box::new(()),
            ..Default::default()
        });
    }

    pub(crate) fn open_auto_review_denials_popup(&mut self) {
        if self.review.recent_auto_review_denials.is_empty() {
            self.add_info_message(
                "No recent auto-review denials in this thread.".to_string(),
                Some("Denials are recorded after auto-review rejects an action.".to_string()),
            );
            return;
        }
        let Some(thread_id) = self.thread_id() else {
            self.add_error_message("That thread is no longer available.".to_string());
            return;
        };

        let mut items = vec![SelectionItem {
            name: "Command".to_string(),
            description: Some("Rationale".to_string()),
            is_disabled: true,
            search_value: Some(String::new()),
            ..Default::default()
        }];
        items.extend(
            self.review
                .recent_auto_review_denials
                .entries()
                .map(|event| {
                    let id = event.id.clone();
                    let summary = auto_review_denials::action_summary(&event.action);
                    let rationale = event
                        .rationale
                        .as_deref()
                        .unwrap_or("Auto-review did not include a rationale.");
                    SelectionItem {
                        name: summary.clone(),
                        description: Some(rationale.to_string()),
                        selected_description: Some(rationale.to_string()),
                        search_value: Some(format!("{summary} {rationale}")),
                        actions: vec![Box::new(move |tx| {
                            tx.send(AppEvent::ApproveRecentAutoReviewDenial {
                                thread_id,
                                id: id.clone(),
                            });
                        })],
                        dismiss_on_select: true,
                        ..Default::default()
                    }
                }),
        );

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Auto-review Denials".to_string()),
            subtitle: Some("Select a denied action to approve.".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            is_searchable: true,
            col_width_mode: ColumnWidthMode::AutoAllRows,
            ..Default::default()
        });
        self.request_redraw();
    }

    pub(crate) fn approve_recent_auto_review_denial(&mut self, thread_id: ThreadId, id: String) {
        let Some(event) = self.review.recent_auto_review_denials.take(&id) else {
            self.add_error_message("That auto-review denial is no longer available.".to_string());
            return;
        };

        self.app_event_tx.send(AppEvent::SubmitThreadOp {
            thread_id,
            op: AppCommand::approve_guardian_denied_action(event),
        });
        self.add_info_message(
            "Approval recorded for one retry of the selected auto-review denial.".to_string(),
            Some(
                "The model will see the approval context; the retry still goes through auto-review."
                    .to_string(),
            ),
        );
    }

    pub(super) fn approval_preset_actions(
        approval: AskForApproval,
        permission_profile: PermissionProfile,
        label: String,
        approvals_reviewer: ApprovalsReviewer,
    ) -> Vec<SelectionAction> {
        vec![Box::new(move |tx| {
            let permission_profile_clone = permission_profile.clone();
            tx.send(AppEvent::CodexOp(AppCommand::override_turn_context(
                /*cwd*/ None,
                Some(approval),
                Some(approvals_reviewer),
                Some(permission_profile_clone.clone()),
                /*windows_sandbox_level*/ None,
                /*model*/ None,
                /*effort*/ None,
                /*summary*/ None,
                /*service_tier*/ None,
                /*collaboration_mode*/ None,
                /*personality*/ None,
            )));
            tx.send(AppEvent::UpdateAskForApprovalPolicy(approval));
            tx.send(AppEvent::UpdatePermissionProfile(permission_profile_clone));
            tx.send(AppEvent::UpdateApprovalsReviewer(approvals_reviewer));
            tx.send(AppEvent::InsertHistoryCell(Box::new(
                history_cell::new_info_event(
                    format!("Permissions updated to {label}"),
                    /*hint*/ None,
                ),
            )));
        })]
    }

    pub(super) fn preset_matches_current(
        current_approval: AskForApproval,
        current_permission_profile: &PermissionProfile,
        cwd: &std::path::Path,
        preset: &ApprovalPreset,
    ) -> bool {
        let preset_approval = AskForApproval::from(preset.approval);
        if current_approval != preset_approval {
            return false;
        }

        match preset.id {
            "full-access" => matches!(current_permission_profile, PermissionProfile::Disabled),
            "read-only" => {
                let file_system_policy = current_permission_profile.file_system_sandbox_policy();
                matches!(
                    current_permission_profile,
                    PermissionProfile::Managed { .. }
                ) && !file_system_policy.has_full_disk_write_access()
                    && file_system_policy
                        .get_writable_roots_with_cwd(cwd)
                        .is_empty()
                    && current_permission_profile.network_sandbox_policy()
                        == preset.permission_profile.network_sandbox_policy()
            }
            "auto" => {
                let file_system_policy = current_permission_profile.file_system_sandbox_policy();
                matches!(
                    current_permission_profile,
                    PermissionProfile::Managed { .. }
                ) && file_system_policy.can_write_path_with_cwd(cwd, cwd)
                    && !file_system_policy.has_full_disk_write_access()
                    && current_permission_profile.network_sandbox_policy()
                        == preset.permission_profile.network_sandbox_policy()
            }
            _ => current_permission_profile == &preset.permission_profile,
        }
    }

    pub(crate) fn open_full_access_confirmation(
        &mut self,
        preset: ApprovalPreset,
        return_to_permissions: bool,
    ) {
        let selected_name = preset.label.to_string();
        let approval = AskForApproval::from(preset.approval);
        let permission_profile = preset.permission_profile;
        let mut header_children: Vec<Box<dyn Renderable>> = Vec::new();
        let title_line = Line::from("Enable full access?").bold();
        let info_line = Line::from(vec![
            "When Codex runs with full access, it can edit any file on your computer and run commands with network, without your approval. "
                .into(),
            "Exercise caution when enabling full access. This significantly increases the risk of data loss, leaks, or unexpected behavior."
                .fg(Color::Red),
        ]);
        header_children.push(Box::new(title_line));
        header_children.push(Box::new(
            Paragraph::new(vec![info_line]).wrap(Wrap { trim: false }),
        ));
        let header = ColumnRenderable::with(header_children);

        let mut accept_actions = Self::approval_preset_actions(
            approval,
            permission_profile.clone(),
            selected_name.clone(),
            ApprovalsReviewer::User,
        );
        accept_actions.push(Box::new(|tx| {
            tx.send(AppEvent::UpdateFullAccessWarningAcknowledged(true));
        }));

        let mut accept_and_remember_actions = Self::approval_preset_actions(
            approval,
            permission_profile,
            selected_name,
            ApprovalsReviewer::User,
        );
        accept_and_remember_actions.push(Box::new(|tx| {
            tx.send(AppEvent::UpdateFullAccessWarningAcknowledged(true));
            tx.send(AppEvent::PersistFullAccessWarningAcknowledged);
        }));

        let deny_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            if return_to_permissions {
                tx.send(AppEvent::OpenPermissionsPopup);
            } else {
                tx.send(AppEvent::OpenApprovalsPopup);
            }
        })];

        let items = vec![
            SelectionItem {
                name: "Yes, continue anyway".to_string(),
                description: Some("Apply full access for this session".to_string()),
                actions: accept_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Yes, and don't ask again".to_string(),
                description: Some("Enable full access and remember this choice".to_string()),
                actions: accept_and_remember_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Cancel".to_string(),
                description: Some("Go back without enabling full access".to_string()),
                actions: deny_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
        ];

        self.bottom_pane.show_selection_view(SelectionViewParams {
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: Box::new(header),
            ..Default::default()
        });
    }
}
