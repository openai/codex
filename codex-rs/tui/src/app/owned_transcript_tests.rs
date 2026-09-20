//! Owned transcript integration preserves composer spacing/input, prompt editing, and gestures.

use super::*;
use crate::app::tests::make_test_app_with_channels;
use crate::app_command::AppCommand;
use crate::app_event::ConsolidationScrollbackReflow;
use crate::chatwidget::tests::helpers::normalize_snapshot_paths;
use crate::history_cell::HistoryCell;
use crate::history_cell::UserHistoryCell;
use crate::session_state::ThreadSessionState;
use crate::test_support::test_path_buf;
use codex_app_server_protocol::AskForApproval;
use codex_config::types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use crossterm::event::MouseButton::Left;
use crossterm::event::MouseButton::Right;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use crossterm::event::MouseEventKind::Down;
use crossterm::event::MouseEventKind::Drag;
use crossterm::event::MouseEventKind::Up;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;

fn user_cell(message: &str) -> Arc<dyn HistoryCell> {
    Arc::new(UserHistoryCell {
        spoken: false,
        message: message.to_string(),
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: Vec::new(),
    })
}

fn attach_thread(app: &mut App, thread_id: ThreadId) {
    app.chat_widget.handle_thread_session(ThreadSessionState {
        windows_sandbox_host: crate::app::WindowsSandboxHost::Local,
        thread_id,
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "gpt-test".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        permission_profile: PermissionProfile::read_only(),
        active_permission_profile: None,
        cwd: test_path_buf("/tmp/project").abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: None,
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: None,
    });
}

fn buffer_text(buffer: &Buffer) -> String {
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

#[tokio::test]
async fn list_spacing_completion_preserves_the_scrolled_reader() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    app.transcript_cells = vec![Arc::new(history_cell::AgentMessageCell::new(
        vec![
            "- First item wraps onto".into(),
            "  a second row".into(),
            "- b".into(),
            "- c".into(),
        ],
        /*is_first_line*/ true,
    ))];
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 26, /*height*/ 1,
    );
    let mut before = Buffer::empty(area);
    app.transcript_view
        .jump_to_entry(&app.transcript_cells, /*index*/ 0);
    app.transcript_view
        .render(area, &mut before, &app.transcript_cells);
    app.transcript_view
        .scroll(&app.transcript_cells, /*rows*/ 3);
    app.transcript_view
        .render(area, &mut before, &app.transcript_cells);
    assert!(buffer_text(&before).contains("- c"));
    app.handle_consolidate_agent_message(
        &mut tui,
        "- First item wraps onto a second row\n- b\n- c".into(),
        app.config.cwd.to_path_buf(),
        /*inline_visualization_context*/ None,
        ConsolidationScrollbackReflow::Required,
        /*deferred_history_cell*/ None,
    )?;
    let mut after = Buffer::empty(area);
    app.transcript_view
        .render(area, &mut after, &app.transcript_cells);
    assert_eq!(after, before);
    app.transcript_view.jump_to_latest();
    app.transcript_view
        .render(area, &mut after, &app.transcript_cells);
    assert_eq!(after, before);
    Ok(())
}

