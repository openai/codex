//! Exercise session-only model selection through picker key events.

use super::*;

#[tokio::test]
async fn session_model_selection_accepts_final_choices_without_saving() {
    for picker in [
        "auto",
        "single",
        "default_only",
        "reasoning",
        "max",
        "ultra",
    ] {
        let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.5")).await;
        let effort = match picker {
            "max" => ReasoningEffortConfig::Max,
            "ultra" => ReasoningEffortConfig::Ultra,
            _ => ReasoningEffortConfig::High,
        };
        let mut preset = get_available_model(&chat, "gpt-5.5");
        preset.default_reasoning_effort = effort.clone();
        preset.supported_reasoning_efforts = vec![ReasoningEffortPreset {
            effort: effort.clone(),
            description: "Selected effort".into(),
        }];
        if picker == "auto" {
            preset.model = "codex-auto-test".into();
        }
        let expected_model = preset.model.clone();
        if picker == "default_only" {
            preset.supported_reasoning_efforts.clear();
        }
        match picker {
            "auto" | "single" | "default_only" => chat.open_model_popup_with_presets(vec![preset]),
            "max" | "ultra" => chat.open_advanced_reasoning_popup(preset),
            "reasoning" => {
                chat.set_reasoning_effort(Some(effort.clone()));
                preset
                    .supported_reasoning_efforts
                    .push(ReasoningEffortPreset {
                        effort: ReasoningEffortConfig::Low,
                        description: "Low effort".into(),
                    });
                chat.open_reasoning_popup(preset);
            }
            _ => unreachable!(),
        }
        while rx.try_recv().is_ok() {}
        chat.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
        let selected = rx.try_recv().expect("session-only selection");
        assert_matches!(selected, AppEvent::SelectSessionModel { model, effort: selected_effort }
            if model == expected_model && selected_effort.as_ref() == Some(&effort));
        assert!(chat.bottom_pane.no_modal_or_popup_active(), "{picker}");
        assert!(
            std::iter::from_fn(|| rx.try_recv().ok()).all(|event| matches!(
                event,
                AppEvent::InsertHistoryCell(_) | AppEvent::SettingsSelectionClosed
            )),
            "{picker}"
        );
    }
}

#[tokio::test]
async fn session_model_selection_notifies_the_original_task_after_each_final_astra_choice() {
    for picker in ["model", "reasoning", "advanced"] {
        let (mut chat, mut events, _ops) = make_chatwidget_manual(Some("gpt-5.5")).await;
        let thread_id = ThreadId::new();
        chat.thread_id = Some(thread_id);
        let mut preset = get_available_model(&chat, "gpt-5.5");
        preset.model = "gpt-6-astra".into();
        let effort = if picker == "advanced" {
            ReasoningEffortConfig::Max
        } else {
            ReasoningEffortConfig::High
        };
        preset.default_reasoning_effort = effort.clone();
        preset.supported_reasoning_efforts = vec![ReasoningEffortPreset {
            effort: effort.clone(),
            description: "Selected effort".into(),
        }];
        match picker {
            "model" => chat.open_model_popup_with_presets(vec![preset]),
            "reasoning" => {
                preset
                    .supported_reasoning_efforts
                    .push(ReasoningEffortPreset {
                        effort: ReasoningEffortConfig::Low,
                        description: "Low effort".into(),
                    });
                chat.open_reasoning_popup(preset);
            }
            "advanced" => chat.open_advanced_reasoning_popup(preset),
            _ => unreachable!(),
        }
        while events.try_recv().is_ok() {}
        chat.handle_key_event(KeyCode::Char('s').into());

        assert_matches!(events.try_recv(), Ok(AppEvent::AstraSelectedFromModelPicker {
            thread_id: selected_thread,
            model,
            action: AstraModelPickerAction::SelectSessionModel { effort: selected_effort },
        }) if selected_thread == thread_id && model == "gpt-6-astra" && selected_effort == Some(effort));
        assert!(
            std::iter::from_fn(|| events.try_recv().ok()).all(|event| matches!(
                event,
                AppEvent::InsertHistoryCell(_) | AppEvent::SettingsSelectionClosed
            ))
        );
        assert!(chat.bottom_pane.no_modal_or_popup_active(), "{picker}");
    }
}

#[tokio::test]
async fn session_model_selection_hides_conflicting_shortcut() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.5")).await;
    let mut config = codex_config::types::TuiKeymap::default();
    config.list.accept = Some(codex_config::types::KeybindingsSpec::One(
        codex_config::types::KeybindingSpec("s".into()),
    ));
    let keymap = RuntimeKeymap::from_config(&config).expect("valid list keymap");
    chat.bottom_pane.set_keymap_bindings(&keymap);
    let preset = get_available_model(&chat, "gpt-5.5");
    chat.open_reasoning_popup(preset);
    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(!popup.contains("for this session"), "{popup}");
    while rx.try_recv().is_ok() {}
    chat.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
    assert!(
        std::iter::from_fn(|| rx.try_recv().ok())
            .any(|event| matches!(event, AppEvent::PersistModelSelection { .. }))
    );
}
