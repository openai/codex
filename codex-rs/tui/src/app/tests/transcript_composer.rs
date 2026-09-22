//! Regression coverage for transcript viewer input, prompt selection, and restoration.
//!
//! The default-off feature must leave the existing viewer and its draft intact.
//! Analytics also preserves the composer and stays separate from transcript backtracking.

use super::*;
use crate::pager_overlay::TranscriptHistoryState;
use crate::test_support::test_path_display;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
    buffer
        .content()
        .chunks(usize::from(buffer.area.width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn press_key(
    app: &mut App,
    tui: &mut crate::tui::Tui,
    app_server: &mut AppServerSession,
    code: KeyCode,
) -> Result<()> {
    app.handle_tui_event(
        tui,
        app_server,
        TuiEvent::Key(KeyEvent::new(code, KeyModifiers::NONE)),
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn analytics_preserves_the_draft_and_retains_the_view_on_reopen() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget
        .apply_external_edit("preserved draft".into());
    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::OpenAnalytics { view: None },
    )
    .await?;
    for code in [
        KeyCode::Char('4'),
        KeyCode::Enter,
        KeyCode::Down,
        KeyCode::Char('r'),
    ] {
        press_key(&mut app, &mut tui, &mut app_server, code).await?;
        assert!(matches!(app.overlay, Some(Overlay::Analytics(_))));
        assert!(!app.backtrack.overlay_preview_active);
    }
    press_key(&mut app, &mut tui, &mut app_server, KeyCode::Char('q')).await?;
    assert!(app.overlay.is_none());
    assert!(app.retained_analytics.is_some());
    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::OpenAnalytics { view: None },
    )
    .await?;
    assert!(matches!(app.overlay, Some(Overlay::Analytics(_))));
    assert!(app.retained_analytics.is_none());
    press_key(&mut app, &mut tui, &mut app_server, KeyCode::Char('q')).await?;
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "preserved draft"
    );
    Ok(())
}

#[tokio::test]
async fn analytics_menu_reopen_preserves_navigation_and_explicit_view_selects_summary() -> Result<()>
{
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let http = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/backend-api/wham/accounts/check"))
        .and(wiremock::matchers::header(
            "chatgpt-account-id",
            "test-account",
        ))
        .respond_with(wiremock::ResponseTemplate::new(/*s*/ 200).set_body_json(
            serde_json::json!({"accounts": [{"id": "test-account", "plan_type": "plus"}]}),
        ))
        .expect(/*r*/ 3)
        .mount(&http)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(
            wiremock::ResponseTemplate::new(/*s*/ 200)
                .set_body_json(serde_json::json!({"stats":{"lifetime_tokens":12345},"data":[]})),
        )
        .mount(&http)
        .await;
    app.config.chatgpt_base_url = format!("{}/backend-api", http.uri());
    app.config.cli_auth_credentials_store_mode = codex_login::AuthCredentialsStoreMode::File;
    app_test_support::write_chatgpt_auth(
        &app.config.codex_home,
        app_test_support::ChatGptAuthFixture::new("test-access-token")
            .account_id("test-account")
            .chatgpt_account_id("test-account")
            .chatgpt_user_id("test-user")
            .plan_type("plus"),
        codex_login::AuthCredentialsStoreMode::File,
    )
    .unwrap();
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut screens = Vec::new();
    for explicit in [
        None,
        None,
        Some(crate::analytics::TokenActivityView::Weekly),
    ] {
        app.handle_event(
            &mut tui,
            &mut app_server,
            AppEvent::OpenAnalytics { view: explicit },
        )
        .await?;
        tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 10), async {
            loop {
                app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
                    .await?;
                let text = buffer_text(crate::custom_terminal::test_support::last_rendered_buffer(
                    &tui.terminal,
                ));
                if screens.is_empty() && text.contains("Lifetime tokens") {
                    press_key(&mut app, &mut tui, &mut app_server, KeyCode::Char('4')).await?;
                    continue;
                }
                if text.contains("Lifetime tokens")
                    || text.contains("No activity")
                    || text.contains("No data reported")
                {
                    return Ok::<(), color_eyre::Report>(());
                }
                tokio::time::sleep(std::time::Duration::from_millis(/*millis*/ 5)).await;
            }
        })
        .await??;
        let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
        if explicit.is_none() {
            let plugins = buffer
                .content()
                .windows("Plugins".len())
                .find(|cells| {
                    cells
                        .iter()
                        .map(ratatui::buffer::Cell::symbol)
                        .collect::<String>()
                        == "Plugins"
                })
                .expect("Plugins tab is visible");
            let active = crate::bottom_pane::active_tab_style();
            assert_eq!(
                (plugins[0].fg, plugins[0].bg),
                (active.fg.unwrap(), active.bg.unwrap()),
                "the initial and reopened report must select Plugins"
            );
        }
        screens.push(buffer_text(buffer));
        press_key(&mut app, &mut tui, &mut app_server, KeyCode::Char('q')).await?;
    }
    assert_eq!(screens[0], screens[1]);
    assert!(screens[2].contains("Lifetime tokens"));
    assert!(screens[2].contains("Weekly"));
    Ok(())
}

