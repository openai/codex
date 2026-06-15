use super::*;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

#[test]
fn startup_waiting_gate_is_only_for_fresh_or_exit_session_selection() {
    assert_eq!(
        App::should_wait_for_initial_session(&SessionSelection::StartFresh),
        true
    );
    assert_eq!(
        App::should_wait_for_initial_session(&SessionSelection::Exit),
        true
    );
    assert_eq!(
        App::should_wait_for_initial_session(&SessionSelection::Resume(
            crate::resume_picker::SessionTarget {
                path: Some(PathBuf::from("/tmp/restore")),
                thread_id: ThreadId::new(),
            }
        )),
        false
    );
    assert_eq!(
        App::should_wait_for_initial_session(&SessionSelection::Fork(
            crate::resume_picker::SessionTarget {
                path: Some(PathBuf::from("/tmp/fork")),
                thread_id: ThreadId::new(),
            }
        )),
        false
    );
}

#[test]
fn startup_paused_goal_prompt_gate_is_only_for_quiet_resume() {
    let resume = SessionSelection::Resume(crate::resume_picker::SessionTarget {
        path: Some(PathBuf::from("/tmp/restore")),
        thread_id: ThreadId::new(),
    });
    let fork = SessionSelection::Fork(crate::resume_picker::SessionTarget {
        path: Some(PathBuf::from("/tmp/fork")),
        thread_id: ThreadId::new(),
    });
    let no_images: Vec<PathBuf> = Vec::new();
    let initial_images = vec![PathBuf::from("/tmp/image.png")];

    assert!(App::should_prompt_for_paused_goal_after_startup_resume(
        &resume, &None, &no_images
    ));
    assert!(!App::should_prompt_for_paused_goal_after_startup_resume(
        &resume,
        &Some("continue from here".to_string()),
        &no_images
    ));
    assert!(!App::should_prompt_for_paused_goal_after_startup_resume(
        &resume,
        &None,
        &initial_images
    ));
    assert!(!App::should_prompt_for_paused_goal_after_startup_resume(
        &SessionSelection::StartFresh,
        &None,
        &no_images
    ));
    assert!(!App::should_prompt_for_paused_goal_after_startup_resume(
        &fork, &None, &no_images
    ));
}

#[test]
fn startup_waiting_gate_holds_active_thread_events_until_primary_thread_configured() {
    let mut wait_for_initial_session =
        App::should_wait_for_initial_session(&SessionSelection::StartFresh);
    assert_eq!(wait_for_initial_session, true);
    assert_eq!(
        App::should_handle_active_thread_events(
            wait_for_initial_session,
            /*has_active_thread_receiver*/ true
        ),
        false
    );

    assert_eq!(
        App::should_stop_waiting_for_initial_session(
            wait_for_initial_session,
            /*primary_thread_id*/ None
        ),
        false
    );
    if App::should_stop_waiting_for_initial_session(wait_for_initial_session, Some(ThreadId::new()))
    {
        wait_for_initial_session = false;
    }
    assert_eq!(wait_for_initial_session, false);

    assert_eq!(
        App::should_handle_active_thread_events(
            wait_for_initial_session,
            /*has_active_thread_receiver*/ true
        ),
        true
    );
}

#[test]
fn startup_waiting_gate_not_applied_for_resume_or_fork_session_selection() {
    let wait_for_resume = App::should_wait_for_initial_session(&SessionSelection::Resume(
        crate::resume_picker::SessionTarget {
            path: Some(PathBuf::from("/tmp/restore")),
            thread_id: ThreadId::new(),
        },
    ));
    assert_eq!(
        App::should_handle_active_thread_events(
            wait_for_resume,
            /*has_active_thread_receiver*/ true
        ),
        true
    );
    let wait_for_fork = App::should_wait_for_initial_session(&SessionSelection::Fork(
        crate::resume_picker::SessionTarget {
            path: Some(PathBuf::from("/tmp/fork")),
            thread_id: ThreadId::new(),
        },
    ));
    assert_eq!(
        App::should_handle_active_thread_events(
            wait_for_fork,
            /*has_active_thread_receiver*/ true
        ),
        true
    );
}

