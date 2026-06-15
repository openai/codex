//! Windows sandbox prompts and warning surfaces for `ChatWidget`.

use super::*;

impl ChatWidget {
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn windows_sandbox_mode_allowed(&self, mode: WindowsSandboxModeToml) -> bool {
        self.config
            .config_layer_stack
            .requirements()
            .windows_sandbox_mode
            .can_set(&Some(mode))
            .is_ok()
    }

    #[cfg(any(target_os = "windows", test))]
    pub(super) fn elevated_windows_sandbox_setup_required(&self) -> bool {
        crate::windows_sandbox::level_from_config(&self.config) == WindowsSandboxLevel::Elevated
            && self
                .config
                .config_layer_stack
                .requirements()
                .windows_sandbox_mode
                .source
                .is_some()
            && !crate::windows_sandbox::sandbox_setup_is_complete(self.config.codex_home.as_path())
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn world_writable_warning_details(&self) -> Option<(Vec<String>, usize, bool)> {
        if self
            .config
            .notices
            .hide_world_writable_warning
            .unwrap_or(false)
        {
            return None;
        }
        let cwd = self.config.cwd.clone();
        let workspace_roots = self.config.effective_workspace_roots();
        let env_map: std::collections::HashMap<String, String> = std::env::vars().collect();
        let permission_profile = self.config.permissions.effective_permission_profile();
        let Ok(permissions) =
            codex_windows_sandbox::ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_workspace_roots(
                &permission_profile,
                workspace_roots.as_slice(),
            )
        else {
            return None;
        };
        match codex_windows_sandbox::apply_world_writable_scan_and_denies_for_permissions(
            self.config.codex_home.as_path(),
            cwd.as_path(),
            &env_map,
            &permissions,
            Some(self.config.codex_home.as_path()),
        ) {
            Ok(_) => None,
            Err(_) => Some((Vec::new(), 0, true)),
        }
    }

    #[cfg(not(target_os = "windows"))]
    #[allow(dead_code)]
    pub(crate) fn world_writable_warning_details(&self) -> Option<(Vec<String>, usize, bool)> {
        None
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn open_world_writable_warning_confirmation(
        &mut self,
        preset: Option<ApprovalPreset>,
        profile_selection: Option<PermissionProfileSelection>,
        sample_paths: Vec<String>,
        extra_count: usize,
        failed_scan: bool,
    ) {
        let (approval, permission_profile, active_permission_profile) = match &preset {
            Some(p) => (
                Some(AskForApproval::from(p.approval)),
                Some(p.permission_profile.clone()),
                Some(p.active_permission_profile.clone()),
            ),
            None => (None, None, None),
        };
        let mut header_children: Vec<Box<dyn Renderable>> = Vec::new();
        let describe_profile = |profile: &PermissionProfile| {
            if matches!(profile, PermissionProfile::Disabled) {
                "Full Access mode"
            } else if profile
                .file_system_sandbox_policy()
                .can_write_path_with_cwd(self.config.cwd.as_path(), self.config.cwd.as_path())
            {
                "Agent mode"
            } else {
                "Read-Only mode"
            }
        };
        let mode_label = preset
            .as_ref()
            .map(|p| describe_profile(&p.permission_profile))
            .unwrap_or_else(|| {
                describe_profile(&self.config.permissions.effective_permission_profile())
            });
        let info_line = if failed_scan {
            Line::from(vec![
                "We couldn't complete the world-writable scan, so protections cannot be verified. "
                    .into(),
                format!("The Windows sandbox cannot guarantee protection in {mode_label}.")
                    .fg(Color::Red),
            ])
        } else {
            Line::from(vec![
                "The Windows sandbox cannot protect writes to folders that are writable by Everyone.".into(),
                " Consider removing write access for Everyone from the following folders:".into(),
            ])
        };
        header_children.push(Box::new(
            Paragraph::new(vec![info_line]).wrap(Wrap { trim: false }),
        ));

        if !sample_paths.is_empty() {
            // Show up to three examples and optionally an "and X more" line.
            let mut lines: Vec<Line> = Vec::new();
            lines.push(Line::from(""));
            for p in &sample_paths {
                lines.push(Line::from(format!("  - {p}")));
            }
            if extra_count > 0 {
                lines.push(Line::from(format!("and {extra_count} more")));
            }
            header_children.push(Box::new(Paragraph::new(lines).wrap(Wrap { trim: false })));
        }
        let header = ColumnRenderable::with(header_children);

        // Build actions ensuring acknowledgement happens before applying the
        // new permission profile, so downstream policy-change hooks don't
        // re-trigger the warning.
        let mut accept_actions: Vec<SelectionAction> = Vec::new();
        // Suppress the immediate re-scan only when a preset will be applied via
        // /permissions, to avoid duplicate warnings from the ensuing policy change.
        if preset.is_some() {
            accept_actions.push(Box::new(|tx| {
                tx.send(AppEvent::SkipNextWorldWritableScan);
            }));
        }
        if let Some(selection) = profile_selection.clone() {
            accept_actions.extend(Self::permission_profile_selection_actions(selection));
        } else if let (Some(approval), Some(permission_profile), Some(active_permission_profile)) = (
            approval,
            permission_profile.clone(),
            active_permission_profile.clone(),
        ) {
            accept_actions.extend(Self::approval_preset_actions(
                approval,
                permission_profile,
                active_permission_profile,
                mode_label.to_string(),
                ApprovalsReviewer::User,
            ));
        }

        let mut accept_and_remember_actions: Vec<SelectionAction> = Vec::new();
        accept_and_remember_actions.push(Box::new(|tx| {
            tx.send(AppEvent::UpdateWorldWritableWarningAcknowledged(true));
            tx.send(AppEvent::PersistWorldWritableWarningAcknowledged);
        }));
        if let Some(selection) = profile_selection {
            accept_and_remember_actions
                .extend(Self::permission_profile_selection_actions(selection));
        } else if let (Some(approval), Some(permission_profile), Some(active_permission_profile)) =
            (approval, permission_profile, active_permission_profile)
        {
            accept_and_remember_actions.extend(Self::approval_preset_actions(
                approval,
                permission_profile,
                active_permission_profile,
                mode_label.to_string(),
                ApprovalsReviewer::User,
            ));
        }

        let items = vec![
            SelectionItem {
                name: "Continue".to_string(),
                description: Some(format!("Apply {mode_label} for this session")),
                actions: accept_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Continue and don't warn again".to_string(),
                description: Some(format!("Enable {mode_label} and remember this choice")),
                actions: accept_and_remember_actions,
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

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn open_world_writable_warning_confirmation(
        &mut self,
        _preset: Option<ApprovalPreset>,
        _profile_selection: Option<PermissionProfileSelection>,
        _sample_paths: Vec<String>,
        _extra_count: usize,
        _failed_scan: bool,
    ) {
    }

    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn open_windows_sandbox_enable_prompt(
        &mut self,
        preset: ApprovalPreset,
        profile_selection: Option<PermissionProfileSelection>,
    ) {
        use ratatui_macros::line;

        self.session_telemetry.counter(
            "codex.windows_sandbox.elevated_prompt_shown",
            /*inc*/ 1,
            &[],
        );

        let allow_unelevated =
            self.windows_sandbox_mode_allowed(WindowsSandboxModeToml::Unelevated);
        let setup_choice_is_required =
            !allow_unelevated || self.elevated_windows_sandbox_setup_required();
        let mut header = ColumnRenderable::new();
        header.push(*Box::new(
            Paragraph::new(if allow_unelevated {
                vec![
                    line!["Set up the Codex agent sandbox to protect your files and control network access. Learn more <https://developers.openai.com/codex/windows>"],
                ]
            } else {
                vec![
                    line!["Your organization requires the default Codex agent sandbox to continue. Set it up to protect your files and control network access."],
                    line!["Learn more <https://developers.openai.com/codex/windows>"],
                ]
            })
            .wrap(Wrap { trim: false }),
        ));

        let accept_otel = self.session_telemetry.clone();
        let legacy_otel = self.session_telemetry.clone();
        let legacy_preset = preset.clone();
        let legacy_profile_selection = profile_selection.clone();
        let quit_otel = self.session_telemetry.clone();
        let retry_preset = preset.clone();
        let retry_profile_selection = profile_selection.clone();
        let mut items = vec![SelectionItem {
            name: "Set up default sandbox (requires Administrator permissions)".to_string(),
            description: None,
            actions: vec![Box::new(move |tx| {
                accept_otel.counter(
                    "codex.windows_sandbox.elevated_prompt_accept",
                    /*inc*/ 1,
                    &[],
                );
                tx.send(AppEvent::BeginWindowsSandboxElevatedSetup {
                    preset: preset.clone(),
                    profile_selection: profile_selection.clone(),
                });
            })],
            dismiss_on_select: true,
            ..Default::default()
        }];
        if allow_unelevated {
            items.push(SelectionItem {
                name: "Use non-admin sandbox (higher risk if prompt injected)".to_string(),
                description: None,
                actions: vec![Box::new(move |tx| {
                    legacy_otel.counter(
                        "codex.windows_sandbox.elevated_prompt_use_legacy",
                        /*inc*/ 1,
                        &[],
                    );
                    tx.send(AppEvent::BeginWindowsSandboxLegacySetup {
                        preset: legacy_preset.clone(),
                        profile_selection: legacy_profile_selection.clone(),
                    });
                })],
                dismiss_on_select: true,
                ..Default::default()
            });
        }
        items.push(SelectionItem {
            name: "Quit".to_string(),
            description: None,
            actions: vec![Box::new(move |tx| {
                quit_otel.counter(
                    "codex.windows_sandbox.elevated_prompt_quit",
                    /*inc*/ 1,
                    &[],
                );
                tx.send(AppEvent::Exit(ExitMode::ShutdownFirst));
            })],
            dismiss_on_select: true,
            ..Default::default()
        });

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: None,
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: Box::new(header),
            on_cancel: setup_choice_is_required.then(|| {
                Box::new(move |tx: &AppEventSender| {
                    tx.send(AppEvent::OpenWindowsSandboxEnablePrompt {
                        preset: retry_preset.clone(),
                        profile_selection: retry_profile_selection.clone(),
                    });
                }) as _
            }),
            ..Default::default()
        });
    }

    #[cfg(all(not(target_os = "windows"), not(test)))]
    pub(crate) fn open_windows_sandbox_enable_prompt(
        &mut self,
        _preset: ApprovalPreset,
        _profile_selection: Option<PermissionProfileSelection>,
    ) {
    }

    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn open_windows_sandbox_fallback_prompt(
        &mut self,
        preset: ApprovalPreset,
        profile_selection: Option<PermissionProfileSelection>,
    ) {
        use ratatui_macros::line;

        let allow_unelevated =
            self.windows_sandbox_mode_allowed(WindowsSandboxModeToml::Unelevated);
        let setup_choice_is_required =
            !allow_unelevated || self.elevated_windows_sandbox_setup_required();
        let mut lines = Vec::new();
        lines.push(line![
            "Couldn't set up your sandbox with Administrator permissions".bold()
        ]);
        lines.push(line![""]);
        if allow_unelevated {
            lines.push(line![
                "You can still use Codex in a non-admin sandbox. It carries greater risk if prompt injected."
            ]);
        } else {
            lines.push(line![
                "Your organization requires the default sandbox before Codex can continue."
            ]);
        }
        lines.push(line![
            "Learn more <https://developers.openai.com/codex/windows>"
        ]);

        let mut header = ColumnRenderable::new();
        header.push(*Box::new(Paragraph::new(lines).wrap(Wrap { trim: false })));

        let elevated_preset = preset.clone();
        let legacy_preset = preset;
        let retry_preset = elevated_preset.clone();
        let retry_profile_selection = profile_selection.clone();
        let elevated_profile_selection = profile_selection.clone();
        let legacy_profile_selection = profile_selection;
        let quit_otel = self.session_telemetry.clone();
        let mut items = vec![SelectionItem {
            name: "Try setting up admin sandbox again".to_string(),
            description: None,
            actions: vec![Box::new({
                let otel = self.session_telemetry.clone();
                let preset = elevated_preset;
                move |tx| {
                    otel.counter(
                        "codex.windows_sandbox.fallback_retry_elevated",
                        /*inc*/ 1,
                        &[],
                    );
                    tx.send(AppEvent::BeginWindowsSandboxElevatedSetup {
                        preset: preset.clone(),
                        profile_selection: elevated_profile_selection.clone(),
                    });
                }
            })],
            dismiss_on_select: true,
            ..Default::default()
        }];
        if allow_unelevated {
            items.push(SelectionItem {
                name: "Use Codex with non-admin sandbox".to_string(),
                description: None,
                actions: vec![Box::new({
                    let otel = self.session_telemetry.clone();
                    let preset = legacy_preset;
                    move |tx| {
                        otel.counter(
                            "codex.windows_sandbox.fallback_use_legacy",
                            /*inc*/ 1,
                            &[],
                        );
                        tx.send(AppEvent::BeginWindowsSandboxLegacySetup {
                            preset: preset.clone(),
                            profile_selection: legacy_profile_selection.clone(),
                        });
                    }
                })],
                dismiss_on_select: true,
                ..Default::default()
            });
        }
        items.push(SelectionItem {
            name: "Quit".to_string(),
            description: None,
            actions: vec![Box::new(move |tx| {
                quit_otel.counter(
                    "codex.windows_sandbox.fallback_prompt_quit",
                    /*inc*/ 1,
                    &[],
                );
                tx.send(AppEvent::Exit(ExitMode::ShutdownFirst));
            })],
            dismiss_on_select: true,
            ..Default::default()
        });

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: None,
            footer_hint: Some(standard_popup_hint_line()),
            items,
            header: Box::new(header),
            on_cancel: setup_choice_is_required.then(|| {
                Box::new(move |tx: &AppEventSender| {
                    tx.send(AppEvent::OpenWindowsSandboxFallbackPrompt {
                        preset: retry_preset.clone(),
                        profile_selection: retry_profile_selection.clone(),
                    });
                }) as _
            }),
            ..Default::default()
        });
    }

    #[cfg(all(not(target_os = "windows"), not(test)))]
    pub(crate) fn open_windows_sandbox_fallback_prompt(
        &mut self,
        _preset: ApprovalPreset,
        _profile_selection: Option<PermissionProfileSelection>,
    ) {
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn maybe_prompt_windows_sandbox_enable(&mut self, show_now: bool) {
        let windows_sandbox_level = crate::windows_sandbox::level_from_config(&self.config);
        let setup_is_required = windows_sandbox_level == WindowsSandboxLevel::Disabled
            || self.elevated_windows_sandbox_setup_required();
        if show_now
            && setup_is_required
            && let Some(preset) = builtin_approval_presets()
                .into_iter()
                .find(|preset| preset.id == "auto")
        {
            self.open_windows_sandbox_enable_prompt(preset, /*profile_selection*/ None);
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn maybe_prompt_windows_sandbox_enable(&mut self, _show_now: bool) {}

    #[cfg(target_os = "windows")]
    pub(crate) fn show_windows_sandbox_setup_status(&mut self) {
        // While elevated sandbox setup runs, prevent typing so the user doesn't
        // accidentally queue messages that will run under an unexpected mode.
        self.bottom_pane.set_composer_input_enabled(
            /*enabled*/ false,
            Some("Input disabled until setup completes.".to_string()),
        );
        self.bottom_pane.ensure_status_indicator();
        self.bottom_pane
            .set_interrupt_hint_visible(/*visible*/ false);
        self.set_status(
            "Setting up sandbox...".to_string(),
            Some("Hang tight, this may take a few minutes".to_string()),
            StatusDetailsCapitalization::CapitalizeFirst,
            STATUS_DETAILS_DEFAULT_MAX_LINES,
        );
        self.request_redraw();
    }

    #[cfg(not(target_os = "windows"))]
    #[allow(dead_code)]
    pub(crate) fn show_windows_sandbox_setup_status(&mut self) {}

    #[cfg(target_os = "windows")]
    pub(crate) fn clear_windows_sandbox_setup_status(&mut self) {
        self.bottom_pane
            .set_composer_input_enabled(/*enabled*/ true, /*placeholder*/ None);
        self.bottom_pane.hide_status_indicator();
        self.request_redraw();
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn clear_windows_sandbox_setup_status(&mut self) {}
}