#[tokio::test]
async fn analytics_keeps_queued_and_late_startup_history_out_of_the_overlay() -> Result<()> {
    for alternate_screen in [true, false] {
        let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
        let mut app_server = start_config_write_test_app_server(&app).await?;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        tui.set_alt_screen_enabled(alternate_screen);
        app.insert_history_cell(
            &mut tui,
            Box::new(PlainHistoryCell::new(vec!["Startup header".into()])),
        );
        assert!(!tui.pending_history_lines_for_test().is_empty());
        app.handle_event(
            &mut tui,
            &mut app_server,
            AppEvent::OpenAnalytics { view: None },
        )
        .await?;
        assert!(tui.pending_history_lines_for_test().is_empty());
        app.insert_history_cell(
            &mut tui,
            Box::new(PlainHistoryCell::new(vec![
                "Late startup announcement".into(),
            ])),
        );
        assert!(tui.pending_history_lines_for_test().is_empty());
        let deferred = app.deferred_history_lines.clone();
        assert!(!deferred.is_empty());
        press_key(&mut app, &mut tui, &mut app_server, KeyCode::Char('q')).await?;
        assert_eq!(tui.pending_history_lines_for_test(), deferred);
        assert_eq!(app.transcript_cells.len(), 2);
    }
    Ok(())
}

#[tokio::test]
async fn transcript_flag_off_preserves_viewer_and_backtracking() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let keymap_config = toml::from_str("[composer]\nsubmit = [\"ctrl-x enter\"]")?;
    app.keymap =
        crate::keymap::RuntimeKeymap::from_config(&keymap_config).expect("valid composer chord");
    app.chat_widget
        .apply_keymap_update(keymap_config, &app.keymap);
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let session = test_thread_session(ThreadId::new(), app.config.cwd.to_path_buf());
    app.chat_widget.handle_thread_session(session);
    app.transcript_cells = ["first", "second"]
        .map(|message| {
            Arc::new(UserHistoryCell {
                message: message.into(),
                text_elements: Vec::new(),
                local_image_paths: Vec::new(),
                remote_image_urls: Vec::new(),
                spoken: false,
            }) as Arc<dyn HistoryCell>
        })
        .to_vec();
    app.chat_widget
        .apply_external_edit("preserved draft".into());
    app.open_transcript_overlay(&mut tui);
    for event in [
        TuiEvent::Paste("not composer input".into()),
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)),
    ] {
        app.handle_tui_event(&mut tui, &mut app_server, event)
            .await?;
    }
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "preserved draft"
    );
    let chord_prefix = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(chord_prefix))
        .await?;
    assert!(!app.key_chord_matcher.is_pending());
    assert!(!app.backtrack.overlay_preview_active);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 100, /*height*/ 12,
    );
    let mut buffer = ratatui::buffer::Buffer::empty(area);
    let Some(Overlay::Transcript(overlay)) = &mut app.overlay else {
        panic!("viewer closed")
    };
    overlay.render(area, &mut buffer);
    insta::assert_snapshot!("transcript_flag_off_viewer", buffer_text(&buffer));
    for (key, selected) in [
        (KeyCode::Esc, 1),
        (KeyCode::Left, 0),
        (KeyCode::Right, 1),
        (KeyCode::Right, 1),
    ] {
        if let Some(Overlay::Transcript(overlay)) = app.overlay.as_mut() {
            overlay.set_history_state(TranscriptHistoryState::LoadingBeginning);
            overlay.render(area, &mut buffer);
            assert_eq!(
                overlay.set_history_state(TranscriptHistoryState::LoadingBeginning),
                TranscriptHistoryState::LoadingBeginning,
            );
        }
        press_key(&mut app, &mut tui, &mut app_server, key).await?;
        let Some(Overlay::Transcript(overlay)) = app.overlay.as_mut() else {
            panic!("viewer closed")
        };
        assert_eq!(
            overlay.set_history_state(TranscriptHistoryState::Complete),
            TranscriptHistoryState::LoadingOlder,
        );
        assert_eq!(app.backtrack.nth_user_message, selected);
    }
    press_key(&mut app, &mut tui, &mut app_server, KeyCode::Enter).await?;
    assert!(app.overlay.is_none());
    assert!(
        std::iter::from_fn(|| app_event_rx.try_recv().ok()).any(|event| matches!(
            event,
            AppEvent::RevertSessionForPromptEdit {
                selected_cell,
                ..
            } if Arc::ptr_eq(&selected_cell, &app.transcript_cells[crate::app_backtrack::nth_user_position(&app.transcript_cells, /*nth*/ 1).unwrap()])
        ))
    );
    Ok(())
}

async fn assert_transcript_close_repaints_inline_draft(mut app: Box<App>) -> Result<()> {
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget.insert_str("EDGE-DRAFT-MUST-SURVIVE");
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
        .await?;
    let inline_viewport = tui.terminal.viewport_area;
    app.open_transcript_overlay(&mut tui);
    tui.enter_alt_screen()?;
    app.insert_history_cell(
        &mut tui,
        Box::new(PlainHistoryCell::new(vec!["arrived in transcript".into()])),
    );
    let deferred_history = app.deferred_history_lines.clone();
    assert!(!deferred_history.is_empty());
    app.apply_raw_output_mode(&mut tui, /*enabled*/ true, /*notify*/ false);
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
        .await?;
    assert_eq!(app.deferred_history_lines, deferred_history);
    assert!(app.transcript_reflow.has_pending_reflow());

    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)),
    )
    .await?;
    assert_eq!(tui.terminal.viewport_area, inline_viewport);
    app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
        .await?;
    assert!(!app.transcript_reflow.has_pending_reflow());
    assert_eq!(
        app.last_rendered_history_tail
            .as_ref()
            .expect("inline history reflowed")
            .lines,
        deferred_history
    );

    insta::assert_snapshot!(
        "transcript_close_restores_inline_draft",
        buffer_text(crate::custom_terminal::test_support::last_rendered_buffer(
            &tui.terminal
        ))
        .replace(&test_path_display("/tmp/project"), "/tmp/project")
    );
    Ok(())
}

#[tokio::test]
async fn transcript_viewer_close_repaints_preserved_inline_draft() -> Result<()> {
    assert_transcript_close_repaints_inline_draft(make_test_app().await).await
}
