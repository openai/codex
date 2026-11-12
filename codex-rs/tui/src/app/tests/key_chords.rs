use super::App;
use super::AppEvent;
use super::AppServerSession;
use super::Result;
use super::RuntimeKeymap;
use super::ThreadId;
use super::TuiEvent;
use super::make_test_app;
use super::make_test_app_with_channels;
use super::start_config_write_test_app_server;
use super::test_thread_session;
use crate::app::owned_transcript::tests::buffer_text;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::chatwidget::ReplayKind;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use crate::chatwidget::tests::helpers::set_active_cell;
use crate::keymap::KeymapContext;
#[cfg(unix)]
use crate::pager_overlay::TranscriptHistoryState;
use crate::tui::Tui;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ToolRequestUserInputOption;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_app_server_protocol::ToolRequestUserInputQuestion;
use codex_app_server_protocol::TurnStatus;
use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
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
async fn empty_thread_backend_chord_preserves_draft_until_first_submission() -> Result<()> {
    let (mut app, mut events, _operations) = make_test_app_with_channels().await;
    let session = test_thread_session(ThreadId::new(), app.config.cwd.to_path_buf());
    app.chat_widget.handle_thread_session(session);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget
        .apply_external_edit("first draft".to_string());
    app.chat_widget
        .set_new_backend_control(Some("Cloud".to_string()));
    let screen = render_bottom_popup(&app.chat_widget, /*width*/ 80);
    insta::assert_snapshot!(screen.lines().find(|line| line.trim_start().starts_with("Cloud")).unwrap().trim(), @"Cloud  ⌃x b to cycle");

    // Cursor movement remains available while the backend selector is visible.
    let key = app.route_key_chord_event(&mut tui, ctrl('b')).unwrap();
    app.chat_widget.handle_key_event(key);
    app.chat_widget.handle_paste("!".to_string());
    assert_eq!(app.chat_widget.composer_text_with_pending(), "first draf!t");

    assert_eq!(app.route_key_chord_event(&mut tui, ctrl('x')), None);
    let key = app
        .route_key_chord_event(&mut tui, KeyCode::Char('b').into())
        .expect("backend chord dispatch");
    app.chat_widget.handle_key_event(key);
    assert!(
        std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::CycleNewConversationBackend))
    );
    assert_eq!(app.chat_widget.composer_text_with_pending(), "first draf!t");

    // A single-key remap takes precedence over cursor movement in the same draft.
    let config = toml::from_str("[global]\nchange_host = 'ctrl-b'")?;
    app.keymap = RuntimeKeymap::from_config(&config).unwrap();
    app.chat_widget.apply_keymap_update(config, &app.keymap);
    app.chat_widget.handle_key_event(ctrl('b'));
    assert!(
        std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::CycleNewConversationBackend))
    );

    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    assert!(
        !render_bottom_popup(&app.chat_widget, /*width*/ 80)
            .lines()
            .any(|line| line.trim_start().starts_with("Cloud"))
    );

    app.chat_widget.handle_key_event(ctrl('b'));
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::CycleNewConversationBackend))
    );
    Ok(())
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

    let config = toml::from_str(
        r#"
        [global]
        open_transcript = "f12 ctrl-t"
        open_external_editor = "f12 e"
        copy = "f12 c"
        open_agents = "f12 a"
        toggle_vim_mode = "f12 v"
        toggle_fast_mode = "f12 f"
        clear_terminal = "f12 k"
    "#,
    )?;
    app.keymap = RuntimeKeymap::from_config(&config).expect("valid custom prefix");
    app.chat_widget.apply_keymap_update(config, &app.keymap);
    tui.set_owned_screen(/*owned*/ true)?;
    tokio::time::pause();
    press(&mut app, &mut tui, &mut app_server, KeyCode::F(12).into()).await?;
    assert!(app.key_chord_matcher.is_pending());
    assert!(app.overlay.is_none());
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
        .await?;
    assert_eq!(tui.shortcut_hud_area, None);
    tokio::time::advance(crate::keymap::CHORD_REVEAL_DELAY).await;
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
        .await?;
    let mut panel =
        ratatui::buffer::Buffer::empty(tui.shortcut_hud_area.expect("visible chord guide"));
    for pos in panel.area.positions() {
        panel[pos] = tui.terminal.previous_buffer()[pos].clone();
    }
    insta::assert_snapshot!("pending_custom_chord", buffer_text(&panel));
    let narrow_size = ratatui::layout::Size::new(/*width*/ 36, /*height*/ 9);
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Resize(narrow_size))
        .await?;
    let narrow = buffer_text(tui.terminal.previous_buffer());
    assert!(
        narrow.contains("… more shortcuts") && narrow.contains("Esc cancel"),
        "{narrow}"
    );
    tui.set_owned_screen(/*owned*/ false)?;
    let inline_size = ratatui::layout::Size::new(/*width*/ 36, /*height*/ 5);
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Resize(inline_size))
        .await?;
    assert_eq!(tui.shortcut_hud_area, Some(tui.terminal.viewport_area));
    insta::assert_snapshot!(
        "pending_custom_chord_inline",
        buffer_text(tui.terminal.previous_buffer())
    );
    tokio::time::advance(std::time::Duration::from_millis(/*millis*/ 650)).await;
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
        .await?;
    assert!(!app.key_chord_matcher.is_pending());
    assert_eq!(tui.shortcut_hud_area, None);

    press(&mut app, &mut tui, &mut app_server, KeyCode::F(12).into()).await?;
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

