use super::App;
use super::AppServerSession;
use super::Result;
use super::RuntimeKeymap;
use super::TuiEvent;
use super::make_test_app;
use super::start_config_write_test_app_server;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use crate::keymap::KeymapContext;
use crate::test_support::test_path_display;
use crate::tui::Tui;
use codex_app_server_protocol::ToolRequestUserInputOption;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_app_server_protocol::ToolRequestUserInputQuestion;
use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

async fn chord_app() -> Result<(App, Tui, AppServerSession)> {
    let mut app = make_test_app().await;
    let mut config = TuiKeymap::default();
    config.global.open_transcript = Some(KeybindingsSpec::One(KeybindingSpec(
        "ctrl-x ctrl-t".to_string(),
    )));
    config.chat.interrupt_turn = Some(KeybindingsSpec::One(KeybindingSpec(
        "ctrl-x ctrl-u".to_string(),
    )));
    config.list.move_down = Some(KeybindingsSpec::One(KeybindingSpec("ctrl-x j".to_string())));
    config.list.accept = Some(KeybindingsSpec::One(KeybindingSpec(
        "ctrl-x enter".to_string(),
    )));
    let runtime =
        RuntimeKeymap::from_config(&config).map_err(|error| color_eyre::eyre::eyre!(error))?;
    app.chat_widget.apply_keymap_update(config, &runtime);
    app.keymap = runtime;

    let app_server = start_config_write_test_app_server(&app).await?;
    let tui = crate::tui::test_support::make_test_tui()?;
    Ok((app, tui, app_server))
}

async fn press(
    app: &mut App,
    tui: &mut Tui,
    app_server: &mut AppServerSession,
    key: KeyEvent,
) -> Result<()> {
    app.handle_tui_event(tui, app_server, TuiEvent::Key(key))
        .await?;
    Ok(())
}

fn ctrl(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL)
}

#[tokio::test]
async fn vim_buffer_jumps_route_default_chords_in_normal_and_operator_contexts() -> Result<()> {
    for (input, command, expected) in [
        ("one\ntwo\nthree", "gg", "!one\ntwo\nthree"),
        ("one\ntwo\nthree", "dgg", "!"),
        ("ag bg", "0fg", "a!g bg"),
        ("ag bg", "0dfg", "! bg"),
    ] {
        let (mut app, mut tui, mut app_server) = chord_app().await?;
        app.chat_widget.toggle_vim_mode_and_notify();
        app.chat_widget.insert_str(input);

        for (index, key) in command.chars().enumerate() {
            press(
                &mut app,
                &mut tui,
                &mut app_server,
                KeyCode::Char(key).into(),
            )
            .await?;
            if key == 'g' && index + 1 < command.len() {
                assert!(app.key_chord_matcher.is_pending());
            }
        }

        assert!(!app.key_chord_matcher.is_pending());
        app.chat_widget.insert_str("!");
        assert_eq!(app.chat_widget.composer_text_with_pending(), expected);
    }
    Ok(())
}

#[tokio::test]
async fn completed_global_chord_reuses_the_existing_action_handler() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    assert!(app.key_chord_matcher.is_pending());
    assert!(app.overlay.is_none());
    insta::assert_snapshot!(
        render_bottom_popup(&app.chat_widget, /*width*/ 80)
            .replace(&test_path_display("/tmp/project"), "/tmp/project"),
        @"
    › Ask Codex to do anything

      ctrl+x … waiting for next key · esc cancel
    "
    );

    press(&mut app, &mut tui, &mut app_server, ctrl('t')).await?;
    assert!(!app.key_chord_matcher.is_pending());
    assert!(app.overlay.is_some());
    Ok(())
}

#[tokio::test]
async fn completed_global_chords_toggle_output_and_request_external_editor() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    let config = toml::from_str(
        "[global]\ntoggle_raw_output = [\"ctrl-x r\"]\nopen_external_editor = [\"ctrl-x e\"]",
    )?;
    app.keymap = RuntimeKeymap::from_config(&config).expect("valid global chords");
    app.chat_widget.apply_keymap_update(config, &app.keymap);

    for key in [ctrl('x'), KeyCode::Char('r').into()] {
        press(&mut app, &mut tui, &mut app_server, key).await?;
    }
    assert!(app.chat_widget.raw_output_mode());

    for key in [ctrl('x'), KeyCode::Char('e').into()] {
        press(&mut app, &mut tui, &mut app_server, key).await?;
    }
    assert_eq!(
        app.chat_widget.external_editor_state(),
        super::ExternalEditorState::Requested
    );
    Ok(())
}

