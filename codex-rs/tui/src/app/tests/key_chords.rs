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
use crate::chatwidget::tests::helpers::set_active_cell;
use crate::keymap::KeymapContext;
#[cfg(unix)]
use crate::pager_overlay::TranscriptHistoryState;
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

async fn chord_app() -> Result<(Box<App>, Tui, AppServerSession)> {
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
async fn global_chord_keeps_hints_and_completes_before_deadline() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    assert!(app.key_chord_matcher.is_pending());
    assert!(app.overlay.is_none());
    tokio::time::pause();
    tokio::time::advance(std::time::Duration::from_millis(/*millis*/ 500)).await;
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
        .await?;
    assert!(app.key_chord_matcher.is_pending());
    insta::assert_snapshot!(
        render_bottom_popup(&app.chat_widget, /*width*/ 80)
            .replace(&test_path_display("/tmp/project"), "/tmp/project"),
        @r"
        › Ask Codex to do anything

          ctrl+x then · ctrl+t open transcript · ctrl+u interrupt turn · esc cancel
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
        "[global]\ntoggle_raw_output = [\"ctrl-x r\", \"pageup\"]\nopen_external_editor = [\"ctrl-x e\"]",
    )?;
    app.keymap = RuntimeKeymap::from_config(&config).expect("valid global chords");
    app.chat_widget.apply_keymap_update(config, &app.keymap);

    for key in [ctrl('x'), KeyCode::Char('r').into()] {
        press(&mut app, &mut tui, &mut app_server, key).await?;
    }
    assert!(app.chat_widget.raw_output_mode());
    tui.set_owned_screen(/*owned*/ true)?;
    press(&mut app, &mut tui, &mut app_server, KeyCode::PageUp.into()).await?;
    assert!(!app.chat_widget.raw_output_mode());
    let config = toml::from_str(
        "[composer]\nsubmit = 'pageup'\n[editor]\nmove_line_start = 'pagedown'\n[global]\nopen_external_editor = 'ctrl-x e'",
    )?;
    app.keymap = RuntimeKeymap::from_config(&config).expect("valid composer and editor bindings");
    app.chat_widget.apply_keymap_update(config, &app.keymap);
    app.chat_widget.apply_external_edit("draft".to_string());
    assert!(!app.handle_owned_transcript_event(
        &mut tui,
        &mut app_server,
        &TuiEvent::Key(KeyCode::PageUp.into()),
    )?);
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::PageDown.into(),
    )
    .await?;
    app.chat_widget.insert_str("X");
    assert_eq!(app.chat_widget.composer_text_with_pending(), "Xdraft");
    tui.set_owned_screen(/*owned*/ false)?;

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
async fn owned_transcript_alt_jumps_preserve_legacy_vim_insert_and_draft() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    app.enhanced_keys_supported = false;
    tui.set_owned_screen(/*owned*/ true)?;
    app.transcript_cells = vec![std::sync::Arc::new(
        crate::history_cell::PlainHistoryCell::new(vec!["Earlier transcript".into()]),
    )];
    app.chat_widget.toggle_vim_mode_and_notify();
    app.chat_widget.insert_str("draft");
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('i').into(),
    )
    .await?;

    let area = ratatui::layout::Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 4,
    );
    let mut buffer = ratatui::buffer::Buffer::empty(area);
    app.transcript_view
        .render(area, &mut buffer, &app.transcript_cells);

    for modifiers in [KeyModifiers::ALT, KeyModifiers::ALT | KeyModifiers::SHIFT] {
        app.transcript_view.history = TranscriptHistoryState::Partial;
        for (character, following, history) in [
            ('<', false, TranscriptHistoryState::LoadingBeginning),
            ('>', true, TranscriptHistoryState::LoadingOlder),
        ] {
            press(
                &mut app,
                &mut tui,
                &mut app_server,
                KeyEvent::new(KeyCode::Char(character), modifiers),
            )
            .await?;
            assert_eq!(
                (
                    app.transcript_view.is_following(),
                    app.transcript_view.history,
                    app.chat_widget.composer_text_with_pending(),
                    app.chat_widget
                        .should_handle_vim_insert_escape(KeyEvent::new(
                            KeyCode::Esc,
                            KeyModifiers::NONE,
                        )),
                ),
                (following, history, "draft".to_string(), true)
            );
        }
    }
    tui.set_owned_screen(/*owned*/ false)?;
    app_server.shutdown().await?;
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
    app.agents_overview
        .view_state
        .lock()
        .unwrap()
        .server_version_notice = Some("Older server".into());
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
        @"  ctrl+x then  n new task  esc cancel"
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