#[tokio::test]
async fn config_editor_chord_preserves_physical_completion_key_until_release() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let mut config = TuiKeymap::default();
    config.list.accept = Some(KeybindingsSpec::One(KeybindingSpec(
        "enter enter".to_string(),
    )));
    let runtime =
        RuntimeKeymap::from_config(&config).map_err(|error| color_eyre::eyre::eyre!(error))?;
    app.chat_widget.apply_keymap_update(config, &runtime);
    app.keymap = runtime;
    app.reconnect.offline = true;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget.open_config_panel();
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('/').into(),
    )
    .await?;
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Paste("terminal title".to_string()),
    )
    .await?;

    for _ in 0..2 {
        press(&mut app, &mut tui, &mut app_server, KeyCode::Enter.into()).await?;
    }
    let handoff = std::iter::from_fn(|| events.try_recv().ok())
        .find_map(|event| match event {
            crate::app_event::AppEvent::OpenConfigEditor(handoff) => Some(handoff),
            _ => None,
        })
        .expect("config editor handoff");
    assert_eq!(
        handoff.activation_key,
        crate::key_hint::plain(KeyCode::Enter)
    );
    assert!(handoff.active_at().is_some());

    app.chat_widget.open_config_editor(
        handoff,
        crate::config_panel::ResumeCurrentAvailability::Available,
    );
    assert!(
        render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Configure Terminal Title")
    );
    let repeat = KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Repeat);
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(repeat))
        .await?;
    let release =
        KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Release);
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(release))
        .await?;
    for _ in 0..2 {
        press(&mut app, &mut tui, &mut app_server, KeyCode::Enter.into()).await?;
    }
    assert!(
        std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, crate::app_event::AppEvent::TerminalTitleSetup { .. }))
    );
    Ok(())
}

