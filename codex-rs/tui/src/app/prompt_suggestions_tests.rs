//! Tests hidden suggestion forks through the app-server API.

use super::*;
use crate::app::session_lifecycle::ThreadAttachPresentation;
use crate::app::test_support::make_test_app;
use crate::app_event_sender::AppEventSender;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use tokio::sync::mpsc::unbounded_channel;

#[derive(Clone, Copy, PartialEq)]
enum Scenario {
    Success,
    UnchangedSettings,
    CancelStartup,
    Timeout,
}

#[tokio::test]
async fn prompt_suggestion_fork_preserves_request_prefix_and_stays_hidden() -> color_eyre::Result<()>
{
    check_suggestion(Scenario::UnchangedSettings).await?;
    check_suggestion(Scenario::Success).await
}

#[tokio::test]
async fn prompt_suggestion_cancelled_startup_cleans_up_without_a_turn() -> color_eyre::Result<()> {
    check_suggestion(Scenario::CancelStartup).await
}

#[tokio::test]
async fn prompt_suggestion_timeout_interrupts_and_cleans_up() -> color_eyre::Result<()> {
    check_suggestion(Scenario::Timeout).await
}

async fn check_suggestion(scenario: Scenario) -> color_eyre::Result<()> {
    let server = wiremock::MockServer::start().await;
    let parent_response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("parent"),
            responses::ev_assistant_message("answer", "Fixed the timeout."),
            responses::ev_completed("parent"),
        ]),
    )
    .await;
    let codex_home = tempdir()?;
    let provider_id = "suggestion-test";
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            "model = \"gpt-5.2\"\n\
             model_provider = \"{provider_id}\"\n\n\
             [model_providers.{provider_id}]\n\
             name = \"Suggestion test\"\n\
             base_url = \"{}/v1\"\n\
             wire_api = \"responses\"\n\
             request_max_retries = 0\n\
             stream_max_retries = 0\n",
            server.uri()
        ),
    )?;

    let mut app = make_test_app().await;
    let (event_tx, mut event_rx) = unbounded_channel();
    app.local_settings.tui.animations = false;
    app.local_settings.tui.prompt_suggestions = true;
    app.app_event_tx = AppEventSender::new(event_tx);
    app.config = ConfigBuilder::default()
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .codex_home(codex_home.path().to_path_buf())
        .fallback_cwd(Some(dunce::canonicalize(codex_home.path())?))
        .build()
        .await?;

    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let started = app_server.start_thread(&app.config).await?;
    let thread_id = started.session.thread_id;
    app.replace_chat_widget_with_app_server_thread(
        &mut tui,
        started,
        ThreadAttachPresentation::SessionLineage,
        /*initial_user_message*/ None,
    )
    .await?;

    app_server
        .thread_set_name(thread_id, "Timeout fix".into())
        .await?;
    app.chat_widget
        .on_thread_name_updated(thread_id, Some("Timeout fix".into()));
    while event_rx.try_recv().is_ok() {}
    let _: TurnStartResponse = app_server
        .request_handle()
        .request_typed(ClientRequest::TurnStart {
            request_id: RequestId::String("parent-turn".into()),
            params: TurnStartParams {
                thread_id: thread_id.to_string(),
                service_tier: (scenario == Scenario::Success).then(|| Some("priority".into())),
                collaboration_mode: (scenario == Scenario::Success).then(|| {
                    codex_protocol::config_types::CollaborationMode {
                        mode: codex_protocol::config_types::ModeKind::Default,
                        settings: codex_protocol::config_types::Settings {
                            model: "gpt-5.2".into(),
                            reasoning_effort: Some(
                                codex_protocol::openai_models::ReasoningEffort::High,
                            ),
                            developer_instructions: Some("Keep requests concise.".into()),
                        },
                    }
                }),
                input: vec![UserInput::Text {
                    text: "Fix the timeout".into(),
                    text_elements: vec![],
                }],
                ..Default::default()
            },
        })
        .await?;
    let mut fork_response = None;
    let mut hidden_id = None;
    let mut interrupted = false;
    tokio::time::timeout(Duration::from_secs(/*secs*/ 45), async {
        loop {
            tokio::select! {
                Some(event) = event_rx.recv() => {
                    if matches!(&event, AppEvent::GeneratePromptSuggestion(_)) {
                        let mut response = responses::sse_response(responses::sse(vec![
                            responses::ev_response_created("prediction"),
                            responses::ev_assistant_message("suggestion", r#"{"suggestion":"Add a regression test"}"#),
                            responses::ev_completed("prediction"),
                        ]));
                        if scenario == Scenario::Timeout { response = response.set_delay(Duration::from_secs(/*secs*/ 60)); }
                        fork_response = Some(responses::mount_response_once(&server, response).await);
                    }
                    if let AppEvent::PromptSuggestionStarted { result, request } = &event {
                        result.as_ref().expect("fork startup");
                        if scenario == Scenario::CancelStartup { request.cancellation.cancel(); }
                    }
                    if let AppEvent::PromptSuggestionFinished { temporary_thread_id, .. } = &event {
                        hidden_id = Some(*temporary_thread_id);
                    }
                    app.handle_event(&mut tui, &mut app_server, event).await?;

                }
                Some(event) = app_server.next_event() => {
                    if let AppServerEvent::ServerNotification(notification) = &event {
                        if let ServerNotification::TurnCompleted(completed) = notification.as_ref()
                            && completed.thread_id != thread_id.to_string()
                        { interrupted = completed.turn.status == codex_app_server_protocol::TurnStatus::Interrupted; }
                        if let ServerNotification::TurnStarted(started) = notification.as_ref()
                            && started.thread_id != thread_id.to_string() && scenario == Scenario::Timeout
                        {
                            tokio::time::pause();
                            tokio::time::advance(Duration::from_secs(/*secs*/ 31)).await;
                            tokio::time::resume();
                        }
                    }
                    app.handle_app_server_event(&app_server, event).await;
                    app.drain_active_thread_events_until(&mut tui, Instant::now() + Duration::from_secs(/*secs*/ 1)).await?;
                }
            }
            if hidden_id.is_some() && (scenario != Scenario::Timeout || interrupted) { break; }
        }
        color_eyre::eyre::Ok(())
    }).await??;
    let hidden_id = hidden_id.expect("hidden fork");
    assert!(!app.thread_event_channels.contains_key(&hidden_id));
    assert!(app.temporary_structured_requests.is_empty());
    let detached: codex_app_server_protocol::ThreadUnsubscribeResponse = app_server
        .request_handle()
        .request_typed(ClientRequest::ThreadUnsubscribe {
            request_id: RequestId::String("verify-cleanup".into()),
            params: codex_app_server_protocol::ThreadUnsubscribeParams {
                thread_id: hidden_id.to_string(),
            },
        })
        .await?;
    assert!(matches!(
        detached.status,
        codex_app_server_protocol::ThreadUnsubscribeStatus::NotSubscribed
            | codex_app_server_protocol::ThreadUnsubscribeStatus::NotLoaded
    ));
    if matches!(scenario, Scenario::CancelStartup | Scenario::Timeout) {
        if scenario == Scenario::CancelStartup {
            assert!(fork_response.as_ref().unwrap().requests().is_empty());
        }
        assert!(!app.chat_widget.has_prompt_suggestion());
        return Ok(());
    }
    let requests = [
        parent_response.single_request(),
        fork_response
            .expect("generation requested")
            .single_request(),
    ];
    if scenario == Scenario::Success {
        insta::assert_snapshot!(
            "prompt_suggestion_request_history",
            context_snapshot::format_request_history_snapshot(
                "A completed turn is forked for a hidden follow-up suggestion with the parent's request prefix intact.",
                &requests,
                &ContextSnapshotOptions::default().rewrite_known_segments(),
            )
        );
    }
    let parent = requests[0].body_json();
    let fork = requests[1].body_json();
    for field in [
        "model",
        "instructions",
        "tools",
        "reasoning",
        "service_tier",
        "text",
        "prompt_cache_key",
        "stream_options",
    ] {
        assert_eq!(&fork[field], &parent[field], "{field}");
    }
    let parent_input = parent["input"].as_array().expect("parent input");
    let fork_input = fork["input"].as_array().expect("fork input");
    assert_eq!(&fork_input[..parent_input.len()], parent_input.as_slice());
    assert!(app.chat_widget.composer_is_empty());
    assert!(
        crate::chatwidget::tests::helpers::render_bottom_popup(&app.chat_widget, /*width*/ 80)
            .contains("Add a regression test")
    );
    Ok(())
}
