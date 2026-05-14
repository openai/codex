//! Settings-adjacent popup surfaces for `ChatWidget`.
//!
//! This keeps theme, personality, audio-device, and experimental-feature UI
//! out of the main orchestration module without changing their event wiring.

use super::*;

impl ChatWidget {
    pub(super) fn open_theme_picker(&mut self) {
        let codex_home = crate::legacy_core::config::find_codex_home().ok();
        let terminal_width = self
            .last_rendered_width
            .get()
            .and_then(|width| u16::try_from(width).ok());
        let params = crate::theme_picker::build_theme_picker_params(
            self.config.tui_theme.as_deref(),
            codex_home.as_deref(),
            terminal_width,
        );
        self.bottom_pane.show_selection_view(params);
    }

    pub(crate) fn open_personality_popup(&mut self) {
        if !self.is_session_configured() {
            self.add_info_message(
                "Personality selection is disabled until startup completes.".to_string(),
                /*hint*/ None,
            );
            return;
        }
        if !self.current_model_supports_personality() {
            let current_model = self.current_model();
            self.add_error_message(format!(
                "Current model ({current_model}) doesn't support personalities. Try /model to pick a different model."
            ));
            return;
        }
        self.open_personality_popup_for_current_model();
    }

    fn open_personality_popup_for_current_model(&mut self) {
        let current_personality = self.config.personality.unwrap_or(Personality::Friendly);
        let personalities = [Personality::Friendly, Personality::Pragmatic];
        let supports_personality = self.current_model_supports_personality();

        let items: Vec<SelectionItem> = personalities
            .into_iter()
            .map(|personality| {
                let name = Self::personality_label(personality).to_string();
                let description = Some(Self::personality_description(personality).to_string());
                let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                    tx.send(AppEvent::CodexOp(AppCommand::override_turn_context(
                        /*cwd*/ None,
                        /*approval_policy*/ None,
                        /*approvals_reviewer*/ None,
                        /*permission_profile*/ None,
                        /*windows_sandbox_level*/ None,
                        /*model*/ None,
                        /*effort*/ None,
                        /*summary*/ None,
                        /*service_tier*/ None,
                        /*collaboration_mode*/ None,
                        Some(personality),
                    )));
                    tx.send(AppEvent::UpdatePersonality(personality));
                    tx.send(AppEvent::PersistPersonalitySelection { personality });
                })];
                SelectionItem {
                    name,
                    description,
                    is_current: current_personality == personality,
                    is_disabled: !supports_personality,
                    actions,
                    dismiss_on_select: true,
                    ..Default::default()
                }
            })
            .collect();

        let mut header = ColumnRenderable::new();
        header.push(Line::from("Select Personality".bold()));
        header.push(Line::from("Choose a communication style for Codex.".dim()));

        self.bottom_pane.show_selection_view(SelectionViewParams {
            header: Box::new(header),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    pub(crate) fn open_realtime_audio_popup(&mut self) {
        let items = [
            RealtimeAudioDeviceKind::Microphone,
            RealtimeAudioDeviceKind::Speaker,
        ]
        .into_iter()
        .map(|kind| {
            let description = Some(format!(
                "Current: {}",
                self.current_realtime_audio_selection_label(kind)
            ));
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::OpenRealtimeAudioDeviceSelection { kind });
            })];
            SelectionItem {
                name: kind.title().to_string(),
                description,
                actions,
                dismiss_on_select: true,
                ..Default::default()
            }
        })
        .collect();

        self.bottom_pane.show_selection_view(SelectionViewParams {
            title: Some("Settings".to_string()),
            subtitle: Some("Configure settings for Codex.".to_string()),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn open_realtime_audio_device_selection(&mut self, kind: RealtimeAudioDeviceKind) {
        match list_realtime_audio_device_names(kind) {
            Ok(device_names) => {
                self.open_realtime_audio_device_selection_with_names(kind, device_names);
            }
            Err(err) => {
                self.add_error_message(format!(
                    "Failed to load realtime {} devices: {err}",
                    kind.noun()
                ));
            }
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn open_realtime_audio_device_selection(&mut self, kind: RealtimeAudioDeviceKind) {
        let _ = kind;
    }

    #[cfg(not(target_os = "linux"))]
    pub(super) fn open_realtime_audio_device_selection_with_names(
        &mut self,
        kind: RealtimeAudioDeviceKind,
        device_names: Vec<String>,
    ) {
        let current_selection = self.current_realtime_audio_device_name(kind);
        let current_available = current_selection
            .as_deref()
            .is_some_and(|name| device_names.iter().any(|device_name| device_name == name));
        let mut items = vec![SelectionItem {
            name: "System default".to_string(),
            description: Some("Use your operating system default device.".to_string()),
            is_current: current_selection.is_none(),
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::PersistRealtimeAudioDeviceSelection { kind, name: None });
            })],
            dismiss_on_select: true,
            ..Default::default()
        }];

        if let Some(selection) = current_selection.as_deref()
            && !current_available
        {
            items.push(SelectionItem {
                name: format!("Unavailable: {selection}"),
                description: Some("Configured device is not currently available.".to_string()),
                is_current: true,
                is_disabled: true,
                disabled_reason: Some("Reconnect the device or choose another one.".to_string()),
                ..Default::default()
            });
        }

        items.extend(device_names.into_iter().map(|device_name| {
            let persisted_name = device_name.clone();
            let actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
                tx.send(AppEvent::PersistRealtimeAudioDeviceSelection {
                    kind,
                    name: Some(persisted_name.clone()),
                });
            })];
            SelectionItem {
                is_current: current_selection.as_deref() == Some(device_name.as_str()),
                name: device_name,
                actions,
                dismiss_on_select: true,
                ..Default::default()
            }
        }));

        let mut header = ColumnRenderable::new();
        header.push(Line::from(format!("Select {}", kind.title()).bold()));
        header.push(Line::from(
            "Saved devices apply to realtime voice only.".dim(),
        ));

        self.bottom_pane.show_selection_view(SelectionViewParams {
            header: Box::new(header),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    pub(crate) fn open_realtime_audio_restart_prompt(&mut self, kind: RealtimeAudioDeviceKind) {
        let restart_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            tx.send(AppEvent::RestartRealtimeAudioDevice { kind });
        })];
        let items = vec![
            SelectionItem {
                name: "Restart now".to_string(),
                description: Some(format!("Restart local {} audio now.", kind.noun())),
                actions: restart_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Apply later".to_string(),
                description: Some(format!(
                    "Keep the current {} until local audio starts again.",
                    kind.noun()
                )),
                dismiss_on_select: true,
                ..Default::default()
            },
        ];

        let mut header = ColumnRenderable::new();
        header.push(Line::from(format!("Restart {} now?", kind.title()).bold()));
        header.push(Line::from(
            "Configuration is saved. Restart local audio to use it immediately.".dim(),
        ));

        self.bottom_pane.show_selection_view(SelectionViewParams {
            header: Box::new(header),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    pub(crate) fn open_experimental_popup(&mut self) {
        let features: Vec<ExperimentalFeatureItem> = FEATURES
            .iter()
            .filter_map(|spec| {
                let name = spec.stage.experimental_menu_name()?;
                let description = spec.stage.experimental_menu_description()?;
                Some(ExperimentalFeatureItem {
                    feature: spec.id,
                    name: name.to_string(),
                    description: description.to_string(),
                    enabled: self.config.features.enabled(spec.id),
                })
            })
            .collect();

        let view = ExperimentalFeaturesView::new(
            features,
            self.app_event_tx.clone(),
            self.bottom_pane.list_keymap(),
        );
        self.bottom_pane.show_view(Box::new(view));
    }

    fn personality_label(personality: Personality) -> &'static str {
        match personality {
            Personality::None => "None",
            Personality::Friendly => "Friendly",
            Personality::Pragmatic => "Pragmatic",
        }
    }

    fn personality_description(personality: Personality) -> &'static str {
        match personality {
            Personality::None => "No personality instructions.",
            Personality::Friendly => "Warm, collaborative, and helpful.",
            Personality::Pragmatic => "Concise, task-focused, and direct.",
        }
    }
}