#[tokio::test]
async fn queued_config_handoff_observes_a_consumed_chord_prefix() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let mut config = TuiKeymap::default();
    config.list.move_down = Some(KeybindingsSpec::One(KeybindingSpec("ctrl-x j".to_string())));
    let runtime =
        RuntimeKeymap::from_config(&config).map_err(|error| color_eyre::eyre::eyre!(error))?;
    app.chat_widget.apply_keymap_update(config, &runtime);
    app.keymap = runtime;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget.open_config_panel();
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Paste("terminal title".to_string()),
    )
    .await?;

    press(&mut app, &mut tui, &mut app_server, KeyCode::Enter.into()).await?;
    let handoff = std::iter::from_fn(|| events.try_recv().ok())
        .find_map(|event| match event {
            crate::app_event::AppEvent::OpenConfigEditor(handoff) => Some(handoff),
            _ => None,
        })
        .expect("config editor handoff");
    assert!(handoff.active_at().is_some());

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    assert_eq!(handoff.active_at(), None);
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
            assert!(!app.should_recover_vim_insert_escape(&tui, key));
        }
        assert!(app.should_recover_vim_insert_escape(
            &tui,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT,)
        ));
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
            assert!(!app.should_recover_vim_insert_escape(&tui, event));
        }
        app.chat_widget.insert_str("abc");
        for key in ['b', 'f'] {
            let event = KeyEvent::new(KeyCode::Char(key), KeyModifiers::ALT);
            assert!(app.should_recover_vim_insert_escape(&tui, event));
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
    tokio::time::pause();
    tokio::time::advance(crate::keymap::CHORD_REVEAL_DELAY).await;
    app.render_chat_widget_frame(
        &mut tui,
        ratatui::layout::Size::new(/*width*/ 80, /*height*/ 24),
    )?;
    assert!(buffer_text(tui.terminal.previous_buffer()).contains("New Task"));
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
    app.chat_widget.handle_key_event(KeyCode::Char('/').into());
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
async fn preset_menu_keys_take_precedence_over_transcript_find() -> Result<()> {
    let mut app = make_test_app().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget.handle_thread_session(test_thread_session(
        ThreadId::new(),
        app.config.cwd.to_path_buf(),
    ));
    app.transcript_view.begin_search();

    app.chat_widget.open_quick_dial_menu();
    assert_eq!(
        app.route_key_chord_event(&mut tui, KeyCode::Esc.into()),
        None
    );
    assert!(app.transcript_view.is_search_editing());

    app.chat_widget.open_quick_dial_menu();
    assert_eq!(
        app.route_key_chord_event(&mut tui, KeyCode::Char('n').into()),
        None
    );
    assert!(app.transcript_view.is_search_editing());
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("New preset"));
    Ok(())
}

