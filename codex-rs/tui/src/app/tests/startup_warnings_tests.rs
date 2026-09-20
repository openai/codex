//! Startup diagnostic ordering, aggregation, and transcript coverage.

use super::*;
use crate::history_cell::StartupWarningsCell;
use codex_app_server_protocol::McpServerStartupFailureReason;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::UnboundedReceiver;

fn insert_warnings(app: &mut App, tui: &mut tui::Tui, messages: &[&str]) {
    app.insert_history_cell(
        tui,
        Box::new(StartupWarningsCell::new(
            messages.iter().map(|message| (*message).into()).collect(),
        )),
    );
}

fn insert_splash(app: &mut App, tui: &mut tui::Tui) {
    let session = test_thread_session(ThreadId::new(), app.config.cwd.to_path_buf());
    app.insert_history_cell(
        tui,
        Box::new(history_cell::new_session_info(
            &app.config,
            &app.local_settings,
            "gpt-test",
            &session.model,
            &session,
            /*is_first_event*/ false,
            /*tooltip_override*/ None,
            /*auth_plan*/ None,
            /*show_fast_status*/ false,
        )),
    );
}

fn drain_history(app: &mut App, tui: &mut tui::Tui, events: &mut UnboundedReceiver<AppEvent>) {
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            app.insert_history_cell(tui, cell);
        }
    }
}

#[tokio::test]
async fn startup_warnings_preserve_stream_repair_and_backtrack_selection() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget.handle_thread_session(test_thread_session(
        ThreadId::new(),
        app.config.cwd.to_path_buf(),
    ));
    app.transcript_cells = vec![Arc::new(UserHistoryCell {
        message: "Selected prompt".into(),
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: Vec::new(),
        spoken: false,
    })];
    app.transcript_reflow.mark_resize_requested_during_stream();
    insert_warnings(&mut app, &mut tui, &["First warning"]);
    assert!(app.transcript_reflow.take_stream_finish_reflow_needed());
    // Recreate the insertion case with a backtracking overlay already open.
    app.transcript_cells.remove(0);
    app.backtrack.overlay_preview_active = true;
    app.open_transcript_overlay(&mut tui);
    app.apply_backtrack_selection_internal(/*nth_user_message*/ 0);
    insert_warnings(&mut app, &mut tui, &["Second warning"]);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 12,
    );
    let mut buffer = Buffer::empty(area);
    let Some(Overlay::Transcript(overlay)) = app.overlay.as_mut() else {
        panic!("expected transcript")
    };
    overlay.render(area, &mut buffer);
    let highlighted = area
        .positions()
        .filter(|position| {
            buffer[*position]
                .style()
                .add_modifier
                .contains(ratatui::style::Modifier::REVERSED)
        })
        .map(|position| buffer[position].symbol())
        .collect::<String>();
    assert!(highlighted.contains("Selected prompt"), "{highlighted}");
    Ok(())
}

#[tokio::test]
async fn mcp_startup_summary_counts_servers_and_sign_in_subset() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget
        .set_mcp_startup_expected_servers(["alpha", "beta", "gamma"].map(str::to_owned));
    insert_splash(&mut app, &mut tui);
    let sign_in = Some(McpServerStartupFailureReason::ReauthenticationRequired);
    for (name, failure_reason) in [("alpha", sign_in), ("beta", sign_in), ("gamma", None)] {
        app.chat_widget.handle_server_notification(
            ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
                thread_id: None,
                name: name.into(),
                status: McpServerStartupState::Failed,
                error: Some(format!("MCP {name}: connection unavailable")),
                failure_reason,
            }),
            /*replay_kind*/ None,
        );
    }
    drain_history(&mut app, &mut tui, &mut events);
    assert_eq!(app.transcript_cells.len(), 2);
    let warnings = &app.transcript_cells[1];
    assert_eq!(history_cell::warning_count(&app.transcript_cells), 3);
    assert_eq!(
        history_cell::warning_entries(&app.transcript_cells)
            .into_iter()
            .map(|entry| (entry.source, entry.details.contains("Sign-in required.")))
            .collect::<Vec<_>>(),
        vec![
            ("MCP · alpha".into(), true),
            ("MCP · beta".into(), true),
            ("MCP · gamma".into(), false)
        ],
    );
    insta::assert_snapshot!(
        "mcp_startup_details",
        lines_to_single_string(&warnings.transcript_lines(/*width*/ 40))
    );
    insert_warnings(&mut app, &mut tui, &["Skill manifest is invalid."]);
    assert_eq!(history_cell::warning_count(&app.transcript_cells), 4);
    Ok(())
}