#[tokio::test]
async fn transcript_fixed_keys_take_precedence_over_pager_chord_prefixes() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    app.keymap = RuntimeKeymap::from_config(&serde_json::from_value(serde_json::json!({
        "global": {"copy": ["ctrl-x ctrl-u"]},
        "pager": {
            "scroll_up": ["ctrl-space x", "ctrl-home x", "ctrl-end x"],
            "close_transcript": ["ctrl-x ctrl-u"]
        }
    }))?)
    .expect("valid pager chords");
    app.chat_widget.insert_str("draft");
    for owned in [true, false] {
        tui.set_owned_screen(owned)?;
        app.open_transcript_overlay(&mut tui);
        for code in [KeyCode::Home, KeyCode::End, KeyCode::Char(' ')] {
            let key = KeyEvent::new(code, KeyModifiers::CONTROL);
            assert_eq!(app.route_key_chord_event(&mut tui, key), Some(key));
            assert!(!app.key_chord_matcher.is_pending());
        }
        press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
        press(&mut app, &mut tui, &mut app_server, ctrl('u')).await?;
        assert!(app.overlay.is_none());
        assert!(!app.transcript_view.is_detailed());
        assert_eq!(app.chat_widget.composer_text_with_pending(), "draft");
    }
    app.transcript_cells = vec![std::sync::Arc::new(
        crate::history_cell::PlainHistoryCell::new(
            (0..50).map(|_| "transcript row".into()).collect(),
        ),
    )];
    tui.set_owned_screen(/*owned*/ true)?;
    let home = KeyEvent::new(KeyCode::Home, KeyModifiers::CONTROL);
    for (binding, keys) in [
        ("ctrl-home", vec![home]),
        ("ctrl-home ctrl-u", vec![home, ctrl('u')]),
        ("ctrl-x ctrl-home", vec![ctrl('x'), home]),
    ] {
        app.keymap = RuntimeKeymap::from_config(&toml::from_str(&format!(
            "[pager]\nclose_transcript = '{binding}'\n[global]\nopen_external_editor = '{binding}'"
        ))?)
        .expect("valid close binding");
        app.open_transcript_overlay(&mut tui);
        app.render_owned_transcript(
            &mut tui,
            ratatui::layout::Size::new(/*width*/ 80, /*height*/ 12),
        )?;
        app.transcript_view
            .scroll(&app.transcript_cells, /*rows*/ -10);
        assert!(app.transcript_view.can_return_to_latest());
        for key in &keys {
            press(&mut app, &mut tui, &mut app_server, *key).await?;
        }
        assert!(!app.transcript_view.is_detailed());
        assert_eq!(app.chat_widget.composer_text_with_pending(), "draft");
        for key in keys {
            press(&mut app, &mut tui, &mut app_server, key).await?;
        }
        assert_eq!(
            app.chat_widget.external_editor_state(),
            crate::chatwidget::ExternalEditorState::Requested
        );
        app.reset_external_editor_state(&mut tui);
    }
    for voice in ["f9", "f9 v"] {
        app.keymap = RuntimeKeymap::from_config(&toml::from_str(&format!(
            "[chat]\ntoggle_voice = '{voice}'\n[pager]\nclose_transcript = 'f9'"
        ))?)
        .unwrap();
        app.open_transcript_overlay(&mut tui);
        press(&mut app, &mut tui, &mut app_server, KeyCode::F(9).into()).await?;
        assert!(!app.transcript_view.is_detailed());
        assert!(!app.key_chord_matcher.is_pending());
    }
    tui.set_owned_screen(/*owned*/ false)?;
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn selection_after_live_commit_uses_the_refreshed_frame() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    let area = ratatui::layout::Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 12,
    );
    let mut overlay =
        crate::pager_overlay::TranscriptOverlay::new(Vec::new(), app.keymap.pager.clone());
    overlay.sync_live_tail(
        /*width*/ 40,
        /*key*/ None,
        |_| Some(vec!["committed output".into()]),
    );
    overlay.render(area, &mut ratatui::buffer::Buffer::empty(area));
    overlay.insert_cell(std::sync::Arc::new(
        crate::history_cell::PlainHistoryCell::new(vec!["committed output".into()]),
    ));
    app.overlay = Some(crate::pager_overlay::Overlay::Transcript(overlay));
    // The active cell has completed, but no Draw has reached the overlay yet.
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL)),
    )
    .await?;
    let Some(crate::pager_overlay::Overlay::Transcript(overlay)) = &mut app.overlay else {
        panic!("overlay closed")
    };
    assert!(overlay.has_active_interaction());
    let mut buffer = ratatui::buffer::Buffer::empty(area);
    overlay.render(area, &mut buffer);
    let text = buffer
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert_eq!(text.matches("committed output").count(), 1);
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    )
    .await?;
    set_active_cell(
        &mut app.chat_widget,
        Box::new(crate::history_cell::StreamingAgentTailCell::new(
            vec!["live continuation wraps at the resized width".into()],
            /*is_first_line*/ false,
        )),
    );
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Resize(ratatui::layout::Size::new(
            /*width*/ 20, /*height*/ 12,
        )),
    )
    .await?;
    app.close_transcript_overlay(&mut tui);
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn key_capture_receives_voice_shortcut_and_chord_prefix() -> Result<()> {
    let (mut app, mut rx, _op_rx) = super::make_test_app_with_channels().await;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    for config in ["", "[chat]\ntoggle_voice = 'f8 v'"] {
        app.keymap = RuntimeKeymap::from_config(&toml::from_str(config)?).unwrap();
        app.chat_widget.open_keymap_capture(
            "composer".into(),
            "submit".into(),
            crate::app_event::KeymapEditIntent::ReplaceAll,
            crate::app_event::KeymapCaptureMode::SingleKey,
            &app.keymap,
        );
        press(&mut app, &mut tui, &mut app_server, KeyCode::F(8).into()).await?;
        assert!(
            matches!(rx.try_recv()?, crate::app_event::AppEvent::KeymapCaptured { key, .. } if key == "f8")
        );
    }
    Ok(())
}