#[tokio::test]
async fn transcript_fixed_keys_take_precedence_over_pager_chord_prefixes() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    app.keymap = RuntimeKeymap::from_config(&serde_json::from_value(serde_json::json!({
        "global": {
            "copy": ["ctrl-x ctrl-u"],
            "find_transcript": ["ctrl-space f"]
        },
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
        if let Some(crate::pager_overlay::Overlay::Transcript(overlay)) = &mut app.overlay {
            overlay.begin_select_mode();
            let fixed = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL);
            assert_eq!(app.route_key_chord_event(&mut tui, fixed), Some(fixed));
            assert!(!app.key_chord_matcher.is_pending());
            press(
                &mut app,
                &mut tui,
                &mut app_server,
                KeyCode::Char('q').into(),
            )
            .await?;
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
    app.keymap = RuntimeKeymap::from_config(&toml::from_str(
        "[global]\nfind_transcript = 'f6 f'\n[pager]\nscroll_down = 'f6'",
    )?)
    .expect("valid pager motion and global chord");
    app.transcript_cells = vec![std::sync::Arc::new(
        crate::history_cell::PlainHistoryCell::new(vec!["first".into(), "second".into()]),
    )];
    tui.set_owned_screen(/*owned*/ true)?;
    app.render_owned_transcript(
        &mut tui,
        ratatui::layout::Size::new(/*width*/ 80, /*height*/ 12),
    )?;
    assert!(app.transcript_view.begin_select_mode(&app.transcript_cells));
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('g').into(),
    )
    .await?;
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('v').into(),
    )
    .await?;
    let before = app.transcript_view.selected_text(&app.transcript_cells);
    press(&mut app, &mut tui, &mut app_server, KeyCode::F(6).into()).await?;
    assert_ne!(
        app.transcript_view.selected_text(&app.transcript_cells),
        before
    );
    assert!(!app.key_chord_matcher.is_pending());
    super::enable_side_retention(&mut app)?;
    let parent = ThreadId::new();
    app.active_thread_id = Some(parent);
    app.side_threads
        .insert(ThreadId::new(), super::SideThreadState::new(parent));
    app.keymap = RuntimeKeymap::for_session(
        &toml::from_str("[pager]\nscroll_up = 'f6 u'\nscroll_down = 'f6 d'")?,
        /*workspace*/ true,
    )
    .unwrap();
    for (stroke, expected) in [
        ('u', before),
        (
            'd',
            app.transcript_view.selected_text(&app.transcript_cells),
        ),
    ] {
        press(&mut app, &mut tui, &mut app_server, KeyCode::F(6).into()).await?;
        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyCode::Char(stroke).into(),
        )
        .await?;
        assert_eq!(
            app.transcript_view.selected_text(&app.transcript_cells),
            expected
        );
    }
    tui.set_owned_screen(/*owned*/ false)?;
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn selection_after_live_commit_uses_the_refreshed_frame() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    let config: TuiKeymap = toml::from_str(
        "[global]\ntoggle_side_conversation = []\nfind_transcript = 'ctrl-7 j'\n[pager]\nscroll_down = 'ctrl-7 j'",
    )?;
    let runtime =
        RuntimeKeymap::from_config(&config).map_err(|error| color_eyre::eyre::eyre!(error))?;
    app.chat_widget.apply_keymap_update(config, &runtime);
    app.keymap = runtime;
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
    overlay.begin_select_mode();
    app.overlay = Some(crate::pager_overlay::Overlay::Transcript(overlay));
    // The active cell has completed, but no Draw has reached the overlay yet.
    assert!(matches!(
        &app.overlay,
        Some(crate::pager_overlay::Overlay::Transcript(overlay)) if overlay.is_select_mode()
    ));
    press(&mut app, &mut tui, &mut app_server, ctrl('/')).await?;
    assert!(app.key_chord_matcher.is_pending());
    let repeat = KeyEvent::new_with_kind(
        KeyCode::Char('/'),
        KeyModifiers::CONTROL,
        KeyEventKind::Repeat,
    );
    press(&mut app, &mut tui, &mut app_server, repeat).await?;
    assert!(app.key_chord_matcher.is_pending());
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('j').into(),
    )
    .await?;
    let Some(crate::pager_overlay::Overlay::Transcript(overlay)) = &mut app.overlay else {
        panic!("overlay closed")
    };
    assert!(overlay.has_active_interaction() && !overlay.is_search_editing());
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
async fn pane_direction_without_neighbor_preserves_fresh_typing() -> Result<()> {
    let (mut app, mut tui, mut app_server) = chord_app().await?;
    super::enable_side_retention(&mut app)?;
    tui.set_owned_screen(/*owned*/ true)?;

    press(&mut app, &mut tui, &mut app_server, ctrl('x')).await?;
    press(
        &mut app,
        &mut tui,
        &mut app_server,
        KeyCode::Char('h').into(),
    )
    .await?;
    for ch in "hello".chars() {
        press(
            &mut app,
            &mut tui,
            &mut app_server,
            KeyCode::Char(ch).into(),
        )
        .await?;
    }
    // Flush the burst of typed characters before inspecting the draft.
    press(&mut app, &mut tui, &mut app_server, KeyCode::End.into()).await?;

    assert_eq!(app.chat_widget.composer_text_with_pending(), "hello");
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn sidebar_chooser_resize_survives_startup_thread_assignment() -> Result<()> {
    let (mut app, mut tui, mut server) = chord_app().await?;
    super::enable_side_retention(&mut app)?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.local_settings.tui.animations = false;
    let size = ratatui::layout::Size::new(/*width*/ 165, /*height*/ 12);
    tui.terminal = crate::tui::Terminal::with_screen_size_and_cursor_position_for_test(
        ratatui::backend::CrosstermBackend::new(std::io::stdout()),
        size,
        Default::default(),
    );
    assert_eq!(app.sidebar_parent(), None);
    for stroke in [
        ctrl('x'),
        KeyCode::Char('t').into(),
        ctrl('x'),
        KeyCode::Char('r').into(),
    ] {
        Box::pin(press(&mut app, &mut tui, &mut server, stroke)).await?;
    }
    app.activate_thread_channel(ThreadId::new()).await;
    let frame = super::side_frame(&mut app, &mut tui, size)?;
    assert_eq!(app.conversation_frame.panel_area().unwrap().width, 55);
    let (_, footer) = frame.lines().last().unwrap().split_once('│').unwrap();
    insta::assert_snapshot!(footer.trim(), @"←/→ resize · Enter/Esc done");
    assert!(
        app.handle_pane_resize(&tui, &TuiEvent::Key(KeyCode::Enter.into()))
            .await
    );
    assert!(app.pane_resize.is_none());
    let frame = super::side_frame(&mut app, &mut tui, size)?;
    let body = frame
        .lines()
        .skip(1)
        .filter_map(|row| row.split_once('│'))
        .map(|(_, sidebar)| sidebar.trim())
        .filter(|row| !row.is_empty())
        .take(3)
        .collect::<Vec<_>>();
    insta::assert_snapshot!(body.join("\n"), @"
    Open in sidebar
    You can start a side chat after your first message
    in the main chat.
    ");
    Box::pin(press(&mut app, &mut tui, &mut server, ctrl('x'))).await?;
    Box::pin(press(
        &mut app,
        &mut tui,
        &mut server,
        KeyCode::Enter.into(),
    ))
    .await?;
    assert_eq!(
        app.chooser_focus(),
        Some(crate::app::sidebar::PaneFocus::Sidebar)
    );
    assert!(app.sidebar().unwrap().creation.is_none());
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn sidebar_chooser_preserves_the_parent_draft_and_starts_a_side_chat() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.local_settings.tui.keymap = toml::from_str("[list]\naccept = ['f5 g']")?;
    super::enable_side_retention(&mut app)?;
    tui.set_owned_screen(/*owned*/ true)?;
    let parent = ThreadId::new();
    server.remember_thread_history_mode(parent, ThreadHistoryMode::Legacy);
    app.chat_widget
        .handle_thread_session(test_thread_session(parent, app.config.cwd.to_path_buf()));
    app.chat_widget.replay_thread_turns(
        vec![super::test_turn(
            "parent-turn",
            TurnStatus::Completed,
            Vec::new(),
        )],
        ReplayKind::ResumeInitialMessages,
    );
    app.chat_widget.local_settings.tui.status_line = Some(vec!["model-with-reasoning".into()]);
    app.chat_widget.refresh_status_line();
    app.chat_widget.insert_str("parent draft");

    let size = ratatui::layout::Size::new(120, 12);
    let terminal_at = |size| {
        crate::tui::Terminal::with_screen_size_and_cursor_position_for_test(
            ratatui::backend::CrosstermBackend::new(std::io::stdout()),
            size,
            Default::default(),
        )
    };
    tui.terminal = terminal_at(size);
    app.local_settings.tui.animations = true;
    tokio::time::pause();
    let open_chooser = [ctrl('x'), KeyCode::Char('s').into()];
    // The second opening reverses a half-finished close.
    for (key, widths) in [(KeyCode::Esc, [1, 20, 40]), (KeyCode::Enter, [20, 30, 40])] {
        for stroke in open_chooser {
            Box::pin(press(&mut app, &mut tui, &mut server, stroke)).await?;
        }
        for (elapsed, width) in [0, 90, 90].into_iter().zip(widths) {
            tokio::time::advance(std::time::Duration::from_millis(elapsed)).await;
            super::side_frame(&mut app, &mut tui, size)?;
            assert_eq!(app.conversation_frame.panel_area().unwrap().width, width);
            if key == KeyCode::Esc && elapsed == 90 && width == 20 {
                let divider = app.conversation_frame.divider_area().unwrap();
                for kind in [
                    crossterm::event::MouseEventKind::Moved,
                    crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                ] {
                    let event = TuiEvent::Mouse(crossterm::event::MouseEvent {
                        kind,
                        column: divider.x,
                        row: divider.y,
                        modifiers: KeyModifiers::NONE,
                    });
                    Box::pin(app.handle_tui_event(&mut tui, &mut server, event)).await?;
                    assert!(tui.link_hover.shape == crate::tui::PointerShape::Default);
                    assert!(app.divider_drag.is_none());
                    assert_eq!(
                        app.chooser_focus(),
                        Some(crate::app::sidebar::PaneFocus::Sidebar)
                    );
                }
            }
        }
        if key == KeyCode::Esc {
            let frame = super::side_frame(&mut app, &mut tui, size)?;
            insta::assert_snapshot!("sidebar_new_chat", frame);
            app.begin_pane_resize(&mut tui)?;
            let resizing = super::side_frame(&mut app, &mut tui, size)?;
            let footer = resizing.lines().last().unwrap().split_once('│').unwrap().1;
            insta::assert_snapshot!(footer.trim(), @"←/→ resize · Enter/Esc done");
            Box::pin(press(
                &mut app,
                &mut tui,
                &mut server,
                KeyCode::Enter.into(),
            ))
            .await?;
            for stroke in [
                ctrl('x'),
                KeyCode::Char('c').into(),
                KeyEvent::new_with_kind(
                    KeyCode::Char('c'),
                    KeyModifiers::NONE,
                    KeyEventKind::Repeat,
                ),
                KeyCode::Char('c').into(),
                KeyCode::Char('!').into(),
                KeyCode::Left.into(), // Flush buffered typing before the focus change.
            ] {
                Box::pin(press(&mut app, &mut tui, &mut server, stroke)).await?;
            }
            assert!(super::side_frame(&mut app, &mut tui, size)?.contains("Open in sidebar"));
            app.keymap = RuntimeKeymap::for_session(
                &toml::from_str("[global]\nresize_panes = ['8','f12 r']")?,
                /*workspace*/ true,
            )
            .unwrap();
            let narrow = ratatui::layout::Size::new(/*width*/ 80, /*height*/ 12);
            // Navigation can request a fresh backend sample between keystrokes.
            tui.terminal = terminal_at(narrow);
            super::side_frame(&mut app, &mut tui, narrow)?;
            assert!(app.conversation_frame.panel_area().is_none());
            let prefix = KeyEvent::from(KeyCode::F(12));
            assert_eq!(app.route_key_chord_event(&mut tui, prefix), Some(prefix));
            for code in [KeyCode::End, KeyCode::Char('8'), KeyCode::End] {
                Box::pin(press(&mut app, &mut tui, &mut server, code.into())).await?;
            }
            assert_eq!(
                app.chat_widget.composer_text_with_pending(),
                "parent draft!8"
            );
            assert!(!app.key_chord_matcher.is_pending());
            app.chat_widget.apply_external_edit("parent draft!".into());
            super::enable_side_retention(&mut app)?;
            tui.terminal = terminal_at(size);
            super::side_frame(&mut app, &mut tui, size)?;
            for stroke in open_chooser {
                Box::pin(press(&mut app, &mut tui, &mut server, stroke)).await?;
            }
        }
        let strokes = if key == KeyCode::Esc {
            &[KeyCode::Esc][..]
        } else {
            &[KeyCode::F(5), KeyCode::Char('g')]
        };
        for stroke in strokes {
            Box::pin(press(&mut app, &mut tui, &mut server, (*stroke).into())).await?;
        }
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            "parent draft!"
        );
        let dispatched = std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::StartSide { parent_thread_id, .. } if parent_thread_id == parent));
        assert_eq!(dispatched, key == KeyCode::Enter);
        if key == KeyCode::Esc {
            super::side_frame(&mut app, &mut tui, size)?;
            tokio::time::advance(std::time::Duration::from_millis(90)).await;
            super::side_frame(&mut app, &mut tui, size)?;
            assert_eq!(app.conversation_frame.panel_area().unwrap().width, 20);
        }
    }
    let first_prompt = crate::chatwidget::UserMessage {
        remote_image_urls: vec!["https://example.com/image.png".into()],
        .."first prompt".into()
    };
    let sibling = ThreadId::new();
    app.sidebar_mut().chat_ready(sibling);
    app.side_threads
        .insert(sibling, crate::app::side::SideThreadState::new(parent));
    let request = uuid::Uuid::new_v4();
    app.sidebar_mut().creation = Some(crate::app::sidebar::SideCreation {
        owner: server.backend_id(),
        state: crate::app::sidebar::CreationState::Waiting {
            request,
            started: std::time::Instant::now(),
        },
        message: Some(first_prompt),
    });
    app.finish_side_creation(
        &mut tui,
        &mut server,
        parent,
        request,
        Err("The side conversation could not be prepared.".into()),
    )
    .await;
    app.sidebar_mut().show(
        crate::app::sidebar::SidebarContent::Chooser,
        crate::app::sidebar::PaneFocus::Main,
    );
    app.handle_start_side(
        &mut tui,
        &mut server,
        parent,
        Some("second prompt".into()),
        crate::app_event::SideStartMode::New,
    )
    .await?;
    let narrow = super::side_frame(&mut app, &mut tui, ratatui::layout::Size::new(30, 8))?;
    insta::assert_snapshot!("sidebar_failed_creation", narrow);
    super::side_frame(&mut app, &mut tui, size)?;
    app.begin_pane_resize(&mut tui)?;
    assert!(app.pane_resize.is_some());
    let resizing = super::side_frame(&mut app, &mut tui, size)?;
    let hints: Vec<_> = resizing
        .lines()
        .filter_map(|row| row.split_once('│'))
        .map(|(_, sidebar)| sidebar.trim())
        .filter(|row| !row.is_empty())
        .collect();
    insta::assert_snapshot!(hints.join("\n"), @"The side conversation could not be
prepared.
←/→ resize · Enter/Esc done");
    Box::pin(press(
        &mut app,
        &mut tui,
        &mut server,
        KeyCode::Enter.into(),
    ))
    .await?;
    assert!(app.sidebar().unwrap().creation.is_some());
    Box::pin(press(&mut app, &mut tui, &mut server, KeyCode::Esc.into())).await?;
    assert!(
        super::side_frame(&mut app, &mut tui, ratatui::layout::Size::new(30, 8))?
            .contains("Open in sidebar")
    );
    let draft = app.chat_widget.composer_draft_snapshot();
    assert!(draft.remote_image_urls.is_empty());
    insta::assert_snapshot!(draft.text, @"second prompt
parent draft!");
    // A page fits a chat with a subtitle or two title-only chats.
    let pane = app.sidebar_mut();
    for _ in 0..4 {
        pane.chat_ready(ThreadId::new());
    }
    pane.show(
        crate::app::sidebar::SidebarContent::Chooser,
        crate::app::sidebar::PaneFocus::Sidebar,
    );
    pane.choice = 2;
    let id = pane.chat_tabs().choice(/*index*/ 2).unwrap();
    pane.latest_prompts.insert(id, "Latest request".into());
    let two_rows = ratatui::layout::Size::new(/*width*/ 120, /*height*/ 12);
    tui.terminal = terminal_at(two_rows);
    super::side_frame(&mut app, &mut tui, two_rows)?;
    for (key, expected) in [
        (KeyCode::PageDown, 3),
        (KeyCode::PageDown, 5),
        (KeyCode::PageUp, 3),
        (KeyCode::PageUp, 2),
    ] {
        Box::pin(press(&mut app, &mut tui, &mut server, key.into())).await?;
        assert_eq!(app.sidebar().unwrap().choice, expected);
    }
    Box::pin(press(&mut app, &mut tui, &mut server, ctrl('c'))).await?;
    assert_eq!(app.chooser_focus(), None);
    assert_eq!(app.chat_widget.composer_text_with_pending(), draft.text);
    tokio::time::resume();
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn key_capture_receives_voice_shortcut_and_chord_prefix() -> Result<()> {
    let (mut app, mut rx, _op_rx) = super::make_test_app_with_channels().await;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    super::enable_side_retention(&mut app)?;
    tui.set_owned_screen(/*owned*/ true)?;
    let parent = ThreadId::new();
    app.active_thread_id = Some(parent);
    let child = ThreadId::new();
    app.side_threads
        .insert(child, super::SideThreadState::new(parent));
    for (config, key) in [
        ("", 8),
        ("[chat]\ntoggle_voice = 'f8 v'", 8),
        ("[global]\nhide_pane = 'f12'", 12),
        ("[global]\nfocus_main_composer = 'f12'", 12),
        ("[global]\nfocus_sidebar_content = 'f12 s'", 12),
        ("[global]\nfocus_pane_left = 'f12 h'", 12),
        ("[global]\nfocus_pane_right = 'f12 l'", 12),
    ] {
        app.keymap =
            RuntimeKeymap::for_session(&toml::from_str(config)?, /*workspace*/ true).unwrap();
        app.chat_widget.open_keymap_capture(
            "composer".into(),
            "submit".into(),
            crate::app_event::KeymapEditIntent::ReplaceAll,
            crate::app_event::KeymapCaptureMode::SingleKey,
            &app.keymap,
        );
        press(&mut app, &mut tui, &mut app_server, KeyCode::F(key).into()).await?;
        assert!(
            matches!(rx.try_recv()?, crate::app_event::AppEvent::KeymapCaptured { key: captured, .. } if captured == format!("f{key}"))
        );
    }
    // A printable remap belongs to the composer unless this session and thread own a pane.
    app.keymap = RuntimeKeymap::for_session(
        &toml::from_str("[global]\nhide_pane = '%'")?,
        /*workspace*/ true,
    )
    .unwrap();
    for (thread, enabled, owned, offline, draft) in [
        (parent, true, true, false, ""),
        (ThreadId::new(), true, true, false, "%"),
        (parent, false, true, false, "%"),
        (parent, true, false, false, "%"),
        (parent, true, true, true, "%"),
    ] {
        app.active_thread_id = Some(thread);
        app.config
            .features
            .set_enabled(codex_features::Feature::TuiWorkspace, enabled)?;
        tui.set_owned_screen(owned)?;
        app.reconnect.offline = offline;
        app.chat_widget.apply_external_edit(String::new());
        for key in [KeyCode::Char('%'), KeyCode::Right] {
            press(&mut app, &mut tui, &mut app_server, key.into()).await?;
        }
        assert_eq!(app.chat_widget.composer_text_with_pending(), draft);
    }

    // Offline, the same printable binding focuses the local composer but cannot switch threads.
    let mut stopped = app_server.request_view();
    app_server.shutdown().await?;
    app.local_settings.tui.keymap = toml::from_str("[global]\nfocus_main_composer = '%'")?;
    super::enable_side_retention(&mut app)?;
    app.active_thread_id = Some(ThreadId::new());
    app.reconnect.offline = true;
    app.chat_widget.apply_external_edit("draft".into());
    app.transcript_view.begin_search();
    for key in [KeyCode::Char('%'), KeyCode::Char('!'), KeyCode::Right] {
        press(&mut app, &mut tui, &mut stopped, key.into()).await?;
    }
    assert!(!app.transcript_view.has_active_interaction());
    assert_eq!(app.chat_widget.composer_text_with_pending(), "draft!");
    app.active_thread_id = Some(child);
    for key in [KeyCode::Char('%'), KeyCode::Right] {
        press(&mut app, &mut tui, &mut stopped, key.into()).await?;
    }
    assert_eq!(app.chat_widget.composer_text_with_pending(), "draft!%");
    // Bare letters and arrows still edit the draft.
    for key in [
        KeyCode::Char('h'),
        KeyCode::Left,
        KeyCode::Char('l'),
        KeyCode::Right,
        KeyCode::Char('!'),
        KeyCode::Right,
    ] {
        press(&mut app, &mut tui, &mut stopped, key.into()).await?;
    }
    assert_eq!(app.chat_widget.composer_text_with_pending(), "draft!%lh!");
    Ok(())
}
