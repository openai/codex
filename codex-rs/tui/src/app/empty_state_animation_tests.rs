//! Empty-state decoration integration uses the actual owned-screen draw path.
//! Startup content opts in; other history dismisses the logo independently of viewport space.

use super::*;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;

fn has_blossom(tui: &tui::Tui) -> bool {
    text(crate::custom_terminal::test_support::last_rendered_buffer(
        &tui.terminal,
    ))
    .chars()
    .any(|ch| ('\u{2801}'..='\u{28ff}').contains(&ch))
}

fn text(buffer: &Buffer) -> String {
    let project = crate::test_support::test_path_display("/tmp/project");
    let normalized_project = format!("{:<width$}", "/tmp/project", width = project.len());
    buffer
        .content
        .chunks(usize::from(buffer.area.width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .replace(&project, &normalized_project)
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn draw(app: &mut App, tui: &mut tui::Tui, size: Size) -> Result<Rect> {
    tui.screen_size_for_event(&TuiEvent::Resize(size))?;
    app.render_owned_transcript(tui, size)
}

#[tokio::test]
async fn fresh_logo_returns_only_to_the_ordinary_composer() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    app.local_settings.tui.animations = true;
    let size = Size::new(/*width*/ 120, /*height*/ 44);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.queue_clear_ui_header(&mut tui);
    app.chat_widget
        .empty_state_animation
        .borrow_mut()
        .start_fresh();
    draw(&mut app, &mut tui, size)?;
    let cursor = tui.terminal.last_known_cursor_pos;
    assert!(has_blossom(&tui));

    app.open_transcript_overlay(&mut tui);
    draw(&mut app, &mut tui, size)?;
    assert!(!has_blossom(&tui));
    assert_eq!(tui.terminal.last_known_cursor_pos, cursor);
    app.close_transcript_overlay(&mut tui);
    draw(&mut app, &mut tui, size)?;
    assert!(has_blossom(&tui));

    app.transcript_view.begin_search();
    draw(&mut app, &mut tui, size)?;
    assert!(!has_blossom(&tui));
    app.transcript_view.handle_key(
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        &app.transcript_cells,
    );
    draw(&mut app, &mut tui, size)?;
    assert!(has_blossom(&tui));

    for enabled in [true, false] {
        app.apply_raw_output_mode(&mut tui, enabled, /*notify*/ false);
        draw(&mut app, &mut tui, size)?;
        assert_eq!(has_blossom(&tui), !enabled);
    }
    app.chat_widget.apply_external_edit(String::new());
    for (key, visible) in [(KeyCode::Char('?'), false), (KeyCode::Right, true)] {
        app.chat_widget
            .handle_key_event(KeyEvent::new(key, KeyModifiers::NONE));
        draw(&mut app, &mut tui, size)?;
        assert_eq!(has_blossom(&tui), visible);
    }
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn non_startup_history_dismisses_logo_until_a_new_thread() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    app.local_settings.tui.animations = true;
    // Leave room for the full release-note card above the centered logo.
    let size = Size::new(/*width*/ 120, /*height*/ 64);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.chat_widget
        .empty_state_animation
        .borrow_mut()
        .start_fresh();
    app.queue_clear_ui_header(&mut tui);
    app.insert_history_cell(
        &mut tui,
        Box::new(history_cell::StartupWarningsCell::new(vec![
            "Startup notice".into(),
        ])),
    );
    app.insert_history_cell(
        &mut tui,
        Box::new(history_cell::new_deprecation_notice(
            "Deprecated setting".into(),
            /*details*/ None,
        )),
    );
    app.insert_history_cell(
        &mut tui,
        Box::new(history_cell::new_server_version_warning(
            crate::status::remote_connection::ServerVersionNotice {
                message: "Server version differs from the client".into(),
                offer_update: false,
            },
        )),
    );
    app.insert_history_cell(
        &mut tui,
        Box::new(history_cell::UpdateAvailableHistoryCell::new(
            "99.0.0".into(),
            /*update_action*/ None,
        )),
    );
    app.insert_history_cell(
        &mut tui,
        Box::new(history_cell::SessionNoticeCell(
            history_cell::PlainHistoryCell::new(vec![
                "Previous session usage and resume hint".into(),
            ]),
        )),
    );
    draw(&mut app, &mut tui, size)?;
    assert!(has_blossom(&tui));

    crate::chatwidget::tests::set_chatgpt_auth(&mut app.chat_widget);
    let request = app.chat_widget.start_rate_limit_reset_startup_check();
    assert!(app.chat_widget.finish_rate_limit_reset_hint_refresh(
        request,
        Vec::new(),
        Ok(codex_app_server_protocol::RateLimitResetCreditsSummary {
            available_count: 1,
            credits: None,
        }),
    ));
    // The same startup notice stays eligible both pending and committed to history.
    assert!(app.chat_widget.empty_state_composer().is_some());
    draw(&mut app, &mut tui, size)?;
    assert!(has_blossom(&tui));
    app.insert_pending_usage_output_if_ready(&mut tui);
    draw(&mut app, &mut tui, size)?;
    assert!(has_blossom(&tui));

    // Even invisible non-startup content ends eligibility before a frame or clear can erase it.
    app.insert_history_cell(
        &mut tui,
        Box::new(history_cell::PlainHistoryCell::new(Vec::new())),
    );
    app.reset_transcript_state_after_clear();
    app.queue_clear_ui_header(&mut tui);
    draw(&mut app, &mut tui, size)?;
    assert!(!has_blossom(&tui));
    app.chat_widget
        .empty_state_animation
        .borrow_mut()
        .start_fresh();
    draw(&mut app, &mut tui, size)?;
    assert!(has_blossom(&tui));
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn empty_state_animation_preserves_header_cursor_and_footer() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    app.local_settings.tui.animations = true;
    let size = Size::new(/*width*/ 120, /*height*/ 44);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.chat_widget
        .empty_state_animation
        .borrow()
        .greeting
        .set(crate::empty_state_animation::Greeting {
            phrase: "Pull up a prompt.",
        })
        .expect("initial greeting");
    app.queue_clear_ui_header(&mut tui);
    app.transcript_cells
        .push(Arc::new(history_cell::StartupWarningsCell::mcp(
            vec!["Example MCP is unavailable".to_string()],
            ["example".to_string()],
            /*failure_reason*/ None,
        )));
    let before_bottom = draw(&mut app, &mut tui, size)?;
    let history_len = app.transcript_cells.len();
    let before = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal).clone();
    let cursor = tui.terminal.last_known_cursor_pos;
    assert!(!has_blossom(&tui));
    app.chat_widget
        .empty_state_animation
        .borrow_mut()
        .start_fresh();
    assert_eq!(draw(&mut app, &mut tui, size)?, before_bottom);
    let after = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
    assert!(has_blossom(&tui));
    assert_eq!(tui.terminal.last_known_cursor_pos, cursor);
    assert!(text(&before).contains("Pull up a prompt."));
    assert_eq!(
        &after.content[after.index_of(/*x*/ 0, before_bottom.y)..],
        &before.content[before.index_of(/*x*/ 0, before_bottom.y)..]
    );
    insta::assert_snapshot!(
        "fresh_thread_header",
        format!(
            "enabled:\n{}\n---\ndisabled:\n{}",
            text(after),
            text(&before)
        )
    );
    // Editing, clearing, and resizing must never choose another phrase.
    let phrase_pos = after
        .content
        .iter()
        .position(|cell| cell.symbol() == "P")
        .expect("the phrase is visible above the composer");
    assert_eq!(after.content[phrase_pos].fg, crate::style::accent_color());

    let short = Size::new(/*width*/ 120, /*height*/ 12);
    draw(&mut app, &mut tui, short)?;
    assert!(!has_blossom(&tui));
    draw(&mut app, &mut tui, size)?;
    assert!(has_blossom(&tui));

    app.chat_widget.apply_external_edit("/m".to_string());
    draw(&mut app, &mut tui, size)?;
    assert!(!has_blossom(&tui));
    app.chat_widget.apply_external_edit(String::new());
    draw(&mut app, &mut tui, size)?;
    assert!(has_blossom(&tui));
    assert!(
        text(crate::custom_terminal::test_support::last_rendered_buffer(
            &tui.terminal
        ))
        .contains("Pull up a prompt.")
    );
    assert_eq!(app.transcript_cells.len(), history_len);
    app.local_settings.tui.animations = false;
    draw(&mut app, &mut tui, size)?;
    let disabled = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
    assert_eq!(disabled, &before);
    app.local_settings.tui.animations = true;
    app.local_settings.tui.effects.welcome = false;
    draw(&mut app, &mut tui, size)?;
    assert_eq!(
        crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal),
        &before
    );
    app.local_settings.tui.effects.welcome = true;
    app.local_settings.tui.effects.shimmer = false;
    draw(&mut app, &mut tui, size)?;
    assert!(has_blossom(&tui));
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn submitting_a_draft_keeps_greeting_and_dismisses_logo_even_after_clear() -> Result<()> {
    let (mut app, _events, _ops) = crate::app::tests::make_test_app_with_channels().await;
    app.local_settings.tui.animations = true;
    let size = Size::new(/*width*/ 120, /*height*/ 44);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.queue_clear_ui_header(&mut tui);
    app.chat_widget
        .empty_state_animation
        .borrow_mut()
        .start_fresh();
    draw(&mut app, &mut tui, size)?;
    assert!(has_blossom(&tui));
    let greeting = *app
        .chat_widget
        .empty_state_animation
        .borrow()
        .greeting
        .get()
        .expect("choose a greeting for the fresh thread");
    app.chat_widget
        .apply_external_edit("first prompt".to_string());
    draw(&mut app, &mut tui, size)?;
    let drafting = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
    assert!(!has_blossom(&tui));
    assert!(text(drafting).contains("first prompt"));
    assert!(text(drafting).contains(greeting.phrase));
    app.chat_widget.apply_external_edit(String::new());
    draw(&mut app, &mut tui, size)?;
    assert!(has_blossom(&tui));
    app.chat_widget
        .apply_external_edit("first prompt".to_string());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    draw(&mut app, &mut tui, size)?;
    assert!(!has_blossom(&tui));
    let submitted = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
    assert_eq!(text(submitted).matches(greeting.phrase).count(), 1);
    // The header is also stable when it scrolls out and back into the viewport.
    let short = Size::new(/*width*/ 120, /*height*/ 8);
    draw(&mut app, &mut tui, short)?;
    draw(&mut app, &mut tui, size)?;
    let restored = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
    assert!(text(restored).contains(greeting.phrase));
    assert!(!has_blossom(&tui));
    app.reset_transcript_state_after_clear();
    draw(&mut app, &mut tui, size)?;
    assert!(!has_blossom(&tui));
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn empty_state_animation_survives_plain_transcript_clicks() -> Result<()> {
    use crossterm::event::MouseButton;
    use crossterm::event::MouseEvent;
    use crossterm::event::MouseEventKind;

    let mut app = crate::app::test_support::make_test_app().await;
    app.local_settings.tui.animations = true;
    let size = Size::new(/*width*/ 120, /*height*/ 44);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.queue_clear_ui_header(&mut tui);
    app.chat_widget
        .empty_state_animation
        .borrow_mut()
        .start_fresh();
    draw(&mut app, &mut tui, size)?;
    let cursor = tui.terminal.last_known_cursor_pos;
    for (kind, column, visible) in [
        (MouseEventKind::Down(MouseButton::Left), 4, true),
        (MouseEventKind::Up(MouseButton::Left), 4, true),
        (MouseEventKind::Down(MouseButton::Left), 7, true),
        (MouseEventKind::Drag(MouseButton::Left), 18, false),
        (MouseEventKind::Up(MouseButton::Left), 18, false),
    ] {
        app.transcript_view.handle_mouse(
            MouseEvent {
                kind,
                column,
                row: 1,
                modifiers: KeyModifiers::NONE,
            },
            &app.transcript_cells,
        );
        draw(&mut app, &mut tui, size)?;
        assert_eq!(has_blossom(&tui), visible);
        if visible {
            assert_eq!(tui.terminal.last_known_cursor_pos, cursor);
        }
    }
    assert!(
        app.transcript_view
            .selected_text(&app.transcript_cells)
            .is_some()
    );
    app.transcript_view.end_selection(&app.transcript_cells);
    draw(&mut app, &mut tui, size)?;
    assert!(!has_blossom(&tui));
    app.transcript_view.jump_to_latest();
    draw(&mut app, &mut tui, size)?;
    assert!(has_blossom(&tui));
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}
