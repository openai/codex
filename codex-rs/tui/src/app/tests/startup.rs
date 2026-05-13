use super::*;
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
fn startup_tooltip_override_is_only_prepared_for_fresh_or_exit_session_selection() {
    assert!(App::should_prepare_startup_tooltip_override(
        &SessionSelection::StartFresh
    ));
    assert!(App::should_prepare_startup_tooltip_override(
        &SessionSelection::Exit
    ));
    assert!(!App::should_prepare_startup_tooltip_override(
        &SessionSelection::Resume(crate::resume_picker::SessionTarget {
            path: Some(PathBuf::from("/tmp/restore")),
            thread_id: ThreadId::new(),
        })
    ));
    assert!(!App::should_prepare_startup_tooltip_override(
        &SessionSelection::Fork(crate::resume_picker::SessionTarget {
            path: Some(PathBuf::from("/tmp/fork")),
            thread_id: ThreadId::new(),
        })
    ));
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