#[tokio::test]
async fn startup_thread_started_submits_queued_startup_input() {
    let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    app.pending_startup_thread_start = true;
    app.chat_widget
        .set_queue_submissions_until_session_configured(/*queue*/ true);
    app.chat_widget
        .apply_external_edit("queued before startup completes".to_string());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(
        app.chat_widget.queued_user_message_texts(),
        vec!["queued before startup completes".to_string()]
    );

    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(
        app.chat_widget.config_ref(),
    ))
    .await
    .expect("embedded app server");
    let thread_id = ThreadId::new();
    app.handle_startup_thread_started(
        &mut app_server,
        Ok(AppServerStartedThread {
            session: test_thread_session(thread_id, test_path_buf("/tmp/project")),
            turns: Vec::new(),
            history_truncated: false,
        }),
    )
    .await
    .expect("startup thread should attach");

    match next_user_turn_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "queued before startup completes".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued startup input submission, got {other:?}"),
    }
}

#[tokio::test]
async fn startup_thread_start_failure_returns_error() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    app.pending_startup_thread_start = true;

    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(
        app.chat_widget.config_ref(),
    ))
    .await
    .expect("embedded app server");
    let err = app
        .handle_startup_thread_started(&mut app_server, Err("boom".to_string()))
        .await
        .expect_err("startup thread failure should exit instead of leaving chat unconfigured");

    assert!(
        err.to_string()
            .contains("Failed to start a fresh session through the app server: boom")
    );
    assert!(!app.pending_startup_thread_start);
    assert_eq!(app.primary_thread_id, None);
}

#[test]
fn stale_startup_thread_started_removes_local_routing_state() -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_stack_size(8 * 1024 * 1024)
        .enable_all()
        .build()?
        .block_on(async {
            let mut app = make_test_app().await;
            let mut app_server =
                crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
            let primary_thread_id = ThreadId::new();
            let stale_thread_id = ThreadId::new();
            app.primary_thread_id = Some(primary_thread_id);
            app.thread_event_channels.insert(
                primary_thread_id,
                ThreadEventChannel::new(THREAD_EVENT_CHANNEL_CAPACITY),
            );
            app.activate_thread_channel(primary_thread_id).await;
            app.thread_event_channels.insert(
                stale_thread_id,
                ThreadEventChannel::new(THREAD_EVENT_CHANNEL_CAPACITY),
            );
            app.agent_navigation.upsert(
                stale_thread_id,
                /*agent_nickname*/ None,
                /*agent_role*/ None,
                /*is_closed*/ false,
            );
            assert!(app.thread_event_channels.contains_key(&stale_thread_id));
            assert!(app.agent_navigation.get(&stale_thread_id).is_some());

            app.handle_startup_thread_started(
                &mut app_server,
                Ok(AppServerStartedThread {
                    session: test_thread_session(stale_thread_id, test_path_buf("/tmp/project")),
                    turns: Vec::new(),
                    history_truncated: false,
                }),
            )
            .await?;

            assert!(!app.thread_event_channels.contains_key(&stale_thread_id));
            assert_eq!(app.agent_navigation.get(&stale_thread_id), None);
            assert_eq!(app.active_thread_id, Some(primary_thread_id));
            Ok(())
        })
}

#[tokio::test]
async fn ignore_same_thread_resume_reports_noop_for_current_thread() {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.chat_widget.handle_thread_session(session.clone());
    app.thread_event_channels.insert(
        thread_id,
        ThreadEventChannel::new_with_session(THREAD_EVENT_CHANNEL_CAPACITY, session, Vec::new()),
    );
    app.activate_thread_channel(thread_id).await;
    while app_event_rx.try_recv().is_ok() {}

    let ignored = app.ignore_same_thread_resume(&crate::resume_picker::SessionTarget {
        path: Some(test_path_buf("/tmp/project")),
        thread_id,
    });

    assert!(ignored);
    let cell = match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected info message after same-thread resume, saw {other:?}"),
    };
    let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 80));
    assert!(rendered.contains(&format!(
        "Already viewing {}.",
        test_path_display("/tmp/project")
    )));
}

#[tokio::test]
async fn ignore_same_thread_resume_allows_reattaching_displayed_inactive_thread() {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.chat_widget.handle_thread_session(session);

    let ignored = app.ignore_same_thread_resume(&crate::resume_picker::SessionTarget {
        path: Some(test_path_buf("/tmp/project")),
        thread_id,
    });

    assert!(!ignored);
    assert!(app.transcript_cells.is_empty());
}
