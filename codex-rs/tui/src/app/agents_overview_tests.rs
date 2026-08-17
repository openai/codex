use crate::app::agents_overview_view::AgentsOverviewGroup;
use crate::app::test_support::make_test_app;
use crate::bottom_pane::BottomPaneView;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use crate::test_support::PathBufExt;
use crate::test_support::test_path_buf;
use crate::test_support::test_path_display;
use codex_app_server_protocol::ThreadActiveFlag;
use codex_app_server_protocol::ThreadStatus;
use codex_protocol::ThreadId;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

#[tokio::test]
async fn shared_agents_dashboard_is_rendered() {
    assert_eq!(
        AgentsOverviewGroup::for_status(&ThreadStatus::SystemError),
        AgentsOverviewGroup::NeedsYou
    );
    let mut app = make_test_app().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let started = app_server
        .start_thread(&app.config)
        .await
        .expect("start example thread");
    let mut thread = app_server
        .thread_read(started.session.thread_id, /*include_turns*/ false)
        .await
        .expect("read example thread");
    thread.name = Some("Example task".to_string());
    thread.preview = "Inspect the current project".to_string();
    thread.cwd = test_path_buf("/tmp/project").abs();
    thread.status = ThreadStatus::Active {
        active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
    };
    app.primary_thread_id = Some(started.session.thread_id);
    let mut view = app.agents_overview_view(vec![thread.clone()], /*selected_thread_id*/ None);
    let state = &app.agents_overview.view_state;
    assert!(!state.lock().unwrap().status_grouping);
    view.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(state.lock().unwrap().status_grouping);
    app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    assert!(state.lock().unwrap().status_grouping);
    view.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    app.chat_widget.show_bottom_pane_view(Box::new(view));

    insta::with_settings!({snapshot_path => "../snapshots"}, {
        insta::assert_snapshot!(
            "agents_overview",
            render_bottom_popup(&app.chat_widget, /*width*/ 96)
                .replace(&test_path_display("/tmp/project"), "/tmp/project")
        );
    });

    let threads = (0..20)
        .map(|index| {
            let mut candidate = thread.clone();
            if index != 0 {
                candidate.id = ThreadId::new().to_string();
                candidate.status = ThreadStatus::Idle;
            }
            candidate.name = (index != 0).then(|| format!("Task {index}"));
            candidate.updated_at = index;
            candidate.cwd = if index == 0 {
                test_path_buf("/tmp/project-selected").abs()
            } else {
                test_path_buf(&format!("/tmp/project-{}", index % 3)).abs()
            };
            candidate
        })
        .collect();
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let view = app.agents_overview_view(threads, /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 96);
    assert!(rendered.lines().any(
        |line| line.contains("› ● Inspect the current project  current")
            && line.contains("Needs input")
    ));

    app.transcript_cells.push(std::sync::Arc::new(
        crate::history_cell::PlainHistoryCell::new(vec![ratatui::text::Line::from(
            "Previous conversation",
        )]),
    ));
    let mut tui = crate::tui::test_support::make_test_tui().expect("test terminal");
    let screen_size = tui.terminal.last_known_screen_size;
    app.render_chat_widget_frame(&mut tui, screen_size)
        .expect("render full-screen dashboard");
    assert_eq!(tui.terminal.viewport_area.height, screen_size.height);
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.render_chat_widget_frame(&mut tui, screen_size)
        .expect("restore conversation after closing dashboard");
    assert!(tui.terminal.viewport_area.height < screen_size.height);
    assert!(app.last_rendered_history_tail.is_some());

    app_server.shutdown().await.expect("shutdown app server");
}