#[tokio::test]
async fn older_page_loading_uses_the_status_row_without_moving_content_or_cursor() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    attach_thread(&mut app, ThreadId::new());
    app.local_settings.tui.animations = false;
    app.chat_widget.apply_external_edit("draft".to_string());
    app.transcript_cells = vec![Arc::new(crate::history_cell::PlainHistoryCell::new(
        (1..=40)
            .map(|row| format!("Transcript row {row:02}").into())
            .collect(),
    ))];
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let mut snapshots = Vec::new();
    for width in [80, 40, 28] {
        let size = Size::new(width, /*height*/ 12);
        tui.terminal.resize(size)?;
        app.transcript_view.history = TranscriptHistoryState::Partial;
        app.render_owned_transcript(&mut tui, size)?;
        app.transcript_view
            .scroll(&app.transcript_cells, /*rows*/ -3);
        let bottom = app.render_owned_transcript(&mut tui, size)?;
        let cursor = tui.terminal.last_known_cursor_pos;
        let before =
            crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal).clone();
        for state in [
            TranscriptHistoryState::LoadingOlder,
            TranscriptHistoryState::LoadingBeginning,
            TranscriptHistoryState::Failed,
            TranscriptHistoryState::Complete,
        ] {
            app.transcript_view.history = state;
            let actual_bottom = app.render_owned_transcript(&mut tui, size)?;
            let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
            assert_eq!(
                (actual_bottom, tui.terminal.last_known_cursor_pos),
                (bottom, cursor)
            );
            let status_start = buffer.index_of(/*x*/ 0, size.height - 1);
            assert_eq!(
                &buffer.content()[..status_start],
                &before.content()[..status_start]
            );
            if state == TranscriptHistoryState::Complete {
                assert_eq!(
                    buffer_text(buffer).lines().last().unwrap().trim(),
                    "esc latest",
                );
            }
            if state == TranscriptHistoryState::LoadingOlder {
                assert_eq!(
                    buffer[(2, size.height - 1)].fg,
                    crate::style::accent_color()
                );
            }
            let rendered = buffer_text(buffer);
            let rendered = rendered.lines().last().unwrap();
            snapshots.push(format!("{width} columns · {state:?}\n{rendered}"));
        }
    }
    insta::assert_snapshot!("owned_history_loading_footer", snapshots.join("\n\n"));
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn owned_transcript_reserves_a_row_above_the_composer() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    attach_thread(&mut app, ThreadId::new());
    app.transcript_cells = vec![Arc::new(crate::history_cell::PlainHistoryCell::new(
        (1..=40)
            .map(|row| format!("Transcript row {row:02}").into())
            .collect(),
    ))];
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let mut snapshots = Vec::new();
    for (label, width, height, draft) in [
        ("Latest", 80, 12, "draft"),
        ("Reading", 80, 12, "draft"),
        (
            "Resized multiline composer",
            40,
            12,
            "draft\nsecond line\nthird line",
        ),
        ("Tiny terminal", 40, 5, "draft"),
        ("Detailed", 80, 12, "preserved draft"),
    ] {
        let size = Size::new(width, height);
        tui.terminal.resize(size)?;
        app.chat_widget.apply_external_edit(draft.to_string());
        if label == "Detailed" {
            app.transcript_cells = vec![Arc::new(crate::history_cell::new_view_image_tool_call(
                codex_utils_path_uri::LegacyAppPathString::from_string("assets/detail-image.png"),
            ))];
            app.open_transcript_overlay(&mut tui);
            assert!(app.overlay.is_none() && app.transcript_view.is_detailed());
        }
        if label == "Reading" {
            app.transcript_view.handle_key(
                KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
                &app.transcript_cells,
            );
        }
        let bottom = app.render_owned_transcript(&mut tui, size)?;
        if bottom.y == 0 {
            assert_eq!(label, "Tiny terminal");
            snapshots.push(format!(
                "{label}\n{}",
                normalize_snapshot_paths(buffer_text(
                    crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal)
                ))
            ));
            continue;
        }
        let gap = Rect::new(/*x*/ 0, bottom.y - 1, width, /*height*/ 1);
        let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
        let start = buffer.index_of(gap.x, gap.y);
        if app.transcript_view.is_following() {
            assert_eq!(
                &buffer.content()[start..start + usize::from(width)],
                Buffer::empty(gap).content(),
                "{label}",
            );
        } else {
            assert!(
                buffer_text(buffer)
                    .lines()
                    .nth(usize::from(gap.y))
                    .unwrap()
                    .contains("Back to bottom")
            );
        }
        let rendered = normalize_snapshot_paths(buffer_text(buffer));
        snapshots.push(format!("{label}\n{rendered}"));
        assert!(
            app.transcript_view
                .handle_mouse(
                    crossterm::event::MouseEvent {
                        kind: crossterm::event::MouseEventKind::Down(
                            crossterm::event::MouseButton::Left
                        ),
                        column: gap.x,
                        row: gap.y,
                        modifiers: KeyModifiers::NONE,
                    },
                    &app.transcript_cells,
                )
                .is_none()
        );
    }
    insta::assert_snapshot!("owned_transcript_composer_gap", snapshots.join("\n\n"));
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn owned_drag_stops_when_focus_or_input_ownership_is_lost() -> Result<()> {
    use crossterm::event::MouseButton;
    use crossterm::event::MouseEvent;
    use crossterm::event::MouseEventKind;

    let mut app = crate::app::test_support::make_test_app().await;
    attach_thread(&mut app, ThreadId::new());
    app.transcript_cells = vec![Arc::new(crate::history_cell::PlainHistoryCell::new(
        (0..40).map(|row| format!("row {row:02}").into()).collect(),
    ))];
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let size = Size::new(/*width*/ 40, /*height*/ 12);
    app.chat_widget
        .apply_external_edit("retained draft 界".into());
    app.chat_widget.handle_key_event(KeyCode::Left.into());
    let draft = app.chat_widget.capture_thread_input_state();
    for owner in ["focus", "resume", "overlay", "popup"] {
        app.transcript_view = Default::default();
        app.overlay = None;
        app.render_owned_transcript(&mut tui, size)?;
        app.transcript_view
            .scroll(&app.transcript_cells, /*rows*/ -12);
        app.render_owned_transcript(&mut tui, size)?;
        for (kind, row) in [
            (MouseEventKind::Down(MouseButton::Left), 2),
            (MouseEventKind::Drag(MouseButton::Left), 0),
        ] {
            app.handle_owned_transcript_event(
                &mut tui,
                &mut app_server,
                &TuiEvent::Mouse(MouseEvent {
                    kind,
                    column: 3,
                    row,
                    modifiers: KeyModifiers::NONE,
                }),
            )?;
        }
        assert!(app.transcript_view.tick_selection(&app.transcript_cells));
        let selected = app.transcript_view.selected_text(&app.transcript_cells);
        let event = match owner {
            "focus" => TuiEvent::FocusLost,
            "resume" => TuiEvent::Resume,
            "overlay" => {
                app.overlay = Some(Overlay::new_static_with_lines(
                    vec!["overlay".into()],
                    "Overlay".to_string(),
                    app.keymap.pager.clone(),
                ));
                TuiEvent::Draw
            }
            "popup" => {
                app.chat_widget.open_feature_enable_prompt(Feature::Collab);
                TuiEvent::Draw
            }
            _ => unreachable!(),
        };
        if matches!(event, TuiEvent::Resume) {
            app.handle_tui_event(&mut tui, &mut app_server, event)
                .await?;
        } else {
            app.handle_owned_transcript_event(&mut tui, &mut app_server, &event)?;
        }
        assert_eq!(app.chat_widget.capture_thread_input_state(), draft);
        assert!(
            !app.transcript_view.tick_selection(&app.transcript_cells),
            "{owner}"
        );
        assert_eq!(
            app.transcript_view.selected_text(&app.transcript_cells),
            selected
        );
    }
    tui.set_owned_screen(/*owned*/ false)?;
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn owned_details_keep_the_composer_cursor_and_screen() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    attach_thread(&mut app, ThreadId::new());
    app.transcript_cells = vec![user_cell("First prompt"), user_cell("Second prompt")];
    app.chat_widget
        .apply_external_edit("preserved draft".to_string());
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.open_transcript_overlay(&mut tui);
    let bottom_area =
        app.render_owned_transcript(&mut tui, Size::new(/*width*/ 80, /*height*/ 24))?;
    let expected_cursor = app
        .chat_widget
        .bottom_pane_renderable(
            /*footer*/ None,
            crate::bottom_pane::CommandPopupPlacement::Overlay,
        )
        .cursor_pos(bottom_area)
        .expect("composer cursor");
    assert_eq!(
        (
            app.overlay.is_none(),
            app.transcript_view.is_detailed(),
            tui.is_owned_screen(),
            tui.is_alt_screen_active()
        ),
        (true, true, true, true),
    );
    assert_eq!(tui.terminal.last_known_cursor_pos, expected_cursor.into());
    let rendered = normalize_snapshot_paths(buffer_text(
        crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal),
    ));
    assert!(rendered.contains("First prompt"));
    assert!(rendered.contains("Second prompt"));
    assert!(rendered.contains("preserved draft"));
    assert!(app.handle_owned_backtrack_event(
        &mut tui,
        &TuiEvent::Key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)),
    )?);
    assert_eq!(
        (
            app.transcript_view.is_detailed(),
            tui.is_owned_screen(),
            app.chat_widget.composer_text_with_pending()
        ),
        (false, true, "preserved draft".to_string()),
    );
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn owned_transcript_keeps_text_out_of_the_pet_columns() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    app.transcript_cells = vec![Arc::new(crate::history_cell::PlainHistoryCell::new(vec![
        "x".repeat(/*n*/ 150).into(),
    ]))];
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let size = Size::new(/*width*/ 80, /*height*/ 24);
    app.render_owned_transcript(&mut tui, size)?;
    app.chat_widget
        .set_pet_image_support_for_tests(crate::pets::PetImageSupport::Supported(
            crate::pets::ImageProtocol::Kitty,
        ));
    app.chat_widget
        .install_test_ambient_pet_for_tests(/*animations_enabled*/ false);
    let width = app.chat_widget.history_wrap_width(size.width);
    assert!(width < size.width);
    let bottom = app.render_owned_transcript(&mut tui, size)?;
    let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
    assert!(buffer_text(buffer).contains(&"x".repeat(/*n*/ 60)));
    for y in 0..bottom.y {
        for x in width..size.width {
            assert_eq!(buffer[(x, y)].symbol(), " ");
        }
    }
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn owned_details_escape_interrupts_work_without_starting_backtrack() -> Result<()> {
    let (mut app, mut events, _operations) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    attach_thread(&mut app, thread_id);
    app.transcript_cells = vec![user_cell("historical prompt")];
    app.chat_widget
        .apply_external_edit("draft survives interrupt".to_string());
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.open_transcript_overlay(&mut tui);
    app.chat_widget.handle_server_notification(
        ServerNotification::TurnStarted(codex_app_server_protocol::TurnStartedNotification {
            thread_id: thread_id.to_string(),
            turn: codex_app_server_protocol::Turn {
                id: "active-turn".to_string(),
                items_view: codex_app_server_protocol::TurnItemsView::Full,
                items: Vec::new(),
                status: codex_app_server_protocol::TurnStatus::InProgress,
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            },
        }),
        /*replay_kind*/ None,
    );
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    )
    .await?;
    let interrupts = std::iter::from_fn(|| events.try_recv().ok())
        .filter(|event| matches!(event, AppEvent::CodexOp(AppCommand::Interrupt)))
        .count();
    assert_eq!(
        (
            interrupts,
            app.backtrack.overlay_preview_active,
            app.transcript_view.is_detailed(),
            app.chat_widget.composer_text_with_pending(),
        ),
        (1, false, true, "draft survives interrupt".to_string()),
    );
    tui.set_owned_screen(/*owned*/ false)?;
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn owned_backtrack_keys_edit_the_selected_prompt_and_restore_compact_view() -> Result<()> {
    let (mut app, mut events, _operations) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    attach_thread(&mut app, thread_id);
    app.transcript_cells = vec![user_cell("first"), user_cell("second")];
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.open_transcript_overlay(&mut tui);
    for (code, selected) in [(KeyCode::Esc, 1), (KeyCode::Left, 0), (KeyCode::Right, 1)] {
        assert!(app.handle_owned_backtrack_event(
            &mut tui,
            &TuiEvent::Key(KeyEvent::new(code, KeyModifiers::NONE))
        )?);
        assert_eq!(
            (
                app.backtrack.overlay_preview_active,
                app.backtrack.nth_user_message
            ),
            (true, selected)
        );
    }
    assert!(app.handle_owned_backtrack_event(
        &mut tui,
        &TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
    )?);
    let selection = std::iter::from_fn(|| events.try_recv().ok()).find_map(|event| match event {
        AppEvent::RevertSessionForPromptEdit {
            thread_id,
            selected_cell,
            prompt,
        } => Some((
            thread_id,
            Arc::ptr_eq(&selected_cell, &app.transcript_cells[1]),
            prompt,
        )),
        _ => None,
    });
    assert_eq!(
        selection,
        Some((
            thread_id,
            true,
            crate::chatwidget::UserMessage::from("second")
        ))
    );
    assert_eq!(
        (
            app.overlay.is_none(),
            app.transcript_view.is_detailed(),
            app.backtrack.overlay_preview_active,
            app.backtrack.base_id,
            tui.is_owned_screen()
        ),
        (true, false, false, None, true),
    );
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn cancelling_owned_backtrack_returns_arrow_keys_to_the_composer() -> Result<()> {
    for input in [
        vec![TuiEvent::Paste("abc".to_string())],
        ['a', 'b', 'c']
            .map(|c| TuiEvent::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)))
            .into(),
    ] {
        let mut app = crate::app::test_support::make_test_app().await;
        attach_thread(&mut app, ThreadId::new());
        app.transcript_cells = vec![user_cell("First prompt"), user_cell("Second prompt")];
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        tui.set_owned_screen(/*owned*/ true)?;
        app.open_transcript_overlay(&mut tui);
        let size = Size::new(/*width*/ 80, /*height*/ 12);
        app.render_owned_transcript(&mut tui, size)?;
        app.handle_tui_event(
            &mut tui,
            &mut app_server,
            TuiEvent::Key(KeyCode::Esc.into()),
        )
        .await?;
        for event in input {
            app.handle_tui_event(&mut tui, &mut app_server, event)
                .await?;
        }
        // An ordinary key cancels preview before reaching the composer. Its arrow-key owner must end
        // with the highlight, without requiring the user to toggle out of detailed presentation.
        // Moving the caret also flushes buffered typing without depending on platform paste timers.
        app.handle_tui_event(
            &mut tui,
            &mut app_server,
            TuiEvent::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
        )
        .await?;
        assert_eq!(app.chat_widget.composer_text_with_pending(), "abc");
        for event in [
            TuiEvent::Paste("X".to_string()),
            TuiEvent::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            TuiEvent::Paste("Y".to_string()),
        ] {
            app.handle_tui_event(&mut tui, &mut app_server, event)
                .await?;
        }
        assert_eq!(app.chat_widget.composer_text_with_pending(), "abXcY");
        assert_eq!(
            (
                app.transcript_view.is_detailed(),
                app.backtrack.primed,
                app.backtrack.overlay_preview_active,
                app.backtrack.base_id,
            ),
            (true, false, false, None),
        );
        assert!(!app.handle_owned_backtrack_event(
            &mut tui,
            &TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        )?);
        app.render_owned_transcript(&mut tui, size)?;
        tui.set_owned_screen(/*owned*/ false)?;
        app_server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn owned_search_and_selection_consume_input_before_composer_and_backtrack() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    attach_thread(&mut app, ThreadId::new());
    app.transcript_cells = vec![user_cell("needle in history")];
    app.chat_widget
        .apply_external_edit("composer draft".to_string());
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.open_transcript_overlay(&mut tui);
    app.render_owned_transcript(&mut tui, Size::new(/*width*/ 80, /*height*/ 24))?;
    for event in [
        TuiEvent::Key(KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE)),
        TuiEvent::Paste("needle".to_string()),
    ] {
        assert!(app.handle_owned_transcript_event(&mut tui, &mut app_server, &event)?);
    }
    app.handle_owned_transcript_event(&mut tui, &mut app_server, &TuiEvent::Draw)?;
    app.render_owned_transcript(&mut tui, Size::new(/*width*/ 80, /*height*/ 24))?;
    assert!(
        app.transcript_view
            .search_footer(/*width*/ 80)
            .expect("query footer")
            .0
            .to_string()
            .starts_with("Find: needle")
    );
    let escape = TuiEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.handle_owned_transcript_event(&mut tui, &mut app_server, &escape)?);
    assert!(!app.backtrack.overlay_preview_active);
    // Start and extend a selection through the app, then inspect its copy action without touching
    // the host clipboard. The app's existing clipboard handler is tested with an injected writer.
    for key in [
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
    ] {
        assert!(app.handle_owned_transcript_event(
            &mut tui,
            &mut app_server,
            &TuiEvent::Key(key)
        )?);
    }
    let copy = app.transcript_view.handle_key(
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        &app.transcript_cells,
    );
    assert!(matches!(copy, Some(ViewAction::Copy(text)) if !text.is_empty()));
    assert!(app.handle_owned_transcript_event(&mut tui, &mut app_server, &escape)?);
    assert!(!app.backtrack.overlay_preview_active);
    assert!(app.handle_owned_transcript_event(&mut tui, &mut app_server, &escape)?);
    assert!(app.transcript_view.is_following());
    assert!(!app.backtrack.overlay_preview_active);
    assert!(!app.handle_owned_transcript_event(&mut tui, &mut app_server, &escape)?);
    assert!(!app.backtrack.overlay_preview_active);
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "composer draft"
    );
    tui.set_owned_screen(/*owned*/ false)?;
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn inline_transcript_search_draws_and_escape_precedes_backtrack() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    attach_thread(&mut app, ThreadId::new());
    app.transcript_cells = vec![user_cell("needle in history")];
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.open_transcript_overlay(&mut tui);
    for event in [
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE)),
        TuiEvent::Paste("needle".to_string()),
        TuiEvent::Draw,
    ] {
        app.handle_backtrack_overlay_event(&mut tui, &mut app_server, event)
            .await?;
    }
    let rendered = buffer_text(crate::custom_terminal::test_support::last_rendered_buffer(
        &tui.terminal,
    ));
    assert!(
        rendered.contains("enter next"),
        "search advances on the inline overlay draw path"
    );
    app.handle_backtrack_overlay_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    )
    .await?;
    assert!(
        matches!(&app.overlay, Some(Overlay::Transcript(overlay)) if !overlay.has_active_interaction())
    );
    assert!(!app.backtrack.overlay_preview_active);
    app.handle_backtrack_overlay_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    )
    .await?;
    assert!(app.backtrack.overlay_preview_active);
    app.close_transcript_overlay(&mut tui);
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn find_owns_editor_chords_without_changing_the_composer_draft() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    app.transcript_cells = vec![user_cell("alpha beta")];
    app.chat_widget
        .apply_external_edit("draft remains intact".to_string());
    app.keymap = RuntimeKeymap::from_config(&serde_json::from_value(serde_json::json!({
        "global": {"find_transcript": "f4", "open_external_editor": "ctrl-g e"},
        "editor": {"move_line_start": "ctrl-x h"},
        "pager": {"find": "f4"}
    }))?)
    .expect("independent query bindings");
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    for owned in [true, false] {
        tui.set_owned_screen(owned)?;
        if !owned {
            app.open_transcript_overlay(&mut tui);
        }
        for event in [
            TuiEvent::Key(KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE)),
            TuiEvent::Draw,
            TuiEvent::Key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL)),
            TuiEvent::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            TuiEvent::Paste("alpha beta".to_string()),
            // A global chord cannot own the next query character.
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL)),
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            // Configured editor chords use the same query TextArea.
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE)),
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)),
            TuiEvent::Key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE)),
        ] {
            let pasted = matches!(event, TuiEvent::Paste(_));
            app.handle_tui_event(&mut tui, &mut app_server, event)
                .await?;
            if pasted {
                app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
                    .await?;
                let rendered = buffer_text(
                    crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal),
                );
                assert!(
                    rendered.contains("enter next"),
                    "Find must resume after pasting over selection"
                );
            }
            if app.key_chord_matcher.is_pending() {
                app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
                    .await?;
                let rendered = buffer_text(
                    crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal),
                );
                let cursor = tui.terminal.last_known_cursor_pos;
                let query_row = rendered
                    .lines()
                    .nth(usize::from(cursor.y))
                    .expect("visible query caret");
                assert_eq!(query_row.trim(), "Find: alpha betax");
                assert!(rendered.contains("h move line start"));
                insta::assert_snapshot!(
                    "owned_find_pending_chord_footer",
                    rendered
                        .lines()
                        .skip(usize::from(cursor.y))
                        .take(/*n*/ 2)
                        .map(str::trim)
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
        }
        app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
            .await?;
        let rendered = buffer_text(crate::custom_terminal::test_support::last_rendered_buffer(
            &tui.terminal,
        ));
        assert!(
            rendered
                .lines()
                .any(|line| line.trim() == "Find: apha betax")
        );
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            "draft remains intact"
        );
        app.handle_tui_event(
            &mut tui,
            &mut app_server,
            TuiEvent::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        )
        .await?;
        assert!(!match &app.overlay {
            Some(Overlay::Transcript(overlay)) => overlay.is_search_active(),
            _ => app.transcript_view.is_search_active(),
        });
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            "draft remains intact"
        );
        app.close_transcript_overlay(&mut tui);
    }
    tui.set_owned_screen(/*owned*/ false)?;
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn offline_find_closes_before_the_next_ctrl_c_quits() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    app.chat_widget
        .apply_external_edit("offline draft".to_string());
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.open_transcript_overlay(&mut tui);
    app.transcript_view.begin_search();
    app.transcript_view.paste_search("needle");
    app.reconnect.offline = true;
    app.chat_widget.pause_for_disconnect();
    app.render_owned_transcript(&mut tui, Size::new(/*width*/ 80, /*height*/ 24))?;
    let rendered = buffer_text(crate::custom_terminal::test_support::last_rendered_buffer(
        &tui.terminal,
    ));
    let cursor = tui.terminal.last_known_cursor_pos;
    assert_eq!(
        rendered.lines().nth(usize::from(cursor.y)).map(str::trim),
        Some("Find: needle")
    );
    assert!(!rendered.contains("ctrl+c quit"));
    let close = TuiEvent::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(matches!(
        app.handle_tui_event(&mut tui, &mut app_server, close)
            .await?,
        AppRunControl::Continue
    ));
    assert!(!app.transcript_view.is_search_active());
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "offline draft"
    );
    assert!(app.transcript_view.is_detailed());
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)),
    )
    .await?;
    assert!(!app.transcript_view.is_detailed());
    let quit = TuiEvent::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(matches!(
        app.handle_tui_event(&mut tui, &mut app_server, quit)
            .await?,
        AppRunControl::Exit(ExitReason::UserRequested)
    ));
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn offline_backtrack_keeps_the_preview_and_draft_without_reverting() -> Result<()> {
    for owned in [true, false] {
        let (mut app, mut events, _operations) = make_test_app_with_channels().await;
        attach_thread(&mut app, ThreadId::new());
        app.transcript_cells = vec![user_cell("earlier prompt"), user_cell("historical prompt")];
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        tui.set_owned_screen(owned)?;
        app.open_transcript_overlay(&mut tui);
        app.handle_tui_event(
            &mut tui,
            &mut app_server,
            TuiEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        )
        .await?;
        app.chat_widget
            .apply_external_edit("offline draft".to_string());
        app.reconnect.offline = true;
        for key in [
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
        ] {
            app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(key))
                .await?;
        }
        assert_eq!(app.backtrack.nth_user_message, 0);
        assert!(if owned {
            app.transcript_view.is_detailed()
        } else {
            matches!(&app.overlay, Some(Overlay::Transcript(overlay)) if overlay.is_detailed())
        });
        assert!(matches!(
            app.handle_tui_event(
                &mut tui,
                &mut app_server,
                TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            )
            .await?,
            AppRunControl::Continue
        ));
        assert_eq!(
            (
                app.backtrack.overlay_preview_active,
                app.chat_widget.composer_text_with_pending()
            ),
            (true, "offline draft".to_string())
        );
        assert!(
            !std::iter::from_fn(|| events.try_recv().ok())
                .any(|event| matches!(event, AppEvent::RevertSessionForPromptEdit { .. }))
        );
        app.handle_tui_event(
            &mut tui,
            &mut app_server,
            TuiEvent::Key(KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE)),
        )
        .await?;
        assert!(if owned {
            app.transcript_view.is_search_active()
        } else {
            matches!(&app.overlay, Some(Overlay::Transcript(overlay)) if overlay.is_search_active())
        });
        for (key, preview_active, draft) in [
            (KeyCode::Esc, true, "offline draft"),
            (
                if owned {
                    KeyCode::Backspace
                } else {
                    KeyCode::Esc
                },
                false,
                if owned {
                    "offline draf"
                } else {
                    "offline draft"
                },
            ),
        ] {
            app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(key.into()))
                .await?;
            assert_eq!(app.backtrack.overlay_preview_active, preview_active);
            assert_eq!(app.chat_widget.composer_text_with_pending(), draft);
        }
        tui.set_owned_screen(/*owned*/ false)?;
        app_server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn slash_picker_overlays_history_without_moving_the_transcript_or_composer() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    attach_thread(&mut app, ThreadId::new());
    app.chat_widget
        .set_status_line(Some("gpt-test default · /tmp/project".into()));
    app.local_settings.tui.animations = false;
    app.chat_widget
        .set_footer_hint_override(Some(vec![("model".to_string(), "high · fast".to_string())]));
    app.transcript_cells = vec![Arc::new(crate::history_cell::PlainHistoryCell::new(
        (1..=40)
            .map(|row| format!("Transcript row {row:02}: content behind the menu").into())
            .collect(),
    ))];
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    for (width, height) in [(80, 14), (32, 14), (80, 7), (80, 5)] {
        let size = Size::new(width, height);
        tui.terminal.resize(size)?;
        app.chat_widget.apply_external_edit("/m".to_string());
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        app.render_owned_transcript(&mut tui, size)?;
        app.transcript_view
            .scroll(&app.transcript_cells, /*rows*/ -3);
        let bottom = app.render_owned_transcript(&mut tui, size)?;
        let cursor = tui.terminal.last_known_cursor_pos;
        let before =
            crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal).clone();

        // Changing the token reopens the menu dismissed above.
        app.chat_widget.apply_external_edit("/mo".to_string());
        app.chat_widget.apply_external_edit("/m".to_string());
        assert_eq!(app.render_owned_transcript(&mut tui, size)?, bottom);
        assert_eq!(tui.terminal.last_known_cursor_pos, cursor);
        let open =
            crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal).clone();
        let open_text = buffer_text(&open);
        if bottom.y > 0 {
            let menu_y = open_text
                .lines()
                .position(|line| line.contains("› /model"))
                .expect("available rows show command matches");
            let prefix_len = menu_y * usize::from(width);
            assert_eq!(
                &open.content()[..prefix_len],
                &before.content()[..prefix_len]
            );
            assert_ne!(open, before);
        }
        let composer_start = open.index_of(bottom.x, bottom.y);
        assert_eq!(
            &open.content()[composer_start..],
            &before.content()[composer_start..]
        );
        if (width, height) == (80, 14) {
            insta::assert_snapshot!("slash_picker_overlays_history", open_text);
        }

        app.chat_widget.apply_external_edit("/mo".to_string());
        assert_eq!(app.render_owned_transcript(&mut tui, size)?, bottom);
        let filtered = buffer_text(crate::custom_terminal::test_support::last_rendered_buffer(
            &tui.terminal,
        ));
        assert_eq!(
            filtered
                .lines()
                .take(usize::from(bottom.y))
                .any(|line| line.contains("› /model")),
            bottom.y > 0,
            "a single command remains visible when the menu has room",
        );
        app.chat_widget.apply_external_edit("/m".to_string());
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.render_owned_transcript(&mut tui, size)?, bottom);
        assert_eq!(tui.terminal.last_known_cursor_pos, cursor);
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.render_owned_transcript(&mut tui, size)?, bottom);
        assert_eq!(
            (
                crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal),
                tui.terminal.last_known_cursor_pos
            ),
            (&before, cursor),
        );
    }
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn double_escape_browses_prompts_without_reverting_and_explains_editing() -> Result<()> {
    let (mut app, mut events, _operations) = make_test_app_with_channels().await;
    app.keymap = RuntimeKeymap::from_config(&toml::from_str(
        "[editor]\nmove_left=[]\nmove_line_start='left h'\n[global]\nopen_transcript='ctrl-x h'\n",
    )?)
    .unwrap();
    attach_thread(&mut app, ThreadId::new());
    app.transcript_cells = vec![
        user_cell("first question"),
        user_cell("second question"),
        user_cell("third question"),
    ];
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let size = Size::new(/*width*/ 80, /*height*/ 16);
    app.render_owned_transcript(&mut tui, size)?;
    for code in [KeyCode::Esc, KeyCode::Esc] {
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyEvent::from(code)))
            .await?;
    }
    assert!(app.backtrack.overlay_preview_active);
    assert_eq!(app.backtrack.nth_user_message, 2);
    for key in [
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        KeyCode::Char('h').into(),
    ] {
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key))
            .await?;
    }
    assert!(app.transcript_view.is_detailed());
    assert_eq!(app.backtrack.nth_user_message, 2);
    for key in [
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        KeyCode::Char('h').into(),
    ] {
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key))
            .await?;
    }
    assert!(!app.transcript_view.is_detailed());

    for (code, index) in [(KeyCode::Left, 1), (KeyCode::Left, 0), (KeyCode::Right, 1)] {
        app.render_owned_transcript(&mut tui, size)?;
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(code.into()))
            .await?;
        assert!(!app.key_chord_matcher.is_pending());
        assert_eq!(app.backtrack.nth_user_message, index);
    }
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::RevertSessionForPromptEdit { .. }))
    );
    app.keymap = RuntimeKeymap::defaults();
    app.render_owned_transcript(&mut tui, size)?;
    insta::assert_snapshot!(
        "prompt_navigation",
        normalize_snapshot_paths(buffer_text(
            crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal)
        ))
    );
    app.close_transcript_overlay(&mut tui);
    assert!(!app.backtrack.overlay_preview_active);
    server.shutdown().await?;
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[path = "owned_transcript_browsing_tests.rs"]
mod browsing;