#[tokio::test]
async fn wrong_second_stroke_passes_through_but_escape_is_consumed() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    let wrong_second = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    assert_eq!(
        app.route_key_chord_event(&mut tui, wrong_second),
        Some(wrong_second)
    );

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(app.route_key_chord_event(&mut tui, escape), None);
    assert!(!app.key_chord_matcher.is_pending());
    assert!(!app.backtrack.primed);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_terminal_recovers_vim_escape_before_normal_commands() -> Result<()> {
    for (key, expected) in [
        (KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT), "ab"),
        (
            KeyEvent::new(KeyCode::Char('D'), KeyModifiers::ALT | KeyModifiers::SHIFT),
            "ab",
        ),
        (KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT), "abc"),
    ] {
        let (mut app, mut tui, mut app_server) = chord_app().await?;
        app.chat_widget.toggle_vim_mode_and_notify();
        app.chat_widget.insert_str("abc");
        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyCode::Char('i').into(),
        )
        .await?;

        press(&mut app, &mut tui, &mut app_server, key).await?;

        assert_eq!(app.chat_widget.composer_text_with_pending(), expected);
        assert!(
            !app.chat_widget
                .should_handle_vim_insert_escape(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        );
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_terminal_preserves_default_and_configured_editor_alt_bindings() -> Result<()> {
    for (key, expected) in [
        ('b', "!abc def"),
        ('f', "abc! def"),
        ('d', "! def"),
        ('k', "!"),
    ] {
        let (mut app, mut tui, mut app_server) = chord_app().await?;
        if key == 'k' {
            let mut config = TuiKeymap::default();
            config.editor.kill_line_end =
                Some(KeybindingsSpec::One(KeybindingSpec("alt-k".to_string())));
            let runtime = RuntimeKeymap::from_config(&config)
                .map_err(|error| color_eyre::eyre::eyre!(error))?;
            app.chat_widget.apply_keymap_update(config, &runtime);
            app.keymap = runtime;
        }
        app.chat_widget.toggle_vim_mode_and_notify();
        app.chat_widget.insert_str("abc def");
        let insert = KeyCode::Char('i').into();
        press(&mut app, &mut tui, &mut app_server, insert).await?;
        press(&mut app, &mut tui, &mut app_server, ctrl('a')).await?;
        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyEvent::new(KeyCode::Char(key), KeyModifiers::ALT),
        )
        .await?;

        assert!(
            app.chat_widget
                .should_handle_vim_insert_escape(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        );
        app.chat_widget.insert_str("!");
        assert_eq!(app.chat_widget.composer_text_with_pending(), expected);
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_terminal_preserves_active_global_alt_shortcuts() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    app.chat_widget.toggle_vim_mode_and_notify();
    app.chat_widget.insert_str("abc");
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('i').into(),
    )
    .await?;

    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT),
    )
    .await?;

    assert!(app.chat_widget.raw_output_mode());
    assert!(
        app.chat_widget
            .should_handle_vim_insert_escape(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_terminal_preserves_configured_composer_alt_shortcuts() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    let mut config = TuiKeymap::default();
    config.composer.history_search_previous =
        Some(KeybindingsSpec::One(KeybindingSpec("alt-q".to_string())));
    let runtime =
        RuntimeKeymap::from_config(&config).map_err(|error| color_eyre::eyre::eyre!(error))?;
    app.chat_widget.apply_keymap_update(config, &runtime);
    app.keymap = runtime;
    app.chat_widget.toggle_vim_mode_and_notify();
    app.chat_widget.insert_str("abc");
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('i').into(),
    )
    .await?;

    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT),
    )
    .await?;

    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("reverse-i-search:"));
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_terminal_preserves_active_alt_chords() -> Result<()> {
    for (binding, prefix, completion) in [
        (
            "alt-q r",
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT),
            KeyCode::Char('r').into(),
        ),
        (
            "ctrl-x alt-h",
            ctrl('x'),
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT),
        ),
    ] {
        let (mut app, mut tui, mut app_server) = chord_app().await?;
        let mut config = TuiKeymap::default();
        config.global.toggle_raw_output =
            Some(KeybindingsSpec::One(KeybindingSpec(binding.to_string())));
        let runtime =
            RuntimeKeymap::from_config(&config).map_err(|error| color_eyre::eyre::eyre!(error))?;
        app.chat_widget.apply_keymap_update(config, &runtime);
        app.keymap = runtime;
        app.chat_widget.toggle_vim_mode_and_notify();
        app.chat_widget.insert_str("abc");
        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyCode::Char('i').into(),
        )
        .await?;

        press(&mut app, &mut tui, &mut app_server, prefix).await?;
        assert!(app.key_chord_matcher.is_pending());
        press(&mut app, &mut tui, &mut app_server, completion).await?;

        assert!(app.chat_widget.raw_output_mode());
        assert!(!app.key_chord_matcher.is_pending());
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_terminal_preserves_image_paste_without_reading_clipboard() -> Result<()> {
    for mode in ['i', 'R'] {
        let (mut app, mut tui, mut app_server) = chord_app().await?;
        app.chat_widget.toggle_vim_mode_and_notify();
        app.chat_widget.insert_str("abc");
        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyCode::Char(mode).into(),
        )
        .await?;
        for key in [
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT),
            KeyEvent::new(KeyCode::Char('V'), KeyModifiers::ALT | KeyModifiers::SHIFT),
        ] {
            assert!(!app.should_recover_vim_insert_escape(key));
        }
        assert!(app.should_recover_vim_insert_escape(KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::ALT,
        )));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn legacy_terminal_preserves_agent_shortcuts_without_editor_word_bindings() -> Result<()> {
    for mode in ['i', 'R'] {
        let (mut app, mut tui, mut app_server) = chord_app().await?;
        let mut config = TuiKeymap::default();
        config.editor.move_word_left = Some(KeybindingsSpec::Many(vec![]));
        config.editor.move_word_right = Some(KeybindingsSpec::Many(vec![]));
        let runtime =
            RuntimeKeymap::from_config(&config).map_err(|error| color_eyre::eyre::eyre!(error))?;
        app.chat_widget.apply_keymap_update(config, &runtime);
        app.keymap = runtime;
        app.chat_widget.toggle_vim_mode_and_notify();
        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyCode::Char(mode).into(),
        )
        .await?;

        for key in ['b', 'f'] {
            let event = KeyEvent::new(KeyCode::Char(key), KeyModifiers::ALT);
            assert!(!app.should_recover_vim_insert_escape(event));
        }
        app.chat_widget.insert_str("abc");
        for key in ['b', 'f'] {
            let event = KeyEvent::new(KeyCode::Char(key), KeyModifiers::ALT);
            assert!(app.should_recover_vim_insert_escape(event));
        }
    }
    Ok(())
}

