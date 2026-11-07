//! Covers the model request sent by a real TUI side-chat fork after a completed parent turn.

#![cfg(test)]

use super::*;
use crate::app::session_lifecycle::ThreadAttachPresentation;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use core_test_support::responses;

/// Captures one real model request and waits until its turn can be inherited by a fork.
async fn ask(
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
