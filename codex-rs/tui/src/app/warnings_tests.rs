//! Exercise the warnings panel through the real app, including composer and scroll restoration.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn warnings_hide_and_restore_draft_and_freeze_until_reopened() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let (widget, sender, mut events, _) =
        crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
    app.chat_widget = widget;
    app.app_event_tx = sender;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.chat_widget.apply_external_edit("/m".into());
    app.transcript_cells
        .push(Arc::new(history_cell::PlainHistoryCell::new(
            (0..50)
                .map(|row| format!("Earlier transcript row {row}").into())
                .collect(),
        )));
    app.insert_history_cell(
        &mut tui,
        Box::new(history_cell::new_warning_event(
            "Example warning\nFull diagnostic with remediation.".into(),
        )),
    );
    let size = Size::new(/*width*/ 80, /*height*/ 24);
    app.render_owned_transcript(&mut tui, size)?;
    app.transcript_view
        .scroll(&app.transcript_cells, /*rows*/ -20);
    app.render_owned_transcript(&mut tui, size)?;
    let before = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal).clone();
    let cursor = tui.terminal.last_known_cursor_pos;
    app.chat_widget.open_warnings(&app.transcript_cells);
    app.render_owned_transcript(&mut tui, size)?;
    let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
    let screen = buffer
        .content
        .chunks(usize::from(size.width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(
        "warnings_page",
        crate::chatwidget::tests::helpers::normalize_snapshot_paths(screen),
    );
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    app.reconnect.offline = true;
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyCode::Char('E').into()),
    )
    .await?;
    assert!(app.chat_widget.keymap_contexts().is_warnings());
    assert_eq!(app.chat_widget.composer_text_with_pending(), "/m");
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Paste("xample".into()))
        .await?;
    assert!(app.chat_widget.keymap_contexts().is_warnings());
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyCode::Char('k').into()),
    )
    .await?;
    assert!(!app.chat_widget.keymap_contexts().is_warnings());
    let repeat =
        KeyEvent::new_with_kind(KeyCode::Char('k'), KeyModifiers::NONE, KeyEventKind::Repeat);
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(repeat))
        .await?;
    assert_eq!(app.chat_widget.composer_text_with_pending(), "/m");
    app.render_owned_transcript(&mut tui, size)?;
    assert_eq!(
        crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal),
        &before
    );
    assert_eq!(tui.terminal.last_known_cursor_pos, cursor);

    app.chat_widget.open_warnings(&app.transcript_cells);
    app.insert_history_cell(
        &mut tui,
        Box::new(history_cell::new_warning_event("Later warning".into())),
    );
    app.render_owned_transcript(&mut tui, size)?;
    let frozen = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal)
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(frozen.contains("1 of 1"));
    // The first keep's queued update may arrive while the frozen viewer is reopened.
    let first_keep = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| matches!(event, AppEvent::UpdateWarnings { kept, .. } if !kept.is_empty()))
        .expect("the first keep queued a decision");
    app.handle_event(&mut tui, &mut app_server, first_keep)
        .await?;
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    assert_eq!(app.chat_widget.composer_text_with_pending(), "/m");
    let dismissal = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| matches!(event, AppEvent::UpdateWarnings { dismissed, .. } if !dismissed.is_empty()))
        .expect("dismiss the viewed warning offline");
    app.handle_event(&mut tui, &mut app_server, dismissal)
        .await?;
    // The slash popup masks the passive footer; check the count with a plain draft.
    app.chat_widget.apply_external_edit("draft".into());
    app.render_owned_transcript(&mut tui, size)?;
    let screen = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal)
        .content
        .chunks(usize::from(size.width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(
        "warnings_partial_dismissal",
        crate::chatwidget::tests::helpers::normalize_snapshot_paths(screen),
    );
    app.chat_widget.apply_external_edit("/m".into());
    app.chat_widget.open_warnings(&app.transcript_cells);
    app.render_owned_transcript(&mut tui, size)?;
    let reopened = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal)
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(reopened.contains("1 of 1"));
    assert!(reopened.contains("Later warning"));
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
    )
    .await?;
    let dismissal = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| matches!(event, AppEvent::UpdateWarnings { dismissed, .. } if !dismissed.is_empty()))
        .expect("Ctrl+C dismisses the viewed warning");
    app.handle_event(&mut tui, &mut app_server, dismissal)
        .await?;
    app.render_owned_transcript(&mut tui, size)?;
    let after = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
    assert!(!after.content.iter().any(|cell| cell.symbol() == "⚠"));
    assert_eq!(tui.terminal.last_known_cursor_pos, cursor);

    // An empty composer must not steal Escape for transcript backtracking.
    app.chat_widget.apply_external_edit(String::new());
    app.chat_widget.open_warnings(&app.transcript_cells);
    let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.should_handle_backtrack_esc(escape));
    app.chat_widget.handle_key_event(escape);
    assert!(app.chat_widget.no_modal_or_popup_active());
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyCode::F(2).into()),
    )
    .await?;
    assert!(app.chat_widget.keymap_contexts().is_warnings());
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    app.chat_widget.apply_external_edit("/warnings".into());
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyCode::Enter.into()),
    )
    .await?;
    let event = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| matches!(event, AppEvent::OpenWarnings))
        .expect("offline /warnings dispatch");
    app.handle_event(&mut tui, &mut app_server, event).await?;
    assert!(app.chat_widget.keymap_contexts().is_warnings());
    app.reconnect.offline = false;
    app_server.shutdown().await?;
    app.show_shutdown_feedback(&mut tui)?;
    assert!(!app.chat_widget.keymap_contexts().is_warnings());
    let screen = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal)
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(screen.contains("Shutting down"));
    Ok(())
}

