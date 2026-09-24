//! Covers confirmed new tasks, existing attachment paths, and the user model picker.

use super::*;
use crate::app::session_lifecycle::ThreadAttachPresentation;
use crate::chatwidget::AstraModelPickerAction;
use crate::terminal_palette::with_test_default_colors;
use crate::terminal_probe::DefaultColors;
use codex_protocol::openai_models::ReasoningEffortPreset;
use pretty_assertions::assert_eq;

fn started(model: &str) -> AppServerStartedThread {
    let mut session = test_thread_session(ThreadId::new(), test_path_buf("/tmp/project"));
    session.model = model.into();
    AppServerStartedThread {
        session,
        turns: Vec::new(),
        blocks_direct_input: false,
        task_tools_available: false,
    }
}

fn render_with_sparkle_palette(chat: &ChatWidget) -> String {
    with_test_default_colors(
        DefaultColors {
            fg: (230, 216, 255),
            bg: (36, 27, 53),
        },
        || render_bottom_popup(chat, /*width*/ 80),
    )
}

fn visible(chat: &ChatWidget) -> bool {
    render_with_sparkle_palette(chat)
        .chars()
        .any(|ch| "⠁⠂⠄⠈⠐⠠⡀⢀".contains(ch))
}

fn type_into(chat: &mut ChatWidget, text: &str) {
    for ch in text.chars() {
        chat.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
    }
}

async fn choose_model(
    app: &mut App,
    tui: &mut crate::tui::Tui,
    server: &mut AppServerSession,
    events: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    model: &str,
    key: KeyCode,
) -> Result<usize> {
    let mut preset = crate::test_support::TEST_MODEL_PRESETS[0].clone();
    preset.model = model.into();
    preset.display_name = model.into();
    preset.show_in_picker = true;
    preset.supported_reasoning_efforts.truncate(/*len*/ 1);
    preset.default_reasoning_effort = preset.supported_reasoning_efforts[0].effort.clone();
    app.chat_widget.open_model_popup_with_presets(vec![preset]);
    app.chat_widget
        .handle_key_event(KeyEvent::new(key, KeyModifiers::NONE));

    let mut sparkle_offers = 0;
    while let Ok(event) = events.try_recv() {
        if matches!(
            &event,
            AppEvent::OpenReasoningPopup { .. }
                | AppEvent::UpdateModel(_)
                | AppEvent::SelectSessionModel { .. }
                | AppEvent::AstraSelectedFromModelPicker { .. }
        ) {
            if let AppEvent::AstraSelectedFromModelPicker { thread_id, .. } = &event {
                assert_eq!(Some(*thread_id), app.chat_widget.thread_id());
            }
            let changes_model = matches!(&event, AppEvent::AstraSelectedFromModelPicker { model, .. }
                if app.chat_widget.current_model() != model);
            app.handle_event(tui, server, event).await?;
            if changes_model && app.chat_widget.current_model() == model {
                sparkle_offers += 1;
            }
        }
    }
    assert!(!app.chat_widget.has_active_view());
    Ok(sparkle_offers)
}