#[tokio::test]
async fn vim_escape_recovery_preserves_enhanced_terminals_and_altgr() -> Result<()> {
    for (enhanced_keys_supported, modifiers) in [
        (true, KeyModifiers::ALT),
        (false, KeyModifiers::ALT | KeyModifiers::CONTROL),
    ] {
        let (mut app, mut tui, mut app_server) = chord_app().await?;
        app.enhanced_keys_supported = enhanced_keys_supported;
        app.chat_widget.toggle_vim_mode_and_notify();
        app.chat_widget.insert_str("abc");
        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyCode::Char('i').into(),
        )
        .await?;

        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyEvent::new(KeyCode::Char('x'), modifiers),
        )
        .await?;

        assert!(
            app.chat_widget
                .should_handle_vim_insert_escape(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        );
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn recovered_vim_escape_cancels_a_pending_key_chord_first() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    app.chat_widget.toggle_vim_mode_and_notify();
    app.chat_widget.insert_str("abc");
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('i').into(),
    )
    .await?;
    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    assert!(app.key_chord_matcher.is_pending());

    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT),
    )
    .await?;

    assert!(!app.key_chord_matcher.is_pending());
    press(&mut app, &mut tui, &mut app_server, KeyCode::Left.into()).await?;
    assert_eq!(app.chat_widget.composer_text_with_pending(), "abcx");
    assert!(
        app.chat_widget
            .should_handle_vim_insert_escape(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
    );
    Ok(())
}

#[tokio::test]
async fn physical_dispatch_band_events_are_dropped() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    let (code, modifiers) = app
        .keymap
        .app
        .open_transcript
        .last()
        .expect("configured chord appends a dispatch token")
        .parts();

    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyEvent::new(code, modifiers),
    )
    .await?;

    assert!(app.overlay.is_none());
    Ok(())
}

