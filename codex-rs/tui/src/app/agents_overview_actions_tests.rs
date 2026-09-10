use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn lifecycle_shortcuts_target_filtered_task_in_any_state() {
    let mut app = make_test_app().await;
    let mut keymap = TuiKeymap::default();
    keymap.agents.hide = Some(KeybindingsSpec::One(KeybindingSpec("f7".into())));
    app.keymap = RuntimeKeymap::from_config(&keymap).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(tx);
    for status in [
        ThreadStatus::Idle,
        ThreadStatus::NotLoaded,
        ThreadStatus::Active {
            active_flags: Vec::new(),
        },
    ] {
        app.agents_overview
            .view_state
            .lock()
            .unwrap()
            .focus_composer();
        let target = ThreadId::new();
        let mut view = app.agents_overview_view(
            vec![
                overview_thread(
                    ThreadId::new(),
                    /*parent_thread_id*/ None,
                    "Other",
                    ThreadStatus::Idle,
                ),
                overview_thread(target, /*parent_thread_id*/ None, "Target", status),
            ],
            Some(target),
        );
        view.handle_key_event(KeyCode::Esc.into());
        view.handle_key_event(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
        for character in "Target".chars() {
            view.handle_key_event(KeyCode::Char(character).into());
        }
        view.handle_key_event(KeyCode::F(7).into());
        assert!(
            matches!(rx.try_recv(), Ok(AppEvent::HideAgentsOverviewThread { thread_id }) if thread_id == target)
        );
        assert!(rx.try_recv().is_err());
        view.handle_key_event(KeyCode::Esc.into());
    }
}

#[tokio::test]
async fn hidden_task_stays_hidden_through_activity_and_seed_until_explicit_resume() -> Result<()> {
    let (mut app, mut rx, _op_rx) = crate::app::tests::make_test_app_with_channels().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let started = app_server.start_thread(&app.config).await?;
    let id = started.session.thread_id;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    let thread = overview_thread(
        id,
        /*parent_thread_id*/ None,
        "Hidden task",
        ThreadStatus::Idle,
    );
    app.agents_overview.threads.insert(id, Some(thread.clone()));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let view = app.agents_overview_view(vec![thread.clone()], Some(id));
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
    let hide = std::iter::from_fn(|| rx.try_recv().ok())
        .find(|event| matches!(event, AppEvent::HideAgentsOverviewThread { .. }))
        .expect("shortcut requests hiding the task");
    Box::pin(app.handle_event(&mut tui, &mut app_server, hide)).await?;
    app.track_agents_overview_notification(&ServerNotification::ThreadStarted(
        ThreadStartedNotification {
            thread: thread.clone(),
        },
    ));
    app.track_agents_overview_notification(&ServerNotification::ThreadStatusChanged(
        codex_app_server_protocol::ThreadStatusChangedNotification {
            thread_id: id.to_string(),
            status: ThreadStatus::Active {
                active_flags: Vec::new(),
            },
        },
    ));
    app.track_agents_overview_notification(&ServerNotification::ThreadClosed(
        ThreadClosedNotification {
            thread_id: id.to_string(),
        },
    ));
    let request_id = Uuid::new_v4();
    app.agents_overview.initialized = false;
    app.agents_overview.request_id = Some(request_id);
    app.apply_agents_overview_thread_refresh(
        &app_server,
        request_id,
        Ok(AgentsOverviewThreadRefresh {
            threads: HashMap::from([(id, Some(thread.clone()))]),
            last_messages: HashMap::new(),
            recent_seed_complete: true,
        }),
    );
    assert_eq!(
        app.agents_overview_view(vec![thread.clone()], /*selected_thread_id*/ None)
            .thread_ids(),
        Vec::<ThreadId>::new()
    );
    assert_eq!(app.primary_thread_id, Some(id));
    app.resume_target_session(
        &mut tui,
        &mut app_server,
        SessionTarget {
            thread_id: id,
            path: None,
            cwd: None,
            history_mode: None,
        },
    )
    .await?;
    assert_eq!(
        app.agents_overview_view(vec![thread], /*selected_thread_id*/ None)
            .thread_ids(),
        vec![id]
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn lifecycle_footer_keeps_custom_chords_with_labels() {
    let mut app = make_test_app().await;
    app.keymap = RuntimeKeymap::from_config(
        &serde_json::from_value(serde_json::json!({
            "agents": { "hide": "f5 f8" }
        }))
        .unwrap(),
    )
    .unwrap();
    let mut view = app.agents_overview_view(
        vec![overview_thread(
            ThreadId::new(),
            /*parent_thread_id*/ None,
            "Task",
            ThreadStatus::Idle,
        )],
        /*selected_thread_id*/ None,
    );
    view.handle_key_event(KeyCode::Esc.into());
    for width in [36, 48, 80] {
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width + 4, /*height*/ 24);
        let mut buffer = ratatui::buffer::Buffer::empty(area);
        view.render(area, &mut buffer);
        let lines = buffer
            .content()
            .chunks(usize::from(area.width))
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert!(
            lines
                .iter()
                .any(|line| line.contains("hide") && line.contains("f5"))
        );
        assert!(
            lines
                .iter()
                .all(|line| unicode_width::UnicodeWidthStr::width(line.as_str())
                    <= usize::from(width))
        );
    }
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    insta::assert_snapshot!(
        "agents_custom_lifecycle_chords",
        render_bottom_popup(&app.chat_widget, /*width*/ 48)
            .replace(&test_path_display("/tmp/project"), "/tmp/project")
    );
}