#[tokio::test]
async fn only_a_confirmed_empty_new_task_shows_the_sparkle() -> Result<()> {
    for scenario in [
        "fresh",
        "empty_resume",
        "fork",
        "replay",
        "initial_prompt",
        "parent_owned",
    ] {
        let (mut app, _events, _ops) = make_test_app_with_channels().await;
        app.local_settings.tui.animations = true;
        app.local_settings.tui.effects.starfield = true;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let mut thread = started("gpt-6-astra");
        let presentation = if scenario == "empty_resume" {
            ThreadAttachPresentation::SessionLineage
        } else {
            ThreadAttachPresentation::Fresh
        };
        if scenario == "fork" {
            thread.session.forked_from_id = Some(ThreadId::new());
        }
        if scenario == "replay" {
            thread
                .turns
                .push(test_turn("existing", TurnStatus::Completed, Vec::new()));
        }
        if scenario == "parent_owned" {
            thread.blocks_direct_input = true;
        }
        let initial_message = (scenario == "initial_prompt")
            .then(|| create_initial_user_message(Some("hello".into()), Vec::new(), Vec::new()))
            .flatten();
        app.replace_chat_widget_with_app_server_thread(
            &mut tui,
            thread,
            presentation,
            initial_message,
        )
        .await?;
        assert_eq!(visible(&app.chat_widget), scenario == "fresh", "{scenario}");
        app.chat_widget.set_model("gpt-5.5");
        app.chat_widget.set_model("gpt-6-astra");
        assert!(
            !visible(&app.chat_widget),
            "automatic model update: {scenario}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn disconnected_sparkle_input_routed_by_app_consumes_the_opportunity_without_drawing()
-> Result<()> {
    for model in ["gpt-5.5", "gpt-6-astra"] {
        for edited in [false, true] {
            let (mut app, _events, _ops) = make_test_app_with_channels().await;
            app.local_settings.tui.animations = true;
            app.local_settings.tui.effects.starfield = true;
            let mut server = start_config_write_test_app_server(&app).await?;
            let mut tui = crate::tui::test_support::make_test_tui()?;
            app.replace_chat_widget_with_app_server_thread(
                &mut tui,
                started(model),
                ThreadAttachPresentation::Fresh,
                /*initial_user_message*/ None,
            )
            .await?;
            assert_eq!(visible(&app.chat_widget), model == "gpt-6-astra");
            app.app_server_target = AppServerTarget::Remote {
                endpoint: crate::resolve_remote_addr("ws://127.0.0.1:9")?,
            };
            assert!(app.begin_reconnect());
            if edited {
                for code in [KeyCode::Char('?'), KeyCode::Backspace] {
                    app.handle_tui_event(
                        &mut tui,
                        &mut server,
                        TuiEvent::Key(KeyEvent::new(code, KeyModifiers::NONE)),
                    )
                    .await?;
                }
            }
            assert_eq!(app.chat_widget.composer_text_with_pending(), "");
            app.chat_widget.set_footer_hint_override(/*items*/ None);
            app.chat_widget.pause_unavailable_thread();
            app.chat_widget.set_model("gpt-6-astra");
            app.chat_widget
                .on_sparkle_model_selected_from_picker("gpt-6-astra");
            assert_eq!(
                visible(&app.chat_widget),
                !edited,
                "{model}, edited: {edited}"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn commands_can_precede_the_sparkle_but_inserted_or_typed_drafts_cannot() -> Result<()> {
    for earlier_input in [
        "",
        "commands",
        "shortcut_help",
        "mention",
        "typed",
        "paste",
        "paste_question_mark",
        "cancel_picker",
    ] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        app.local_settings.tui.animations = true;
        app.local_settings.tui.effects.starfield = true;
        app.local_settings.tui.disable_paste_burst = Some(true);
        let mut server = start_config_write_test_app_server(&app).await?;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        app.replace_chat_widget_with_app_server_thread(
            &mut tui,
            started("gpt-5.5"),
            ThreadAttachPresentation::Fresh,
            /*initial_user_message*/ None,
        )
        .await?;
        assert!(!visible(&app.chat_widget));
        match earlier_input {
            "typed" => {
                type_into(&mut app.chat_widget, "x");
                app.chat_widget
                    .handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            }
            "paste" | "paste_question_mark" => {
                let pasted = if earlier_input == "paste" { "x" } else { "?" };
                app.chat_widget.handle_paste(pasted.into());
                app.chat_widget
                    .handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            }
            "shortcut_help" => {
                app.chat_widget
                    .handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT));
                assert!(
                    render_bottom_popup(&app.chat_widget, /*width*/ 80)
                        .contains("Keyboard shortcuts")
                );
            }
            "commands" => {
                for command in ["/status", "/pwd"] {
                    type_into(&mut app.chat_widget, command);
                    app.chat_widget
                        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }
            }
            "mention" => {
                type_into(&mut app.chat_widget, "/mention");
                app.chat_widget
                    .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                app.chat_widget
                    .handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            }
            "" | "cancel_picker" => {}
            _ => unreachable!(),
        }
        type_into(&mut app.chat_widget, "/model");
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        if earlier_input == "cancel_picker" {
            assert!(app.chat_widget.has_active_view());
            app.chat_widget
                .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
            app.chat_widget.set_model("gpt-6-astra");
        } else {
            assert_eq!(
                choose_model(
                    &mut app,
                    &mut tui,
                    &mut server,
                    &mut events,
                    "gpt-6-astra",
                    KeyCode::Enter,
                )
                .await?,
                1
            );
        }
        assert_eq!(
            visible(&app.chat_widget),
            matches!(earlier_input, "" | "commands" | "shortcut_help"),
            "{earlier_input}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn overlays_shortcuts_and_key_chords_leave_the_main_sparkle_untouched() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    app.local_settings.tui.animations = true;
    app.local_settings.tui.effects.starfield = true;
    let mut server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.replace_chat_widget_with_app_server_thread(
        &mut tui,
        started("gpt-6-astra"),
        ThreadAttachPresentation::Fresh,
        /*initial_user_message*/ None,
    )
    .await?;
    let keymap_config = toml::from_str("[global]\ntoggle_raw_output = [\"ctrl-x r\"]")?;
    app.keymap = RuntimeKeymap::from_config(&keymap_config).expect("valid app chord");
    app.chat_widget
        .apply_keymap_update(keymap_config, &app.keymap);
    assert!(visible(&app.chat_widget));

    let ctrl = |ch| KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL);
    for (event, pending, raw) in [
        (ctrl('x'), true, false),
        (KeyCode::Char('r').into(), false, true),
        (ctrl('x'), true, true),
        (KeyCode::Esc.into(), false, true),
    ] {
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(event))
            .await?;
        assert_eq!(
            (
                app.key_chord_matcher.is_pending(),
                app.chat_widget.raw_output_mode(),
                visible(&app.chat_widget)
            ),
            (pending, raw, true)
        );
    }

    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(ctrl('t')))
        .await?;
    assert!(app.overlay.is_some());
    for event in [
        TuiEvent::Key(KeyCode::Char('j').into()),
        TuiEvent::Paste("overlay input".into()),
        TuiEvent::Key(ctrl('t')),
    ] {
        app.handle_tui_event(&mut tui, &mut server, event).await?;
    }
    assert!(app.overlay.is_none());
    assert_eq!(
        (
            app.chat_widget.composer_is_empty(),
            visible(&app.chat_widget)
        ),
        (true, true)
    );

    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into()))
        .await?;
    assert!(!visible(&app.chat_widget));
    Ok(())
}

#[tokio::test]
async fn a_command_pending_on_fresh_astra_start_can_finish_before_the_sparkle() -> Result<()> {
    for (command, visible_after) in [("/status", true), ("/mention", false)] {
        let (mut app, _events, _ops) = make_test_app_with_channels().await;
        app.chat_widget.local_settings.tui.animations = true;
        app.chat_widget.local_settings.tui.effects.starfield = true;
        app.pending_startup_thread_start = true;
        app.chat_widget.handle_paste(command.into());
        let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
        app.handle_startup_thread_started(&mut server, Ok(started("gpt-6-astra")))
            .await?;
        assert_eq!(
            (
                app.chat_widget.composer_text_with_pending(),
                visible(&app.chat_widget)
            ),
            (command.into(), false)
        );

        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        assert_eq!(visible(&app.chat_widget), visible_after, "{command}");
    }
    Ok(())
}

#[tokio::test]
async fn only_confirmed_picker_model_changes_can_arm_the_sparkle() -> Result<()> {
    for completion in [
        "esc",
        "ctrl_c",
        "same_model",
        "automatic_while_open",
        "pick_astra",
        "old_task",
    ] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        app.local_settings.tui.animations = true;
        app.local_settings.tui.effects.starfield = true;
        app.local_settings.tui.disable_paste_burst = Some(true);
        let mut server = start_config_write_test_app_server(&app).await?;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let thread = started("gpt-5.5");
        let original_id = thread.session.thread_id;
        app.replace_chat_widget_with_app_server_thread(
            &mut tui,
            thread,
            ThreadAttachPresentation::Fresh,
            /*initial_user_message*/ None,
        )
        .await?;
        assert!(!visible(&app.chat_widget));
        type_into(&mut app.chat_widget, "/model");
        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        assert!(app.chat_widget.has_active_view());

        match completion {
            "esc" | "ctrl_c" => {
                let key = if completion == "esc" {
                    KeyCode::Esc.into()
                } else {
                    KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
                };
                app.chat_widget.handle_key_event(key);
                assert!(!app.chat_widget.has_active_view());
                app.chat_widget.set_model("gpt-6-astra");
            }
            "same_model" => {
                assert_eq!(
                    choose_model(
                        &mut app,
                        &mut tui,
                        &mut server,
                        &mut events,
                        "gpt-5.5",
                        KeyCode::Enter,
                    )
                    .await?,
                    0
                );
                app.chat_widget.set_model("gpt-6-astra");
            }
            "automatic_while_open" => {
                app.chat_widget.set_model("gpt-6-astra");
                app.chat_widget
                    .handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
                assert!(!app.chat_widget.has_active_view());
            }
            "pick_astra" => {
                assert_eq!(
                    choose_model(
                        &mut app,
                        &mut tui,
                        &mut server,
                        &mut events,
                        "gpt-6-astra",
                        KeyCode::Enter,
                    )
                    .await?,
                    1
                );
            }
            "old_task" => {
                app.replace_chat_widget_with_app_server_thread(
                    &mut tui,
                    started("gpt-5.5"),
                    ThreadAttachPresentation::Fresh,
                    /*initial_user_message*/ None,
                )
                .await?;
                app.chat_widget.set_model("gpt-6-astra");
                app.handle_event(
                    &mut tui,
                    &mut server,
                    AppEvent::AstraSelectedFromModelPicker {
                        thread_id: original_id,
                        model: "gpt-6-astra".into(),
                        action: AstraModelPickerAction::UpdateModel,
                    },
                )
                .await?;
            }
            _ => unreachable!(),
        }
        assert_eq!(
            visible(&app.chat_widget),
            completion == "pick_astra",
            "{completion}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn astra_picker_confirms_the_model_at_application_after_an_automatic_update() -> Result<()> {
    let mut snapshots = Vec::new();
    for (picker, key, switches_away) in [
        ("default no-op", KeyCode::Enter, false),
        ("session no-op", KeyCode::Char('s'), false),
        ("advanced no-op", KeyCode::Enter, false),
        ("default queued no-op", KeyCode::Enter, false),
        ("session queued no-op", KeyCode::Char('s'), false),
        ("default changes model", KeyCode::Enter, true),
        ("session changes model", KeyCode::Char('s'), true),
        ("advanced changes model", KeyCode::Enter, true),
    ] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        app.config.model = Some("gpt-5.5".into());
        app.local_settings.tui.animations = true;
        app.local_settings.tui.effects.starfield = true;
        let mut server = start_config_write_test_app_server(&app).await?;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let thread = server.start_thread(&app.config).await?;
        app.replace_chat_widget_with_app_server_thread(
            &mut tui,
            thread,
            ThreadAttachPresentation::Fresh,
            /*initial_user_message*/ None,
        )
        .await?;
        if switches_away {
            app.chat_widget.set_model("gpt-6-astra");
        }
        assert!(!visible(&app.chat_widget));

        let mut preset = crate::test_support::TEST_MODEL_PRESETS[0].clone();
        preset.model = "gpt-6-astra".into();
        preset.default_reasoning_effort = if picker.starts_with("advanced") {
            ReasoningEffortConfig::Ultra
        } else {
            ReasoningEffortConfig::Medium
        };
        preset.supported_reasoning_efforts = vec![
            ReasoningEffortPreset {
                effort: preset.default_reasoning_effort.clone(),
                description: "Selected effort".into(),
            },
            ReasoningEffortPreset {
                effort: ReasoningEffortConfig::Low,
                description: "Low effort".into(),
            },
        ];
        if picker.starts_with("advanced") {
            app.chat_widget.open_advanced_reasoning_popup(preset);
        } else {
            app.chat_widget.open_reasoning_popup(preset);
        }
        assert!(app.chat_widget.has_active_view());
        if !picker.contains("queued") {
            app.chat_widget.set_model(if switches_away {
                "gpt-5.5"
            } else {
                "gpt-6-astra"
            });
            assert!(app.chat_widget.has_active_view());
        }
        app.chat_widget.handle_key_event(key.into());
        if picker.contains("queued") {
            app.chat_widget.set_model("gpt-6-astra");
        }
        while let Ok(event) = events.try_recv() {
            if matches!(
                &event,
                AppEvent::UpdateModel(_)
                    | AppEvent::UpdateReasoningEffort(_)
                    | AppEvent::SelectSessionModel { .. }
                    | AppEvent::ApplyAdvancedReasoning { .. }
                    | AppEvent::AstraSelectedFromModelPicker { .. }
            ) {
                app.handle_event(&mut tui, &mut server, event).await?;
            }
        }
        assert!(!app.chat_widget.has_active_view(), "{picker}");
        assert_eq!(app.chat_widget.current_model(), "gpt-6-astra", "{picker}");
        assert_eq!(visible(&app.chat_widget), switches_away, "{picker}");
        if picker.starts_with("default") && !picker.contains("queued") {
            let rendered = render_with_sparkle_palette(&app.chat_widget);
            let lines = rendered.lines().collect::<Vec<_>>();
            let prompt = lines
                .iter()
                .position(|line| line.contains("Ask Codex to do anything"))
                .expect("empty composer shows its placeholder");
            let composer = lines[prompt.saturating_sub(1)..=prompt + 1].join("\n");
            snapshots.push(format!("{picker}:\n{composer}"));
        }
        server.shutdown().await?;
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
    Ok(())
}

#[test]
fn astra_picker_xhigh_applies_on_a_bounded_stack() -> Result<()> {
    const STACK_SIZE: usize = 4 * 1024 * 1024;
    const SERVER_STACK_SIZE: usize = 8 * 1024 * 1024;
    const WORKER_THREADS: usize = 1;

    std::thread::Builder::new()
        .name("astra-picker".into())
        .stack_size(STACK_SIZE)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(WORKER_THREADS)
                .thread_stack_size(SERVER_STACK_SIZE)
                .enable_all()
                .build()?;
            runtime.block_on(Box::pin(async {
                let (mut app, mut events, _ops) = Box::pin(make_test_app_with_channels()).await;
                app.config.model = Some("gpt-5.5".into());
                let mut server = Box::pin(start_config_write_test_app_server(&app)).await?;
                let mut tui = crate::tui::test_support::make_test_tui()?;
                let thread = server.start_thread(&app.config).await?;
                Box::pin(app.replace_chat_widget_with_app_server_thread(
                    &mut tui,
                    thread,
                    ThreadAttachPresentation::Fresh,
                    /*initial_user_message*/ None,
                ))
                .await?;

                let mut preset = crate::test_support::TEST_MODEL_PRESETS[0].clone();
                preset.model = "gpt-6-astra".into();
                preset.default_reasoning_effort = ReasoningEffortConfig::XHigh;
                preset.supported_reasoning_efforts = vec![
                    ReasoningEffortPreset {
                        effort: ReasoningEffortConfig::XHigh,
                        description: "Extra High".into(),
                    },
                    ReasoningEffortPreset {
                        effort: ReasoningEffortConfig::Low,
                        description: "Low".into(),
                    },
                ];
                app.chat_widget.open_reasoning_popup(preset);
                app.chat_widget.handle_key_event(KeyCode::Enter.into());
                let mut confirmed = false;
                while let Ok(event) = events.try_recv() {
                    confirmed |= matches!(event, AppEvent::AstraSelectedFromModelPicker { .. });
                    if matches!(
                        &event,
                        AppEvent::AstraSelectedFromModelPicker { .. }
                            | AppEvent::UpdateReasoningEffort(_)
                            | AppEvent::PersistModelSelection { .. }
                    ) {
                        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
                    }
                }
                assert!(confirmed);
                assert_eq!(
                    (
                        app.chat_widget.current_model(),
                        app.chat_widget.current_reasoning_effort(),
                    ),
                    ("gpt-6-astra", Some(ReasoningEffortConfig::XHigh)),
                );
                server.shutdown().await?;
                Ok(())
            }))
        })?
        .join()
        .expect("Astra picker thread panicked")
}

