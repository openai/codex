//! Exercises real app-server side chats, including their policy, titles, and restored transcripts.

#![cfg(test)]

use super::*;
use crate::app::session_lifecycle::ThreadAttachPresentation;
use crate::app_event::AgentsOverviewAction;
use crate::app_server_session::AppServerSessionEvent;
use crate::app_server_session::SideChatStorage;
use crate::sidebar_chooser::side_chat_label;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use core_test_support::responses;
use pretty_assertions::assert_eq;

/// Captures one real model request and waits until its turn can be inherited by a fork.
pub(super) async fn ask(
    server: &mut AppServerSession,
    model: &wiremock::MockServer,
    thread: ThreadId,
    text: &str,
) -> Result<responses::ResponsesRequest> {
    let mock = responses::mount_sse_once(
        model,
        responses::sse(vec![
            responses::ev_assistant_message("reply", "Answer"),
            responses::ev_completed("done"),
        ]),
    )
    .await;
    let request_id = server.next_request_id();
    let _: TurnStartResponse = server
        .request_handle()
        .request_typed(ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.to_string(),
                input: vec![UserInput::Text {
                    text: text.to_owned(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;
    loop {
        let event = server.next_event().await.expect("connected app-server");
        if let AppServerEvent::ServerNotification(n) = event.event
            && matches!(n.as_ref(), ServerNotification::TurnCompleted(t) if t.thread_id == thread.to_string())
        {
            return Ok(mock.single_request());
        }
    }
}

#[tokio::test]
async fn temporary_side_chat_sends_policy_on_its_first_request() -> Result<()> {
    let mut app = make_test_app().await;
    let model = responses::start_mock_server().await;
    app_test_support::MockResponsesConfig::new(&model.uri()).write(&app.config.codex_home)?;
    app.config = app
        .rebuild_config_for_cwd(app.config.cwd.to_path_buf())
        .await?;
    let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let started = server.start_thread(&app.config).await?;
    let parent = started.session.thread_id;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.replace_chat_widget_with_app_server_thread(
        &mut tui,
        started,
        ThreadAttachPresentation::SessionLineage,
        /*initial_user_message*/ None,
    )
    .await?;
    ask(&mut server, &model, parent, "Parent question").await?;

    let fork = server
        .sidebar_fork(
            app.local_settings.clone(),
            app.side_fork_config(),
            /*workspace_roots*/ None,
            parent,
            /*profile*/ None,
            crate::app_server_session::SideChatStorage::Temporary,
        )
        .await?;
    fork.boundary.expect("side-chat context injection");
    let child = ask(
        &mut server,
        &model,
        fork.thread.session.thread_id,
        "Side question",
    )
    .await?;
    let policy = child
        .message_input_texts("developer")
        .into_iter()
        .find(|text| text.contains("<side_conversation>"))
        .expect("first child request must contain the side-chat developer policy");
    assert!(policy.contains("Scheduled updates"));
    assert!(policy.contains("main conversation without calling create_worktree"));
    assert!(
        child
            .message_input_texts("user")
            .iter()
            .any(|text| text.starts_with("Side conversation boundary."))
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saved_side_chat_title_uses_only_its_first_direct_input() -> Result<()> {
    let question = "Why is CI failing on Windows?";
    let pixel = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
    let image = crate::chatwidget::UserMessage {
        remote_image_urls: vec![pixel.into()],
        .."".into()
    };
    let mut labels = Vec::new();
    for message in [question.into(), image] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        let model = responses::start_mock_server().await;
        app_test_support::MockResponsesConfig::new(&model.uri())
            .enable_feature(Feature::TuiWorkspace)
            .write(&app.config.codex_home)?;
        app.config = app
            .rebuild_config_for_cwd(app.config.cwd.to_path_buf())
            .await?;
        let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
        let started = server.start_thread(&app.config).await?;
        let parent = started.session.thread_id;
        ask(&mut server, &model, parent, "Prepare a release").await?;
        server
            .thread_set_name(parent, "Release plan".into())
            .await?;
        app.enqueue_primary_thread_session(started.session, started.turns)
            .await?;
        enable_side_retention(&mut app)?;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        tui.set_owned_screen(/*owned*/ true)?;
        let title_prompt = crate::app::thread_title::thread_title_prompt(question);
        responses::mount_response_handler(&model, move |request| {
            let answer = if request.message_input_texts("user").contains(&title_prompt) {
                r#"{"title":"Windows CI failure"}"#
            } else {
                "Answer"
            };
            responses::sse_response(responses::sse(vec![
                responses::ev_assistant_message("reply", answer),
                responses::ev_completed("done"),
            ]))
        })
        .await;
        Box::pin(app.handle_start_side(
            &mut tui,
            &mut server,
            parent,
            Some(message),
            crate::app_event::SideStartMode::New,
        ))
        .await?;

        time::timeout(Duration::from_secs(15), async {
            let mut child_finished = false;
            while !child_finished || !app.pending_thread_titles.is_empty() {
                tokio::select! {
                    Some(event) = events.recv() => {
                        boxed_future!(app.handle_event(&mut tui, &mut server, event)).await?;
                    }
                    Some(event) = server.next_event() => {
                        if let AppServerEvent::ServerNotification(n) = &event.event
                            && let ServerNotification::TurnCompleted(turn) = n.as_ref()
                            && app.side_thread_for_parent(parent)
                                .is_some_and(|id| id.to_string() == turn.thread_id)
                        {
                            child_finished = true;
                        }
                        app.handle_backend_event(&server, event).await;
                    }
                }
            }
            Ok::<_, color_eyre::Report>(())
        })
        .await??;
        let saved = server
            .list_saved_side_chats(parent, app.config.codex_home.as_path())
            .await?;
        let [child] = saved.as_slice() else {
            panic!("expected one saved side chat")
        };
        assert!(!server.side_chat_fork_names.contains_key(&child.id));
        labels.push(side_chat_label(child).unwrap().to_owned());
        let parent = server.thread_read(parent, /*include_turns*/ false).await?;
        assert_eq!(parent.name.as_deref(), Some("Release plan"));
        server.shutdown().await?;
    }
    insta::assert_snapshot!(labels.join("\n"), @r"
    Windows CI failure
    [Image]
    ");
    Ok(())
}

async fn saved_chat_parent(
    app: &mut App,
    tui: &crate::tui::Tui,
) -> Result<(AppServerSession, wiremock::MockServer, ThreadId)> {
    let model = responses::start_mock_server().await;
    app_test_support::MockResponsesConfig::new(&model.uri())
        .enable_feature(Feature::TuiWorkspace)
        .write(&app.config.codex_home)?;
    app.config = app
        .rebuild_config_for_cwd(app.config.cwd.to_path_buf())
        .await?;
    app.local_settings = crate::local_settings::LocalSettings::for_tui(&app.config, tui);
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let parent = server.start_thread(&app.config).await?.session.thread_id;
    ask(&mut server, &model, parent, "Main-only prompt").await?;
    server.thread_set_name(parent, "Main plan".into()).await?;
    Ok((server, model, parent))
}

async fn fork_saved_chat(
    app: &App,
    server: &mut AppServerSession,
    parent: ThreadId,
) -> Result<ThreadId> {
    let fork = server
        .sidebar_fork(
            app.local_settings.clone(),
            app.config.clone(),
            /*workspace_roots*/ None,
            parent,
            /*profile*/ None,
            SideChatStorage::Attached,
        )
        .await?;
    fork.boundary.expect("saved side chat");
    Ok(fork.thread.session.thread_id)
}

#[tokio::test]
async fn pending_scrollback_switch_keeps_pasted_input_in_the_source_draft() -> Result<()> {
    let (mut app, mut events, mut ops) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let (mut server, _model, parent) = Box::pin(saved_chat_parent(&mut app, &tui)).await?;
    let child = fork_saved_chat(&app, &mut server, parent).await?;
    app.active_thread_id = Some(parent);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(parent, app.config.cwd.to_path_buf()));
    while ops.try_recv().is_ok() {}

    let draft = "Check the build";
    let cycle = TuiEvent::Key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::CONTROL));
    let enter = TuiEvent::Key(KeyCode::Enter.into());
    for event in [cycle, TuiEvent::Paste(draft.into()), enter.clone()] {
        boxed_future!(app.handle_tui_event(&mut tui, &mut server, event)).await?;
    }
    assert_eq!(app.chat_widget.composer_text_with_pending(), draft);
    let size = Size::new(/*width*/ 48, /*height*/ 12);
    let discovering = side_frame(&mut app, &mut tui, size)?;

    // Apply the list, but hold the attachment completion until after Esc.
    while app.sidebar().unwrap().discovery.request().is_some() {
        let event = events.recv().await.expect("saved chat list");
        boxed_future!(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    assert_eq!(
        app.sidebar()
            .unwrap()
            .opening
            .as_ref()
            .unwrap()
            .target
            .child,
        child
    );
    for key in [KeyCode::Enter, KeyCode::Tab] {
        boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key.into())))
            .await?;
    }
    assert_eq!(app.chat_widget.composer_text_with_pending(), draft);
    assert!(
        !std::iter::from_fn(|| ops.try_recv().ok()).any(|op| matches!(op, Op::UserTurn { .. }))
    );
    let attaching = side_frame(&mut app, &mut tui, size)?;
    assert_eq!(discovering, attaching);
    insta::assert_snapshot!(normalize_snapshot_paths(attaching.trim()), @"
    › Check the build

      gpt-test default · /tmp/project
      Switching chats… · esc to cancel
    ");

    boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into())))
        .await?;
    loop {
        let event = app
            .saved_openings
            .next_event(&mut events)
            .await
            .expect("prepared chat");
        let prepared = matches!(event, AppEvent::SavedChatPrepared { .. });
        boxed_future!(app.handle_event(&mut tui, &mut server, event)).await?;
        if prepared {
            break;
        }
    }
    assert_eq!(app.current_displayed_thread_id(), Some(parent));
    assert_eq!(app.chat_widget.composer_text_with_pending(), draft);
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, enter)).await?;
    assert!(matches!(ops.try_recv()?, Op::UserTurn { .. }));
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn scrollback_cycles_saved_side_chats_after_resume() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let (mut server, model, parent) = Box::pin(saved_chat_parent(&mut app, &tui)).await?;
    let mut children = Vec::new();
    let chats = [
        ("Build checks", "First side prompt"),
        ("Release notes", "Second side prompt"),
    ];
    for (title, prompt) in chats {
        let child = fork_saved_chat(&app, &mut server, parent).await?;
        ask(&mut server, &model, child, prompt).await?;
        server.thread_set_name(child, title.into()).await?;
        children.push(child);
    }
    server.shutdown().await?;

    let cycle = TuiEvent::Key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::CONTROL));
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let resumed = server
        .resume_thread(
            &app.local_settings,
            app.config.clone(),
            parent,
            crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
        )
        .await?;
    app.enqueue_primary_thread_session(resumed.session, resumed.turns)
        .await?;
    drain_app_events(&mut app, &mut tui, &mut server, &mut events).await?;

    let unarchive = || {
        AppServerEvent::ServerNotification(Box::new(ServerNotification::ThreadUnarchived(
            codex_app_server_protocol::ThreadUnarchivedNotification {
                thread_id: children[0].to_string(),
            },
        )))
    };

    // Join the initial list. A change on a parked backend retires that request.
    app.refresh_saved_chats(&server, parent);
    let stale = app.sidebar().unwrap().discovery.request().unwrap();
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, cycle.clone())).await?;
    assert_eq!(app.sidebar().unwrap().discovery.request(), Some(stale));
    assert_eq!(app.current_displayed_thread_id(), Some(parent));
    app.handle_polled_backend_event(
        &server,
        crate::backend_id::BackendId::new(),
        &mut Default::default(),
        AppServerSessionEvent {
            backend_id: server.backend_id(),
            event: unarchive(),
        },
    )
    .await;
    assert_eq!(app.sidebar().unwrap().discovery.request(), None);

    // On the displayed backend the same change retries the cycle. Esc cancels it.
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, cycle.clone())).await?;
    let stale = app.sidebar().unwrap().discovery.request().unwrap();
    app.handle_app_server_event(&server, unarchive()).await;
    assert_ne!(app.sidebar().unwrap().discovery.request(), Some(stale));
    assert!(matches!(
        app.sidebar().unwrap().discovery,
        crate::sidebar_chooser::Discovery::Loading { cycle: true, .. }
    ));
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into())))
        .await?;
    while app.sidebar().unwrap().discovery.request().is_some() {
        let event = events
            .recv()
            .await
            .expect("replacement saved side chat list");
        boxed_future!(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    assert_eq!(app.current_displayed_thread_id(), Some(parent));
    assert!(app.sidebar().unwrap().opening.is_none());

    // A new fork from another client must enter the next cycle without a notification.
    let mut other_client = server.request_view();
    let empty = fork_saved_chat(&app, &mut other_client, parent).await?;
    let mut footers = Vec::new();
    let draft = "Main draft";
    for (expected, details) in children
        .into_iter()
        .zip(chats.map(Some))
        .chain([(empty, None)])
    {
        let origin = app.current_displayed_thread_id();
        boxed_future!(app.handle_tui_event(&mut tui, &mut server, cycle.clone())).await?;
        assert_eq!(app.current_displayed_thread_id(), origin);
        if origin == Some(parent) {
            boxed_future!(app.handle_tui_event(
                &mut tui,
                &mut server,
                TuiEvent::Paste(draft.into())
            ))
            .await?;
        }
        while app.current_displayed_thread_id() != Some(expected) {
            let event = time::timeout(
                Duration::from_secs(15),
                app.saved_openings.next_event(&mut events),
            )
            .await?
            .expect("saved side chat response");
            boxed_future!(app.handle_event(&mut tui, &mut server, event)).await?;
        }
        drain_app_events(&mut app, &mut tui, &mut server, &mut events).await?;
        assert!(app.chat_widget.composer_text_with_pending().is_empty());
        if let Some((title, prompt)) = details {
            assert_eq!(app.chat_widget.thread_name().as_deref(), Some(title));
            let transcript = app
                .transcript_cells
                .iter()
                .flat_map(|cell| cell.display_lines(/*width*/ 100))
                .map(|row| row.to_string())
                .collect::<String>();
            assert!(transcript.contains(prompt) && !transcript.contains("Main-only prompt"));
            let footer = render_bottom_popup(&app.chat_widget, /*width*/ 120);
            let (_, side_footer) = footer.split_once("Side ").expect("side chat footer");
            footers.push(side_footer.lines().next().unwrap().trim().to_string());
        }
    }
    // An explicit rename supersedes the inherited title even if the text matches.
    assert_eq!(
        server.side_chat_fork_names[&empty].as_deref(),
        Some("Main plan")
    );
    other_client
        .thread_set_name(empty, "Main plan".into())
        .await?;
    time::timeout(Duration::from_secs(15), async {
        while server.side_chat_fork_names.contains_key(&empty) {
            server.next_event().await.expect("rename notification");
        }
    })
    .await?;
    boxed_future!(app.handle_tui_event(&mut tui, &mut server, cycle.clone())).await?;
    assert_eq!(app.current_displayed_thread_id(), Some(parent));
    drain_app_events(&mut app, &mut tui, &mut server, &mut events).await?;
    assert_eq!(app.chat_widget.composer_text_with_pending(), draft);
    assert!(!tui.is_alt_screen_active());
    insta::assert_snapshot!(footers.join("\n"), @"
    1/3 from main thread · ⌃/ to cycle · ⌃c to close
    2/3 from main thread · ⌃/ to cycle · ⌃c to close
    ");
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn overview_delete_removes_only_verified_saved_side_chats() -> Result<()> {
    let mut app = make_test_app().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.pause_events();
    let (mut server, _model, parent) = Box::pin(saved_chat_parent(&mut app, &tui)).await?;
    let saved = fork_saved_chat(&app, &mut server, parent).await?;
    let delete = || AppEvent::RunAgentsOverviewAction {
        thread_id: parent,
        action: AgentsOverviewAction::Delete,
    };
    for attach_to_source in [true, false] {
        let other: codex_app_server_protocol::ThreadForkResponse = server
            .request_handle()
            .request_typed(ClientRequest::ThreadFork {
                request_id: server.next_request_id(),
                params: codex_app_server_protocol::ThreadForkParams {
                    thread_id: parent.to_string(),
                    attach_to_source,
                    exclude_turns: true,
                    ..Default::default()
                },
            })
            .await?;
        let other = ThreadId::from_string(&other.thread.id)?;
        Box::pin(app.handle_event(&mut tui, &mut server, delete())).await?;
        for id in [parent, saved, other] {
            server.thread_read(id, /*include_turns*/ false).await?;
        }
        server.thread_delete(other).await?;
    }

    Box::pin(app.handle_event(&mut tui, &mut server, delete())).await?;
    for id in [parent, saved] {
        let error = server
            .thread_read(id, /*include_turns*/ false)
            .await
            .expect_err("deleted thread");
        assert!(format!("{error:#}").contains(&format!("thread not loaded: {id}")));
    }
    server.shutdown().await?;
    Ok(())
}
