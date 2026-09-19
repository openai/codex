//! Owned transcript integration preserves composer spacing/input, prompt editing, and gestures.

use super::*;
use crate::chatwidget::tests::helpers::normalize_snapshot_paths;
use crate::session_state::ThreadSessionState;
use crate::test_support::test_path_buf;
use codex_app_server_protocol::AskForApproval;
use codex_config::types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;

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
    for owner in ["focus", "overlay", "popup"] {
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
        app.handle_owned_transcript_event(&mut tui, &mut app_server, &event)?;
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
