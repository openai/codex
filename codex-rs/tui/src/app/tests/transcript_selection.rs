//! Selection keys remain inside the overlay while drafts and backtrack previews are pending.

use super::*;
use crate::chatwidget::UserMessage;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn text_selection_temporarily_replaces_the_rendered_backtrack_highlight() -> Result<()> {
    let (mut app, mut events, mut operations) = make_test_app_with_channels().await;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget.apply_external_edit("retained draft".into());
    app.chat_widget.handle_key_event(KeyCode::Left.into());
    let draft = app.chat_widget.capture_thread_input_state();
    let prompt = "Selected prompt";
    app.transcript_cells = vec![Arc::new(UserHistoryCell {
        message: prompt.into(),
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: Vec::new(),
        spoken: false,
    })];
    app.open_transcript_overlay(&mut tui);
    while events.try_recv().is_ok() {}
    while operations.try_recv().is_ok() {}
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyCode::Esc.into()),
    )
    .await?;
    assert!(app.backtrack.overlay_preview_active);
    let preview = (app.backtrack.base_id, app.backtrack.nth_user_message);
    let render = |app: &mut App| {
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 32, /*height*/ 12,
        );
        let mut buffer = Buffer::empty(area);
        // Match the App owner's footer projection before this fixed-size overlay render.
        let footer = app.prompt_navigation_footer(area.width);
        let Some(Overlay::Transcript(overlay)) = app.overlay.as_mut() else {
            panic!("expected transcript overlay");
        };
        overlay.browsing_footer = footer.and_then(|footer| footer.text.lines.into_iter().next());
        overlay.render(area, &mut buffer);
        let highlighted = buffer
            .content
            .iter()
            .filter(|cell| cell.modifier.contains(ratatui::style::Modifier::REVERSED))
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>();
        (buffer, highlighted)
    };
    let (backtrack, highlighted) = render(&mut app);
    assert_eq!(highlighted, prompt);

    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL)),
    )
    .await?;
    let (_, highlighted) = render(&mut app);
    assert_eq!(highlighted, "");
    // The display-only blank before a user message may be the selection's first source row.
    for _ in 0..2 {
        app.handle_tui_event(
            &mut tui,
            &mut app_server,
            TuiEvent::Key(KeyCode::Right.into()),
        )
        .await?;
    }
    let (_, highlighted) = render(&mut app);
    assert!(!highlighted.is_empty());
    assert!(prompt.starts_with(&highlighted));
    assert!(highlighted.len() < prompt.len());

    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyCode::Esc.into()),
    )
    .await?;
    let (restored, highlighted) = render(&mut app);
    assert_eq!(highlighted, prompt);
    assert_eq!(restored, backtrack);
    assert!(app.backtrack.overlay_preview_active);
    assert_eq!(
        (app.backtrack.base_id, app.backtrack.nth_user_message),
        preview
    );
    assert_eq!(app.chat_widget.capture_thread_input_state(), draft);
    assert!(operations.try_recv().is_err());
    assert!(events.try_recv().is_err());
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn offline_selection_reserves_keys_and_preserves_draft_and_backtrack_state() -> Result<()> {
    let (mut app, mut events, mut operations) = make_test_app_with_channels().await;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.keymap = RuntimeKeymap::from_config(&toml::from_str(
        "[pager]\nclose = 'ctrl-c x'\npage_up = 'ctrl-x p'\n",
    )?)
    .unwrap();
    app.chat_widget
        .restore_user_message_to_composer(UserMessage {
            text: "draft prompt".to_owned(),
            local_images: Vec::new(),
            remote_image_urls: vec!["https://example.com/draft.png".to_owned()],
            text_elements: Vec::new(),
            mention_bindings: Vec::new(),
        });
    app.chat_widget.handle_key_event(KeyCode::Left.into());
    let draft = app.chat_widget.capture_thread_input_state();
    app.transcript_cells = vec![plain_line_cell("selected text")];
    app.open_transcript_overlay(&mut tui);
    let Some(Overlay::Transcript(overlay)) = app.overlay.as_mut() else {
        panic!("transcript overlay");
    };
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 12,
    );
    overlay.render(area, &mut Buffer::empty(area));
    app.backtrack.overlay_preview_active = true;
    app.backtrack.nth_user_message = 3;
    while events.try_recv().is_ok() {}
    while operations.try_recv().is_ok() {}

    assert_eq!(
        app.route_key_chord_event(
            &mut tui,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        ),
        None,
    );
    assert!(app.key_chord_matcher.is_pending());
    app.reconnect.offline = true;
    for key in [
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        KeyCode::Enter.into(),
        KeyCode::Right.into(),
        KeyCode::Left.into(),
        KeyCode::Esc.into(),
    ] {
        assert!(matches!(
            app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(key))
                .await?,
            AppRunControl::Continue,
        ));
        assert!(app.overlay.is_some());
        assert!(!app.key_chord_matcher.is_pending());
        assert_eq!(app.chat_widget.capture_thread_input_state(), draft);
        assert_eq!(app.backtrack.nth_user_message, 3);
        assert!(app.backtrack.overlay_preview_active);
        assert!(operations.try_recv().is_err());
        assert!(events.try_recv().is_err());
    }
    let Some(Overlay::Transcript(overlay)) = app.overlay.as_ref() else {
        panic!("transcript overlay");
    };
    assert!(!overlay.has_active_interaction());
    app.chat_widget.insert_str("!");
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "draft promp!t"
    );
    assert!(matches!(
        app.handle_tui_event(
            &mut tui,
            &mut app_server,
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        )
        .await?,
        AppRunControl::Exit(ExitReason::UserRequested),
    ));
    app_server.shutdown().await?;
    Ok(())
}
