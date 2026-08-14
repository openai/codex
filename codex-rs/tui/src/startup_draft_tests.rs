use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tokio::sync::mpsc::unbounded_channel;

use super::StartupDraftInitialScreen;
use super::StartupDraftPump;
use super::handle_startup_draft_key;
use super::startup_draft_bottom_pane;
use super::startup_draft_renderable;
use super::startup_session_header;
use crate::app_event_sender::AppEventSender;
use crate::legacy_core::config::ConfigBuilder;
use crate::render::renderable::Renderable;
use crate::tui::FrameRequester;
use crate::tui::TuiEvent;

fn startup_test_pump<I>(events: I) -> StartupDraftPump
where
    I: Iterator<Item = TuiEvent> + Send + 'static,
{
    let (tx, rx) = unbounded_channel();
    StartupDraftPump {
        header: startup_session_header(/*config*/ None),
        bottom_pane: startup_draft_bottom_pane(
            AppEventSender::new(tx),
            FrameRequester::test_dummy(),
            /*enhanced_keys_supported*/ false,
        ),
        events: Box::pin(futures::stream::iter(events)),
        app_event_rx: rx,
        initial_screen: StartupDraftInitialScreen::Composer,
        pending_paste_newline: None,
    }
}

#[test]
fn startup_draft_renders_full_empty_and_multiline_composer_frames() {
    let mut pump = startup_test_pump(std::iter::empty());
    let mut snapshots = Vec::new();

    for (label, width, text) in [
        ("empty", 48, ""),
        ("multiline", 48, "first startup line\nsecond startup line"),
        ("narrow", 18, "first startup line\nsecond startup line"),
    ] {
        pump.bottom_pane
            .set_composer_text(text.to_string(), Vec::new(), Vec::new());
        let renderable = startup_draft_renderable(&pump.header, &pump.bottom_pane);
        let area = Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            width,
            renderable.desired_height(width),
        );
        let mut buffer = Buffer::empty(area);
        renderable.render(area, &mut buffer);
        let cursor = renderable
            .cursor_pos(area)
            .expect("keep the editable composer cursor visible below its header");
        let frame = (0..area.height)
            .map(|row| {
                (0..area.width)
                    .map(|column| buffer[(column, row)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
            .replace(crate::version::CODEX_CLI_VERSION, "<VERSION>");

        assert!(
            cursor.1 >= pump.header.desired_height(width),
            "the composer cursor should remain below the startup header"
        );
        snapshots.push(format!("{label} ({width} columns):\n{frame}"));
    }

    insta::assert_snapshot!("startup_draft_full_frames", snapshots.join("\n---\n"));
}

#[tokio::test]
async fn startup_draft_hydrates_its_header_without_moving_the_composer() {
    let codex_home = tempfile::tempdir().expect("create temporary Codex home");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("build startup configuration");
    let mut pump = startup_test_pump(std::iter::empty());
    let width = 80;
    let initial_height =
        startup_draft_renderable(&pump.header, &pump.bottom_pane).desired_height(width);

    assert_eq!(
        pump.header.raw_lines().last().map(ToString::to_string),
        Some("directory: loading".to_string())
    );
    pump.apply_config(&config);
    let expected_directory = format!(
        "directory: {}",
        crate::history_cell::SessionHeaderHistoryCell::format_directory_inner(
            config.cwd.as_path(),
            /*max_width*/ None,
        )
    );
    assert_eq!(
        pump.header.raw_lines().last().map(ToString::to_string),
        Some(expected_directory)
    );
    assert_eq!(
        startup_draft_renderable(&pump.header, &pump.bottom_pane).desired_height(width),
        initial_height
    );
}

#[test]
fn startup_draft_preserves_literal_text_without_popups_or_startup_actions() {
    let mut pump = startup_test_pump(std::iter::empty());
    for character in "/quit @src $skill /tmp/image.png".chars() {
        handle_startup_draft_key(
            &mut pump.bottom_pane,
            KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
        )
        .expect("preserve literal startup text");
    }
    pump.bottom_pane.flush_composer_paste_burst();
    let expected = "/quit @src $skill /tmp/image.png";
    assert_eq!(pump.bottom_pane.composer_text(), expected);

    for key in [
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Repeat),
        KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT),
        KeyEvent::new_with_kind(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ),
    ] {
        handle_startup_draft_key(&mut pump.bottom_pane, key).expect("ignore startup action");
    }

    assert_eq!(pump.bottom_pane.composer_text(), expected);
    assert!(pump.bottom_pane.composer_local_images().is_empty());
    assert!(pump.bottom_pane.no_modal_or_popup_active());
    assert!(pump.app_event_rx.try_recv().is_err());
}

#[test]
fn startup_draft_preserves_control_cancellation_and_forward_deletion() {
    let mut pump = startup_test_pump(std::iter::empty());
    for character in "abc".chars() {
        handle_startup_draft_key(
            &mut pump.bottom_pane,
            KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
        )
        .expect("buffer startup text before Ctrl+D");
    }
    assert!(pump.bottom_pane.is_in_paste_burst());
    handle_startup_draft_key(
        &mut pump.bottom_pane,
        KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
    )
    .expect("move inside the startup draft");
    handle_startup_draft_key(
        &mut pump.bottom_pane,
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
    )
    .expect("delete the character under the cursor");
    assert_eq!(pump.bottom_pane.composer_text(), "ab");

    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    handle_startup_draft_key(&mut pump.bottom_pane, ctrl_c)
        .expect("clear nonempty startup draft before cancelling");
    assert_eq!(pump.bottom_pane.composer_text(), "");

    handle_startup_draft_key(
        &mut pump.bottom_pane,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
    )
    .expect("hold a startup character before cancellation");
    handle_startup_draft_key(&mut pump.bottom_pane, ctrl_c)
        .expect("clear the held character instead of cancelling startup");

    for key in [
        ctrl_c,
        KeyEvent::new(KeyCode::Char('\u{3}'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('\u{4}'), KeyModifiers::NONE),
    ] {
        let error = handle_startup_draft_key(&mut pump.bottom_pane, key)
            .expect_err("cancel the empty draft using normalized terminal shortcuts");
        assert!(super::StartupCancelled::matches(&error));
    }
}

#[test]
fn startup_draft_preserves_multiline_editing_without_submitting() {
    let mut pump = startup_test_pump(std::iter::empty());
    pump.bottom_pane.insert_str("first");

    for key in [
        KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
        KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('\n'), KeyModifiers::NONE),
    ] {
        handle_startup_draft_key(&mut pump.bottom_pane, key).expect("insert a startup newline");
    }
    pump.bottom_pane.flush_composer_paste_burst();
    handle_startup_draft_key(
        &mut pump.bottom_pane,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    )
    .expect("ignore startup submission");
    pump.bottom_pane.insert_str("second");

    assert_eq!(pump.bottom_pane.composer_text(), "first\n\n\n\nsecond");
    assert!(pump.app_event_rx.try_recv().is_err());
}

#[tokio::test]
async fn startup_draft_preserves_non_bracketed_multiline_pastes_without_submitting() {
    let events = "first line\n\nsecond line\nthird line"
        .chars()
        .map(|character| {
            TuiEvent::Key(KeyEvent::new(
                if character == '\n' {
                    KeyCode::Enter
                } else {
                    KeyCode::Char(character)
                },
                KeyModifiers::NONE,
            ))
        });
    let mut pump = startup_test_pump(events);
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test terminal");

    pump.flush_pending_events(&mut tui)
        .await
        .expect("preserve multiline non-bracketed paste");

    assert_eq!(
        pump.bottom_pane.composer_text(),
        "first line\n\nsecond line\nthird line"
    );
    assert!(pump.app_event_rx.try_recv().is_err());

    pump.handle_event(
        &mut tui,
        TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    )
    .expect("ignore standalone Enter after the paste finishes");
    assert_eq!(
        pump.bottom_pane.composer_text(),
        "first line\n\nsecond line\nthird line"
    );
    assert!(pump.app_event_rx.try_recv().is_err());
}

#[tokio::test]
async fn startup_draft_preserves_deferred_paste_newlines_across_handoff() {
    let (event_tx, event_rx) = unbounded_channel();
    let mut pump = startup_test_pump(std::iter::empty());
    pump.events = Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(
        event_rx,
    ));
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test terminal");

    for character in "first line".chars() {
        pump.handle_event(
            &mut tui,
            TuiEvent::Key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE)),
        )
        .expect("buffer the first non-bracketed paste line");
    }
    pump.handle_event(
        &mut tui,
        TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    )
    .expect("defer an ambiguous paste newline");
    assert!(pump.pending_paste_newline.is_some());

    let continuation = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(/*millis*/ 10)).await;
        event_tx
            .send(TuiEvent::Key(KeyEvent::new(
                KeyCode::Char('s'),
                KeyModifiers::NONE,
            )))
            .expect("continue the paste during handoff");
    });
    pump.flush_pending_paste_newline(&mut tui)
        .await
        .expect("finish the deferred paste before startup handoff");
    continuation.await.expect("join the paste continuation");

    assert_eq!(pump.into_draft().text, "first line\ns");
}