#[tokio::test]
async fn startup_warnings_wait_for_splash_and_coalesce_with_full_details() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let first = "Skill manifest is invalid.";
    insert_warnings(&mut app, &mut tui, &[first]);
    let pending = &app.transcript_cells[0];
    assert!(pending.display_lines(/*width*/ 80).is_empty());

    insert_splash(&mut app, &mut tui);
    insert_warnings(&mut app, &mut tui, &[first, "MCP alpha failed to start."]);

    assert_eq!(app.transcript_cells.len(), 2);
    let header = app.transcript_cells[0].as_any();
    assert!(header.is::<history_cell::SessionInfoCell>());
    let display = app.transcript_cells[1].display_lines(/*width*/ 80);
    let transcript = app.transcript_cells[1].transcript_lines(/*width*/ 80);
    insta::assert_snapshot!(format!("display:\n{}\n\ntranscript:\n{}", lines_to_single_string(&display), lines_to_single_string(&transcript)), @"
    display:


    transcript:
    ⚠ Skill manifest is invalid.
    ⚠ MCP alpha failed to start.
    ");
    assert_eq!(history_cell::warning_count(&app.transcript_cells), 2);
    Ok(())
}

#[tokio::test]
async fn startup_skill_load_order_preserves_runtime_error_recurrence() -> Result<()> {
    for background_first in [false, true] {
        let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let mut server = start_config_write_test_app_server(&app).await?;
        let cwd = app.config.cwd.to_path_buf();
        let response = SkillsListResponse {
            data: vec![codex_app_server_protocol::SkillsListEntry {
                cwd: cwd.clone(),
                skills: vec![],
                errors: vec![SkillErrorInfo {
                    path: PathBuf::from("bad/SKILL.md"),
                    message: "invalid description".into(),
                }],
            }],
        };
        // Session attachment and the background startup request may finish in either order.
        for background in [background_first, !background_first] {
            if background {
                app.handle_event(
                    &mut tui,
                    &mut server,
                    AppEvent::SkillsListLoaded {
                        cwd: cwd.clone(),
                        result: Ok(response.clone()),
                    },
                )
                .await?;
            } else {
                app.handle_skills_list_result(Ok(response.clone()), "initial session refresh");
            }
        }
        drain_history(&mut app, &mut tui, &mut events);
        assert_eq!(app.transcript_cells.len(), 1);
        assert!(app.transcript_cells[0].as_any().is::<StartupWarningsCell>());
        app.handle_skills_list_result(Ok(SkillsListResponse { data: vec![] }), "clean refresh");
        app.handle_skills_list_result(Ok(response), "runtime refresh");
        drain_history(&mut app, &mut tui, &mut events);
        assert_eq!(app.transcript_cells.len(), 3);
        let runtime = app.transcript_cells[1..]
            .iter()
            .flat_map(|cell| cell.transcript_lines(/*width*/ 120))
            .collect::<Vec<_>>();
        insta::allow_duplicates! {
            insta::assert_snapshot!(lines_to_single_string(&runtime), @"
            ⚠ Skipped loading 1 skill(s) due to invalid SKILL.md files.
            ⚠ bad/SKILL.md: invalid description
            ");
        }
        server.shutdown().await?;
    }
    Ok(())
}
