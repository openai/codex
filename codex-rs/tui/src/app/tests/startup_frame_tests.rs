//! Startup handoff preserves the loading frame until replayed cells can replace it.

use super::*;
use crate::chatwidget::tests::helpers::normalize_agent_center_snapshot;
use crate::chatwidget::tests::helpers::normalize_snapshot_paths;
use crate::custom_terminal::test_support::last_rendered_buffer;
use crate::startup_draft::tests::quiet_startup_test_pump;
use pretty_assertions::assert_eq;

fn frame_text(tui: &tui::Tui) -> String {
    last_rendered_buffer(&tui.terminal)
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

#[tokio::test]
async fn owned_startup_tip_follows_splash_in_transcript() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    app.local_settings.tui.show_tooltips = true;
    app.local_settings.tui.animations = false;
    let session = test_thread_session(ThreadId::new(), app.config.cwd.to_path_buf());
    app.transcript_cells = vec![
        Arc::new(new_session_info(
            &app.config,
            &app.local_settings,
            &session.model,
            &session.model,
            &session,
            /*is_first_event*/ false,
            Some("Use /mcp to list configured MCP tools.".into()),
            /*auth_plan*/ None,
            /*show_fast_status*/ false,
        )),
        Arc::new(AgentMessageCell::new(
            vec!["Conversation continues here.".into()],
            /*is_first_line*/ true,
        )),
    ];
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let mut snapshots = Vec::new();
    for width in [80, 32] {
        let size = Size::new(width, /*height*/ 20);
        tui.screen_size_for_event(&TuiEvent::Resize(size))?;
        app.render_owned_transcript(&mut tui, size)?;
        let buffer = last_rendered_buffer(&tui.terminal);
        let screen = buffer
            .content()
            .chunks(usize::from(buffer.area.width))
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n");
        snapshots.push(format!("{width} columns\n{screen}"));
    }
    insta::assert_snapshot!(
        "startup_tip_in_transcript",
        normalize_snapshot_paths(normalize_agent_center_snapshot(snapshots.join("\n\n"))),
    );
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}

#[tokio::test]
async fn owned_startup_preserves_loading_until_resume_replay_is_applied() -> Result<()> {
    for items in [
        Vec::new(),
        vec![ThreadItem::AgentMessage {
            id: "answer".into(),
            text: "Retained **answer** after resume.".into(),
            phase: None,
            memory_citation: None,
            delivery: None,
            questions: None,
        }],
    ] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        app.local_settings.tui.show_tooltips = false;
        let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        tui.set_owned_screen(/*owned*/ true)?;
        let thread_id = ThreadId::new();
        let mut startup = quiet_startup_test_pump();
        startup.apply_config(&app.config);
        startup.update_session_selection(
            &mut tui,
            &SessionSelection::Resume(crate::resume_picker::SessionTarget {
                path: None,
                thread_id,
                cwd: None,
                history_mode: None,
            }),
        )?;
        let loading = last_rendered_buffer(&tui.terminal).clone();
        let loading_cursor = tui.terminal.last_known_cursor_pos;
        assert!(frame_text(&tui).contains("Resuming session…"));

        let has_answer = !items.is_empty();
        let turns = if has_answer {
            vec![test_turn("resumed-turn", TurnStatus::Completed, items)]
        } else {
            Vec::new()
        };
        app.enqueue_primary_thread_session(
            test_thread_session(thread_id, app.config.cwd.to_path_buf()),
            turns,
        )
        .await?;
        let mut draft = Some(startup.into_draft());
        assert!(!events.is_empty());
        assert!(app.transcript_cells.is_empty());
        while !events.is_empty() {
            app.render_startup_frame(&mut tui, &events)?;
            assert_eq!(last_rendered_buffer(&tui.terminal), &loading);
            assert_eq!(tui.terminal.last_known_cursor_pos, loading_cursor);
            let event = events.try_recv()?;
            let control = Box::pin(app.handle_event(&mut tui, &mut app_server, event)).await?;
            assert!(matches!(control, AppRunControl::Continue));
        }
        app.chat_widget.restore_startup_draft_when_ready(&mut draft);
        app.render_startup_frame(&mut tui, &events)?;
        assert!(draft.is_none());
        let rendered = frame_text(&tui);
        assert!(!rendered.contains("Resuming session…"));
        if has_answer {
            assert!(rendered.contains("Retained answer after resume."));
        } else {
            assert!(rendered.contains("OpenAI Codex"));
        }
        tui.set_owned_screen(/*owned*/ false)?;
        app_server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn owned_startup_renders_visible_decisions_and_overview_before_pending_events() -> Result<()>
{
    for overview in [false, true] {
        let (mut app, events, _ops) = make_test_app_with_channels().await;
        app.app_event_tx
            .send(AppEvent::BeginInitialHistoryReplayBuffer);
        if overview {
            let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
            app.chat_widget.show_bottom_pane_view(Box::new(view));
        } else {
            let preset = codex_utils_approval_presets::builtin_approval_presets()
                .into_iter()
                .find(|preset| preset.id == "auto")
                .expect("auto preset");
            app.chat_widget
                .open_windows_sandbox_enable_prompt(preset, /*profile_selection*/ None);
        }
        let mut tui = crate::tui::test_support::make_test_tui()?;
        tui.set_owned_screen(/*owned*/ true)?;
        assert!(!events.is_empty());
        app.render_startup_frame(&mut tui, &events)?;
        assert!(frame_text(&tui).contains(if overview {
            "Agent command center"
        } else {
            "sandbox"
        }));
        tui.set_owned_screen(/*owned*/ false)?;
    }
    Ok(())
}

#[tokio::test]
async fn inline_startup_still_renders_with_pending_history() -> Result<()> {
    let (mut app, events, _ops) = make_test_app_with_channels().await;
    app.app_event_tx
        .send(AppEvent::BeginInitialHistoryReplayBuffer);
    app.chat_widget
        .apply_external_edit("inline startup draft".into());
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.render_startup_frame(&mut tui, &events)?;
    assert!(frame_text(&tui).contains("inline startup draft"));
    Ok(())
}