#[tokio::test]
async fn startup_draft_does_not_turn_a_standalone_enter_into_a_newline_at_handoff() {
    let (_event_tx, event_rx) = unbounded_channel();
    let mut pump = startup_test_pump(std::iter::empty());
    pump.events = Box::pin(tokio_stream::wrappers::UnboundedReceiverStream::new(
        event_rx,
    ));
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test terminal");

    for character in "first line".chars() {
        pump.handle_event(
            &mut tui,
            TuiEvent::Key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE)),
        )
        .expect("buffer startup typing");
    }
    pump.handle_event(
        &mut tui,
        TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    )
    .expect("keep standalone startup Enter provisional");
    assert!(pump.pending_paste_newline.is_some());

    pump.flush_pending_paste_newline(&mut tui)
        .await
        .expect("discard standalone Enter after its lookahead expires");

    assert_eq!(pump.into_draft().text, "first line");
}

#[test]
fn startup_draft_allows_local_editor_shortcuts_without_startup_actions() {
    let mut pump = startup_test_pump(std::iter::empty());
    pump.bottom_pane.insert_str("first second");

    for (key, expected_cursor) in [
        (
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT),
            "first ".len(),
        ),
        (KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL), 0),
        (
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
            "first second".len(),
        ),
    ] {
        handle_startup_draft_key(&mut pump.bottom_pane, key).expect("apply editor movement");
        assert_eq!(pump.bottom_pane.composer_cursor(), expected_cursor);
    }
    handle_startup_draft_key(
        &mut pump.bottom_pane,
        KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
    )
    .expect("delete the previous word");
    assert_eq!(pump.bottom_pane.composer_text(), "first ");

    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.editor.move_line_start = vec![crate::key_hint::ctrl(KeyCode::Char('z'))];
    keymap.editor.move_line_end = vec![crate::key_hint::ctrl(KeyCode::Char('v'))];
    keymap.editor.move_left = vec![crate::key_hint::ctrl(KeyCode::Char('s'))];
    keymap.editor.insert_newline = vec![crate::key_hint::plain(KeyCode::Enter)];
    keymap.composer.submit = vec![crate::key_hint::ctrl(KeyCode::Char('s'))];
    pump.bottom_pane.set_keymap_bindings(&keymap);
    handle_startup_draft_key(
        &mut pump.bottom_pane,
        KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
    )
    .expect("use a configured editor movement");
    assert_eq!(pump.bottom_pane.composer_cursor(), 0);

    for key in [
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('\u{16}'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('\u{12}'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('\u{13}'), KeyModifiers::NONE),
    ] {
        handle_startup_draft_key(&mut pump.bottom_pane, key)
            .expect("ignore image, history, and configured submission actions");
    }

    keymap.composer.submit = vec![crate::key_hint::plain(KeyCode::Char('s'))];
    keymap.composer.queue = vec![crate::key_hint::plain(KeyCode::Char('q'))];
    keymap.composer.history_search_previous = vec![crate::key_hint::plain(KeyCode::Char('r'))];
    pump.bottom_pane.set_keymap_bindings(&keymap);
    for character in ['s', 'q', 'r'] {
        handle_startup_draft_key(
            &mut pump.bottom_pane,
            KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
        )
        .expect("ignore configured plain composer actions");
    }
    assert_eq!(pump.bottom_pane.composer_cursor(), 0);
    assert_eq!(pump.bottom_pane.composer_text(), "first ");
    assert!(pump.app_event_rx.try_recv().is_err());
}