#[tokio::test]
async fn find_refreshes_live_details_before_searching_the_first_query() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    crate::chatwidget::tests::helpers::set_active_cell(
        &mut app.chat_widget,
        Box::new(crate::exec_cell::new_active_exec_command(
            "live".into(),
            vec!["printf visible\nprintf needle".into()],
            Vec::new(),
            codex_app_server_protocol::CommandExecutionSource::Agent,
            /*interaction_input*/ None,
            /*animations_enabled*/ false,
        )),
    );
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.render_owned_transcript(&mut tui, Size::new(/*width*/ 80, /*height*/ 24))?;
    assert!(
        !buffer_text(crate::custom_terminal::test_support::last_rendered_buffer(
            &tui.terminal
        ))
        .contains("needle")
    );
    for event in [
        TuiEvent::Key(KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE)),
        TuiEvent::Paste("needle".into()),
        TuiEvent::Draw,
    ] {
        app.handle_tui_event(&mut tui, &mut app_server, event)
            .await?;
    }
    let rendered = buffer_text(crate::custom_terminal::test_support::last_rendered_buffer(
        &tui.terminal,
    ));
    assert!(rendered.contains("enter next"), "{rendered}");
    tui.set_owned_screen(/*owned*/ false)?;
    app_server.shutdown().await?;
    Ok(())
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> TuiEvent {
    TuiEvent::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn row_containing(tui: &tui::Tui, text: &str) -> u16 {
    let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
    buffer_text(buffer)
        .lines()
        .position(|row| row.contains(text))
        .unwrap() as u16
}

#[tokio::test]
async fn fullscreen_composer_mouse_copy_and_input_ownership() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let size = tui.terminal.size()?;
    app.transcript_cells = vec![user_cell("transcript text")];
    app.chat_widget.apply_external_edit("hello world".into());
    app.render_owned_transcript(&mut tui, size)?;
    let end = tui.terminal.last_known_cursor_pos;
    let x = end.x - 11;
    let y = end.y;
    for event in [
        mouse(Down(Left), x, y),
        mouse(Drag(Left), x + 5, y),
        mouse(Up(Left), x + 5, y),
    ] {
        assert!(app.handle_owned_transcript_event(&mut tui, &mut server, &event)?);
    }
    assert!(!app.transcript_view.has_active_interaction());
    app.render_owned_transcript(&mut tui, size)?;
    let cursor = tui.terminal.last_known_cursor_pos;
    let draft = app.chat_widget.capture_thread_input_state();
    for event in [
        mouse(Up(Right), x + 2, y),
        mouse(Down(Right), /*column*/ 0, y),
        mouse(Down(Right), size.width, y),
        mouse(Down(Right), x, y - 1),
        mouse(Down(Right), x, y + 1),
    ] {
        assert!(!app.handle_composer_copy_event(&mut tui, &event, |_, _| unreachable!()));
    }
    let copy_events = [
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::SUPER)),
        mouse(Down(Right), x + 2, y),
    ];
    let mut selection_frames = Vec::new();
    for event in &copy_events {
        for result in [
            Err("clipboard unavailable".to_string()),
            Ok(crate::clipboard_copy::CopyStatus::Unconfirmed),
            Ok(crate::clipboard_copy::CopyStatus::Confirmed),
        ] {
            assert!(app.handle_composer_copy_event(&mut tui, event, |_, text| {
                assert_eq!(text, "hello");
                result.clone()
            }));
            app.render_owned_transcript(&mut tui, size)?;
            let buffer = crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal);
            let rendered_draft = (x..x + 11)
                .map(|column| buffer[(column, y)].symbol())
                .collect::<String>();
            let selection = (x..x + 11)
                .map(|column| {
                    if buffer[(column, y)]
                        .modifier
                        .contains(ratatui::style::Modifier::REVERSED)
                    {
                        '^'
                    } else {
                        '·'
                    }
                })
                .collect::<String>();
            let cleared = matches!(event, TuiEvent::Mouse(_))
                && result == Ok(crate::clipboard_copy::CopyStatus::Confirmed);
            assert_eq!(
                (
                    app.chat_widget.capture_thread_input_state(),
                    tui.terminal.last_known_cursor_pos,
                    selection.as_str(),
                ),
                (
                    draft.clone(),
                    cursor,
                    if cleared {
                        "···········"
                    } else {
                        "^^^^^······"
                    },
                )
            );
            let gesture = if matches!(event, TuiEvent::Mouse(_)) {
                "right-click"
            } else {
                "keyboard"
            };
            selection_frames.push(format!(
                "{gesture} {result:?}\n{rendered_draft}\n{selection}"
            ));
        }
    }
    assert_eq!(app.chat_widget.capture_thread_input_state(), draft);
    app.render_owned_transcript(&mut tui, size)?;
    assert!(row_containing(&tui, "Copied 5 chars to host clipboard") < y);
    insta::assert_snapshot!(
        "fullscreen_composer_right_click_copy",
        format!(
            "{}\n\n{}",
            selection_frames.join("\n\n"),
            normalize_snapshot_paths(buffer_text(
                crate::custom_terminal::test_support::last_rendered_buffer(&tui.terminal)
            ))
        )
    );

    app.chat_widget.handle_key_event(KeyCode::Char('x').into());
    for event in &copy_events {
        assert!(!app.handle_composer_copy_event(&mut tui, event, |_, _| unreachable!()));
    }
    assert_eq!(app.chat_widget.composer_text_with_pending(), "hellox world");

    // A transcript drag that crosses into the composer remains a transcript selection.
    let transcript_y = row_containing(&tui, "transcript text");
    for event in [
        mouse(Down(Left), /*column*/ 1, transcript_y),
        mouse(Drag(Left), x + 3, y),
        mouse(Up(Left), x + 3, y),
    ] {
        app.handle_owned_transcript_event(&mut tui, &mut server, &event)?;
    }
    assert!(app.transcript_view.has_active_interaction());
    for event in &copy_events {
        assert!(!app.handle_composer_copy_event(&mut tui, event, |_, _| unreachable!()));
    }

    // A fresh composer click takes ownership away from the transcript selection.
    app.handle_owned_transcript_event(&mut tui, &mut server, &mouse(Down(Left), x, y))?;
    assert!(!app.transcript_view.has_active_interaction());
    app.chat_widget.apply_external_edit("/".into());
    assert!(!app.chat_widget.no_modal_or_popup_active());
    assert!(app.handle_owned_transcript_event(&mut tui, &mut server, &mouse(Down(Left), x, y))?);
    app.handle_owned_transcript_event(&mut tui, &mut server, &mouse(Drag(Left), x + 1, y))?;
    assert!(app.chat_widget.no_modal_or_popup_active());
    app.chat_widget.open_feature_enable_prompt(Feature::Collab);
    for event in &copy_events {
        assert!(
            !app.handle_composer_copy_event(&mut tui, event, |_, _| panic!(
                "modals own copy input"
            ))
        );
    }
    assert!(!app.handle_owned_transcript_event(&mut tui, &mut server, &mouse(Down(Left), x, y))?);
    server.shutdown().await?;
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}
