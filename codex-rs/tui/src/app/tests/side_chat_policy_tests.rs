//! Covers the model request sent by a real TUI side-chat fork after a completed parent turn.

#![cfg(test)]

use super::*;
use crate::app::session_lifecycle::ThreadAttachPresentation;
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