#[cfg(target_os = "windows")]
#[test]
fn startup_draft_preserves_windows_altgr_text_input() {
    let mut pump = startup_test_pump(std::iter::empty());
    handle_startup_draft_key(
        &mut pump.bottom_pane,
        KeyEvent::new(
            KeyCode::Char('@'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        ),
    )
    .expect("preserve printable AltGr input");
    pump.bottom_pane.flush_composer_paste_burst();
    assert_eq!(pump.bottom_pane.composer_text(), "@");
}

#[tokio::test]
async fn startup_draft_applies_paste_burst_preferences_without_losing_buffered_input() {
    let codex_home = tempfile::tempdir().expect("create temporary Codex home");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("build startup configuration");
    let mut pump = startup_test_pump(std::iter::empty());

    handle_startup_draft_key(
        &mut pump.bottom_pane,
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
    )
    .expect("buffer input before startup configuration loads");
    assert!(pump.bottom_pane.is_in_paste_burst());
    assert_eq!(pump.bottom_pane.composer_text(), "");

    config.disable_paste_burst = true;
    pump.apply_config(&config);
    assert_eq!(pump.bottom_pane.composer_text(), "a");
    assert!(!pump.bottom_pane.is_in_paste_burst());

    handle_startup_draft_key(
        &mut pump.bottom_pane,
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
    )
    .expect("insert text immediately when paste detection is disabled");
    assert_eq!(pump.bottom_pane.composer_text(), "ab");
    assert!(!pump.bottom_pane.is_in_paste_burst());

    config.disable_paste_burst = false;
    pump.apply_config(&config);
    handle_startup_draft_key(
        &mut pump.bottom_pane,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
    )
    .expect("restore paste detection after a later configuration change");
    assert_eq!(pump.bottom_pane.composer_text(), "ab");
    assert!(pump.bottom_pane.is_in_paste_burst());

    config.disable_paste_burst = true;
    pump.apply_config(&config);
    assert_eq!(pump.bottom_pane.composer_text(), "abc");
    assert!(!pump.bottom_pane.is_in_paste_burst());
    assert!(pump.app_event_rx.try_recv().is_err());
}

#[tokio::test]
async fn startup_draft_applies_editor_keymap_without_enabling_vim() {
    let codex_home = tempfile::tempdir().expect("create temporary Codex home");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("build startup configuration");
    config.tui_vim_mode_default = true;
    config.tui_keymap.editor.move_line_start = Some(codex_config::types::KeybindingsSpec::One(
        codex_config::types::KeybindingSpec("ctrl-z".to_string()),
    ));
    let mut pump = startup_test_pump(std::iter::empty());
    pump.bottom_pane.insert_str("draft");
    pump.apply_config(&config);
    assert!(!pump.bottom_pane.composer_is_vim_enabled());
    handle_startup_draft_key(
        &mut pump.bottom_pane,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
    )
    .expect("insert ordinary text despite the configured Vim mode");
    pump.bottom_pane.flush_composer_paste_burst();
    assert_eq!(pump.bottom_pane.composer_text(), "draftx");
    handle_startup_draft_key(
        &mut pump.bottom_pane,
        KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
    )
    .expect("honor a configured safe editor shortcut");
    assert_eq!(pump.bottom_pane.composer_cursor(), 0);
}

#[tokio::test]
async fn startup_draft_waits_for_onboarding_before_accepting_input() {
    let mut composer_pump = startup_test_pump(std::iter::empty());
    let mut composer_tui =
        crate::tui::test_support::make_test_tui().expect("create composer test terminal");
    composer_pump
        .show_initial_screen(&mut composer_tui)
        .expect("draw the composer when no protected screen is expected");
    assert!(!composer_tui.terminal.viewport_area.is_empty());
    drop(composer_tui);

    let mut pump = startup_test_pump(
        [
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            TuiEvent::Paste("not a draft".to_string()),
            TuiEvent::Draw,
        ]
        .into_iter(),
    );
    let codex_home = tempfile::tempdir().expect("create an existing custom Codex home");
    std::fs::write(codex_home.path().join("history.jsonl"), "")
        .expect("create existing startup history");
    let system_config_path = codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(
        codex_home.path().join("system.toml"),
    )
    .expect("resolve missing system configuration");
    let search_override = [("web_search".to_string(), toml::Value::String("live".into()))];
    pump.initial_screen =
        if crate::startup_preflight::has_only_search_config_override(&search_override)
            && crate::startup_preflight::should_delay_startup_composer_for_first_login(
                codex_home.path(),
                Ok(system_config_path),
                || Ok(false),
                |name| (name == "CODEX_HOME").then(|| codex_home.path().as_os_str().to_os_string()),
            )
        {
            StartupDraftInitialScreen::Onboarding
        } else {
            StartupDraftInitialScreen::Composer
        };
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test terminal");
    pump.show_initial_screen(&mut tui)
        .expect("keep the composer hidden until onboarding finishes");

    pump.flush_pending_events(&mut tui)
        .await
        .expect("ignore input before onboarding owns the screen");
    assert!(pump.bottom_pane.composer_is_empty());
    assert!(tui.terminal.viewport_area.is_empty());
    let hidden_area = tui.terminal.viewport_area;
    let mut frames = format!(
        "before onboarding: hidden ({}x{} viewport)",
        hidden_area.width, hidden_area.height
    );

    pump.show(&mut tui)
        .expect("show the composer after onboarding finishes");
    assert!(!tui.terminal.viewport_area.is_empty());
    let area = tui.terminal.viewport_area;
    let renderable = startup_draft_renderable(&pump.header, &pump.bottom_pane);
    let mut buffer = Buffer::empty(area);
    renderable.render(area, &mut buffer);
    let visible_frame = (area.top()..area.bottom())
        .map(|row| {
            (area.left()..area.right())
                .map(|column| buffer[(column, row)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .replace(crate::version::CODEX_CLI_VERSION, "<VERSION>");
    drop(renderable);
    frames.push_str(&format!("\n---\nafter onboarding:\n{visible_frame}"));
    insta::assert_snapshot!("startup_draft_onboarding_transition", frames);

    pump.handle_event(
        &mut tui,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
    )
    .expect("edit the composer after onboarding finishes");
    pump.bottom_pane.flush_composer_paste_burst();
    assert_eq!(pump.bottom_pane.composer_text(), "y");
}

#[tokio::test]
async fn startup_draft_allows_cancellation_before_onboarding_appears() {
    for character in ['c', 'd'] {
        let mut pump = startup_test_pump(std::iter::once(TuiEvent::Key(KeyEvent::new(
            KeyCode::Char(character),
            KeyModifiers::CONTROL,
        ))));
        pump.initial_screen = StartupDraftInitialScreen::Onboarding;
        let mut tui = crate::tui::test_support::make_test_tui().expect("create test terminal");

        let error = pump
            .flush_pending_events(&mut tui)
            .await
            .expect_err("cancel startup before onboarding appears");
        assert!(super::StartupCancelled::matches(&error));
    }
}

#[tokio::test]
async fn startup_draft_waits_for_session_picker_before_accepting_input() {
    let mut pump = startup_test_pump(
        [
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            TuiEvent::Paste("not a draft".to_string()),
            TuiEvent::Draw,
        ]
        .into_iter(),
    );
    pump.initial_screen = StartupDraftInitialScreen::SessionPicker;
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test terminal");

    pump.flush_pending_events(&mut tui)
        .await
        .expect("ignore input before the session picker owns the screen");
    assert!(pump.bottom_pane.composer_is_empty());
    assert!(tui.terminal.viewport_area.is_empty());

    pump.show(&mut tui)
        .expect("show the composer after a session is selected");
    assert!(!tui.terminal.viewport_area.is_empty());
    pump.handle_event(
        &mut tui,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE)),
    )
    .expect("edit the composer after the session picker exits");
    pump.bottom_pane.flush_composer_paste_burst();
    assert_eq!(pump.bottom_pane.composer_text(), "y");

    let ctrl_c = TuiEvent::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    pump.handle_event(&mut tui, ctrl_c.clone())
        .expect("clear the visible startup draft before cancelling");
    assert!(pump.bottom_pane.composer_is_empty());
    let error = pump
        .handle_event(&mut tui, ctrl_c)
        .expect_err("cancel the visible startup composer");
    assert!(super::StartupCancelled::matches(&error));
}

#[tokio::test]
async fn startup_draft_allows_cancellation_before_session_picker_appears() {
    let mut pump = startup_test_pump(std::iter::once(TuiEvent::Key(KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
    ))));
    pump.initial_screen = StartupDraftInitialScreen::SessionPicker;
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test terminal");

    let error = pump
        .flush_pending_events(&mut tui)
        .await
        .expect_err("cancel startup before the session picker appears");
    assert!(super::StartupCancelled::matches(&error));
}

#[tokio::test]
async fn startup_draft_flushes_large_event_backlogs_without_submitting_or_retaining_app_events() {
    let events = (0..=1024).map(|_| TuiEvent::Draw).chain([
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE)),
        TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        TuiEvent::Paste("\u{1b}[31m last\r\nline ".to_string()),
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('@'), KeyModifiers::NONE)),
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE)),
        TuiEvent::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
    ]);
    let mut pump = startup_test_pump(events);
    pump.bottom_pane.insert_str("trusted");
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test terminal");
    pump.flush_pending_events(&mut tui)
        .await
        .expect("flush safe startup input");

    assert!(pump.app_event_rx.try_recv().is_err());
    let draft = pump.into_draft();
    assert_eq!(draft.text, "trusted! last\nline @s");
    assert_eq!(draft.cursor, draft.text.len() - 1);
    assert!(draft.local_images.is_empty());
}

#[tokio::test]
async fn startup_draft_preserves_large_pastes_without_attaching_images() {
    let pasted = "x".repeat(/*n*/ 1_100);
    let mut pump = startup_test_pump(std::iter::once(TuiEvent::Paste(pasted.clone())));
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test terminal");
    pump.flush_pending_events(&mut tui)
        .await
        .expect("preserve large startup paste");

    let draft = pump.into_draft();
    assert_eq!(draft.pending_pastes, vec![(draft.text.clone(), pasted)]);
    assert_eq!(draft.cursor, draft.text.len());
    assert!(draft.local_images.is_empty());
}