#[tokio::test]
async fn physical_chords_route_list_and_mixed_request_input_modals() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    app.chat_widget.show_selection_view(SelectionViewParams {
        appearance: crate::bottom_pane::SelectionAppearance::Picker,
        view_id: Some("list"),
        items: ["First", "Second"]
            .into_iter()
            .map(|name| SelectionItem {
                name: name.to_string(),
                dismiss_on_select: true,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    });
    assert_eq!(
        app.chat_widget.selected_index_for_present_view("list"),
        Some(0)
    );

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    assert!(app.key_chord_matcher.is_pending());
    assert_eq!(
        app.chat_widget.selected_index_for_present_view("list"),
        Some(0)
    );
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('j').into(),
    )
    .await?;
    assert_eq!(
        app.chat_widget.selected_index_for_present_view("list"),
        Some(1)
    );

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    press(&mut app, &mut tui, &mut app_server, KeyCode::Enter.into()).await?;
    assert_eq!(
        app.chat_widget.selected_index_for_present_view("list"),
        None
    );

    app.chat_widget
        .handle_request_user_input_now(ToolRequestUserInputParams {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "call-1".to_string(),
            questions: vec![ToolRequestUserInputQuestion {
                id: "choice".to_string(),
                header: "Pick one".to_string(),
                question: "Choose an option.".to_string(),
                is_other: false,
                is_secret: false,
                options: Some(
                    ["First", "Second"]
                        .into_iter()
                        .map(|label| ToolRequestUserInputOption {
                            label: label.to_string(),
                            description: label.to_string(),
                        })
                        .collect(),
                ),
            }],
            is_blocking: true,
            auto_resolution_ms: None,
        });
    let contexts = app.chat_widget.keymap_contexts();
    assert!(contexts.contains(KeymapContext::Chat));
    assert!(contexts.contains(KeymapContext::List));
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("› 1. First"));

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    assert!(app.key_chord_matcher.is_pending());
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('j').into(),
    )
    .await?;
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("› 2. Second"));

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    press(&mut app, &mut tui, &mut app_server, ctrl('u')).await?;
    assert!(app.chat_widget.can_launch_external_editor());
    Ok(())
}

#[tokio::test]
async fn dashboard_chord_hint_survives_refresh_and_clears_on_cancel() -> Result<()> {
    let mut app = make_test_app().await;
    app.keymap =
        RuntimeKeymap::from_config(&toml::from_str("[agents]\nnew_task = [\"ctrl-x n\"]")?)
            .unwrap();
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let before = render_bottom_popup(&app.chat_widget, /*width*/ 80);
    assert_eq!(app.route_key_chord_event(&mut tui, ctrl('x')), None);
    let _ = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    insta::assert_snapshot!(
        render_bottom_popup(&app.chat_widget, /*width*/ 80).lines().last().unwrap(),
        @"  ctrl+x … waiting for next key  esc cancel"
    );
    assert_eq!(
        app.route_key_chord_event(&mut tui, KeyCode::Esc.into()),
        None
    );
    assert_eq!(render_bottom_popup(&app.chat_widget, /*width*/ 80), before);
    Ok(())
}

#[tokio::test]
async fn command_center_chords_do_not_capture_search_text() -> Result<()> {
    let mut app = make_test_app().await;
    app.keymap = RuntimeKeymap::from_config(&toml::from_str(
        "[agents]\nnew_task = 'n n'\n[ list ]\naccept = 'ctrl-x enter'",
    )?)
    .unwrap();
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    app.chat_widget.handle_key_event(KeyCode::Char('f').into());
    for key in "new".chars() {
        let event = KeyCode::Char(key).into();
        assert_eq!(app.route_key_chord_event(&mut tui, event), Some(event));
        app.chat_widget.handle_key_event(event);
    }
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Search › new"));
    assert_eq!(app.route_key_chord_event(&mut tui, ctrl('x')), None);
    assert!(app.key_chord_matcher.is_pending());
    app.route_key_chord_event(&mut tui, KeyCode::Esc.into());
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    assert_eq!(
        app.route_key_chord_event(&mut tui, KeyCode::Char('n').into()),
        None
    );
    assert!(app.key_chord_matcher.is_pending());
    Ok(())
}