#[tokio::test]
async fn session_only_astra_picker_shows_stars_only_on_an_untouched_task() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    app.config.model = Some("gpt-5.5".into());
    app.config.model_reasoning_effort = Some(ReasoningEffortConfig::Medium);
    app.local_settings.tui.animations = true;
    app.local_settings.tui.effects.starfield = true;
    app.local_settings.tui.disable_paste_burst = Some(true);
    let config_path = app.config.codex_home.join("config.toml");
    let defaults = "model = 'gpt-5.5'\nmodel_reasoning_effort = 'medium'\n";
    std::fs::write(&config_path, defaults)?;
    let mut server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut snapshots = Vec::new();

    for scenario in ["untouched", "typed then erased"] {
        let thread = server.start_thread(&app.config).await?;
        assert_eq!(thread.session.model, "gpt-5.5");
        app.replace_chat_widget_with_app_server_thread(
            &mut tui,
            thread,
            ThreadAttachPresentation::Fresh,
            /*initial_user_message*/ None,
        )
        .await?;
        assert!(!visible(&app.chat_widget));
        if scenario == "typed then erased" {
            type_into(&mut app.chat_widget, "x");
            app.chat_widget.handle_key_event(KeyCode::Backspace.into());
        }
        type_into(&mut app.chat_widget, "/model");
        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        assert!(app.chat_widget.has_active_view());
        assert_eq!(
            choose_model(
                &mut app,
                &mut tui,
                &mut server,
                &mut events,
                "gpt-6-astra",
                KeyCode::Char('s'),
            )
            .await?,
            1
        );
        assert_eq!(app.chat_widget.current_model(), "gpt-6-astra");

        let rendered = render_with_sparkle_palette(&app.chat_widget);
        let lines = rendered.lines().collect::<Vec<_>>();
        let prompt = lines
            .iter()
            .position(|line| line.contains("Ask Codex to do anything"))
            .expect("empty composer shows its placeholder");
        let composer = lines[prompt.saturating_sub(1)..=prompt + 1].join("\n");
        assert_eq!(
            composer.chars().any(|ch| "⠁⠂⠄⠈⠐⠠⡀⢀".contains(ch)),
            scenario == "untouched"
        );
        snapshots.push(format!("{scenario}:\n{composer}"));
        assert_eq!(std::fs::read(&config_path)?, defaults.as_bytes());
        assert_eq!(
            (&app.config.model, &app.config.model_reasoning_effort),
            (
                &Some("gpt-5.5".into()),
                &Some(ReasoningEffortConfig::Medium),
            )
        );

        type_into(&mut app.chat_widget, "x");
        app.chat_widget.handle_key_event(KeyCode::Backspace.into());
        assert_eq!(
            choose_model(
                &mut app,
                &mut tui,
                &mut server,
                &mut events,
                "gpt-6-astra",
                KeyCode::Char('s'),
            )
            .await?,
            0
        );
        assert!(!visible(&app.chat_widget));
    }

    insta::assert_snapshot!(snapshots.join("\n\n"));
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn a_read_only_command_can_return_to_active_stars_but_mention_ends_them() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    app.local_settings.tui.animations = true;
    app.local_settings.tui.effects.starfield = true;
    app.local_settings.tui.disable_paste_burst = Some(true);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.replace_chat_widget_with_app_server_thread(
        &mut tui,
        started("gpt-6-astra"),
        ThreadAttachPresentation::Fresh,
        /*initial_user_message*/ None,
    )
    .await?;
    assert!(visible(&app.chat_widget));
    type_into(&mut app.chat_widget, "/status");
    assert!(!visible(&app.chat_widget));
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(visible(&app.chat_widget));
    type_into(&mut app.chat_widget, "/mention");
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    assert!(!visible(&app.chat_widget));
    Ok(())
}