#[tokio::test]
async fn warnings_badge_and_pages_work_with_terminal_scrollback() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ false)?;
    app.chat_widget.apply_external_edit("fallback draft".into());
    app.insert_history_cell(
        &mut tui,
        Box::new(history_cell::new_warning_event(
            "Fallback diagnostic".into(),
        )),
    );
    let size = Size::new(/*width*/ 80, /*height*/ 24);
    app.render_chat_widget_frame(&mut tui, size)?;
    let before = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal).clone();
    assert!(before.content.iter().any(|cell| cell.symbol() == "⚠"));
    app.chat_widget.open_warnings(&app.transcript_cells);
    app.render_chat_widget_frame(&mut tui, size)?;
    let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
    let screen = buffer
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    for text in ["Warnings", "Fallback diagnostic"] {
        assert!(screen.contains(text));
    }
    assert!(!screen.contains("fallback draft"));
    app.chat_widget.handle_key_event(KeyCode::Char('k').into());
    app.render_chat_widget_frame(&mut tui, size)?;
    assert_eq!(
        crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal),
        &before
    );
    Ok(())
}

#[tokio::test]
async fn warnings_changed_details_reappear_and_clear_resets_dismissal() -> Result<()> {
    use crate::chatwidget::tests::helpers::render_bottom_popup;
    use codex_app_server_protocol::McpServerStartupFailureReason;

    let (mut app, mut events, _ops) = crate::app::tests::make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let original = history_cell::StartupWarningsCell::mcp(
        vec!["example MCP unavailable".into()],
        vec!["example".into()],
        /*failure_reason*/ None,
    );
    app.insert_history_cell(&mut tui, Box::new(original.clone()));
    app.chat_widget.open_warnings(&app.transcript_cells);
    let before = render_bottom_popup(&app.chat_widget, /*width*/ 80);
    // The ID stays the same. Sign-in instructions arrive while the old page is open.
    app.insert_history_cell(
        &mut tui,
        Box::new(history_cell::StartupWarningsCell::mcp(
            vec!["Login again".into()],
            vec!["example".into()],
            Some(McpServerStartupFailureReason::ReauthenticationRequired),
        )),
    );
    assert_eq!(render_bottom_popup(&app.chat_widget, /*width*/ 80), before);
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    let dismissal = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| matches!(event, AppEvent::UpdateWarnings { dismissed, .. } if !dismissed.is_empty()))
        .expect("old details dismissed");
    app.handle_event(&mut tui, &mut app_server, dismissal)
        .await?;
    app.chat_widget.open_warnings(&app.transcript_cells);
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Sign-in required"));
    app.chat_widget.handle_key_event(KeyCode::F(2).into());
    let dismissal = std::iter::from_fn(|| events.try_recv().ok())
        .find(|event| matches!(event, AppEvent::UpdateWarnings { dismissed, .. } if !dismissed.is_empty()))
        .expect("updated details dismissed");
    app.handle_event(&mut tui, &mut app_server, dismissal)
        .await?;
    // Duplicate unchanged delivery must not restore the badge or page.
    app.insert_history_cell(&mut tui, Box::new(original.clone()));
    app.chat_widget.open_warnings(&app.transcript_cells);
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("No warnings"));
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    app.reset_app_ui_state_after_clear();
    app.insert_history_cell(&mut tui, Box::new(original.clone()));
    app.chat_widget.open_warnings(&app.transcript_cells);
    assert_eq!(render_bottom_popup(&app.chat_widget, /*width*/ 80), before);
    // Terminal input can win the race with queued app events in embedded mode.
    // Reopen before applying the prior dismissal; keep must be the final decision.
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    app.chat_widget.open_warnings(&app.transcript_cells);
    app.chat_widget.handle_key_event(KeyCode::Char('k').into());
    while let Ok(event) = events.try_recv() {
        if matches!(event, AppEvent::UpdateWarnings { .. }) {
            app.handle_event(&mut tui, &mut app_server, event).await?;
        }
    }
    app.chat_widget.open_warnings(&app.transcript_cells);
    assert_eq!(render_bottom_popup(&app.chat_widget, /*width*/ 80), before);
    // Clear before the queued dismissal arrives, then deliver the same warning again.
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    app.reset_app_ui_state_after_clear();
    app.insert_history_cell(&mut tui, Box::new(original));
    while let Ok(event) = events.try_recv() {
        if matches!(event, AppEvent::UpdateWarnings { .. }) {
            app.handle_event(&mut tui, &mut app_server, event).await?;
        }
    }
    app.chat_widget.open_warnings(&app.transcript_cells);
    assert_eq!(render_bottom_popup(&app.chat_widget, /*width*/ 80), before);
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn warnings_keep_remapped_copy_keys_out_of_composer_handlers() {
    let (mut widget, _, mut rx, _) =
        crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
    let diagnostic = "Example warning\nFull diagnostic";
    let cells: Vec<Arc<dyn HistoryCell>> =
        vec![Arc::new(history_cell::new_warning_event(diagnostic.into()))];
    for key in ['r', 'u'] {
        let config: codex_config::types::TuiKeymap = serde_json::from_value(serde_json::json!({
            "global": {"copy": format!("ctrl-{key}")},
            "composer": {"history_search_previous": []},
            "editor": {"kill_line_start": []}
        }))
        .expect("copy binding");
        let runtime = crate::keymap::RuntimeKeymap::from_config(&config).expect("valid remap");
        widget.apply_keymap_update(config, &runtime);
        widget.open_warnings(&cells);
        widget.handle_key_event(KeyEvent::new(KeyCode::Char(key), KeyModifiers::CONTROL));
        let mut copied = Vec::new();
        while let Ok(event) = rx.try_recv() {
            if let AppEvent::CopyWarning(text) = event {
                copied.push(text);
            }
        }
        assert_eq!(copied, vec![diagnostic.to_string()]);
    }
    let mut app = crate::app::test_support::make_test_app().await;
    let mut tui = crate::tui::test_support::make_test_tui().expect("test terminal");
    let config = serde_json::from_value(serde_json::json!({
        "global": {"copy": "ctrl-x c"},
        "list": {"accept": "ctrl-x"}
    }))
    .expect("copy chord");
    app.keymap = crate::keymap::RuntimeKeymap::from_config(&config).expect("valid chord");
    widget.apply_keymap_update(config, &app.keymap);
    widget.open_warnings(&cells);
    app.chat_widget = widget;
    assert!(
        app.route_key_chord_event(
            &mut tui,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)
        )
        .is_none()
    );
    let key = app
        .route_key_chord_event(&mut tui, KeyCode::Char('c').into())
        .expect("copy action");
    app.chat_widget.handle_key_event(key);
    assert!(
        std::iter::from_fn(|| rx.try_recv().ok())
            .any(|event| matches!(event, AppEvent::CopyWarning(text) if text == diagnostic))
    );
}