#[tokio::test]
async fn early_input_and_real_work_consume_the_sparkle() -> Result<()> {
    for scenario in ["untouched", "typed_then_erased", "paste", "turn", "review"] {
        let (mut app, _events, _ops) = make_test_app_with_channels().await;
        app.chat_widget.local_settings.tui.animations = true;
        app.chat_widget.local_settings.tui.effects.starfield = true;
        app.pending_startup_thread_start = true;
        match scenario {
            "typed_then_erased" => {
                app.chat_widget.handle_paste("x".into());
                app.chat_widget
                    .handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
            }
            "paste" => app.chat_widget.handle_paste("draft".into()),
            "untouched" | "turn" | "review" => {}
            _ => unreachable!(),
        }
        let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
        let thread = started("gpt-6-astra");
        let thread_id = thread.session.thread_id;
        app.handle_startup_thread_started(&mut server, Ok(thread))
            .await?;
        if scenario == "turn" {
            app.handle_app_server_event(
                &server,
                codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
                    turn_started_notification(thread_id, "work"),
                )),
            )
            .await;
            let mut tui = crate::tui::test_support::make_test_tui()?;
            app.drain_active_thread_events(&mut tui).await?;
        } else if scenario == "review" {
            app.chat_widget.on_review_started();
        }
        assert_eq!(
            visible(&app.chat_widget),
            scenario == "untouched",
            "{scenario}"
        );
    }
    Ok(())
}
