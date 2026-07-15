use super::*;
use crate::app::safety_buffering::SafetyBufferedRetry;
use crate::app::session_lifecycle::ThreadAttachPresentation;
use crate::chatwidget::UserMessage;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ModelSafetyBufferingUpdatedNotification;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::StreamingSseServer;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tokio::sync::oneshot;

const CURRENT_MODEL: &str = "gpt-5.2";
const FASTER_MODEL: &str = "gpt-5.4";
const MODEL_PROVIDER_ID: &str = "safety-retry-test";
const PREVIOUS_PROMPT: &str = "Establish context";
const RETRY_PROMPT: &str = "Handle the safety-buffered request";
const COMMITTED_STEER: &str = "Keep the accepted steer";
const UNSENT_DRAFT: &str = "Keep this unsent draft";

fn response_chunks(response_id: &str) -> Vec<StreamingSseChunk> {
    [
        ev_response_created(response_id),
        ev_assistant_message(&format!("message-{response_id}"), "done"),
        ev_completed(response_id),
    ]
    .into_iter()
    .map(|event| StreamingSseChunk {
        gate: None,
        body: responses::sse(vec![event]),
    })
    .collect()
}

fn gated_response_chunks(response_id: &str) -> (Vec<StreamingSseChunk>, oneshot::Sender<()>) {
    let (release_tx, release_rx) = oneshot::channel();
    (
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![ev_response_created(response_id)]),
            },
            StreamingSseChunk {
                gate: Some(release_rx),
                body: responses::sse(vec![ev_completed(response_id)]),
            },
        ],
        release_tx,
    )
}

fn next_user_turn_event(
    app_event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> AppCommand {
    while let Ok(event) = app_event_rx.try_recv() {
        if let AppEvent::CodexOp(turn @ AppCommand::UserTurn { .. }) = event {
            return turn;
        }
    }
    panic!("expected UserTurn app event");
}

fn submit_prompt(app: &mut App, prompt: &str) {
    app.chat_widget.apply_external_edit(prompt.to_string());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
}

fn drain_active_thread_events(app: &mut App) {
    while let Some(event) = app
        .active_thread_rx
        .as_mut()
        .and_then(|receiver| receiver.try_recv().ok())
    {
        app.handle_thread_event_now(event);
    }
}

async fn next_turn_started(
    app: &mut App,
    app_server: &mut AppServerSession,
    thread_id: ThreadId,
) -> String {
    loop {
        let event = tokio::time::timeout(
            std::time::Duration::from_secs(/*secs*/ 5),
            app_server.next_event(),
        )
        .await
        .expect("app-server should emit a turn/start event")
        .expect("app-server event stream should remain open");
        let started_turn_id = match &event {
            AppServerEvent::ServerNotification(ServerNotification::TurnStarted(notification))
                if notification.thread_id == thread_id.to_string() =>
            {
                Some(notification.turn.id.clone())
            }
            _ => None,
        };
        app.handle_app_server_event(app_server, event).await;
        drain_active_thread_events(app);
        if let Some(turn_id) = started_turn_id {
            return turn_id;
        }
    }
}

async fn wait_for_turn_completed(
    app: &mut App,
    app_server: &mut AppServerSession,
    thread_id: ThreadId,
) {
    loop {
        let event = tokio::time::timeout(
            std::time::Duration::from_secs(/*secs*/ 5),
            app_server.next_event(),
        )
        .await
        .expect("app-server should emit a turn/completed event")
        .expect("app-server event stream should remain open");
        let completed = matches!(
            &event,
            AppServerEvent::ServerNotification(ServerNotification::TurnCompleted(notification))
                if notification.thread_id == thread_id.to_string()
        );
        app.handle_app_server_event(app_server, event).await;
        drain_active_thread_events(app);
        if completed {
            return;
        }
    }
}

async fn drive_until_request_count(
    app: &mut App,
    app_server: &mut AppServerSession,
    server: &StreamingSseServer,
    expected_request_count: usize,
) {
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(/*secs*/ 5));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            () = server.wait_for_request_count(expected_request_count) => return,
            event = app_server.next_event() => {
                let event = event.expect("app-server event stream should remain open");
                app.handle_app_server_event(app_server, event).await;
                drain_active_thread_events(app);
            }
            () = &mut timeout => {
                panic!("expected {expected_request_count} Responses API requests");
            }
        }
    }
}

fn user_input_texts(body: &Value) -> Vec<String> {
    body.get("input")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter(|item| item.get("role").and_then(Value::as_str) == Some("user"))
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten()
        .filter(|span| span.get("type").and_then(Value::as_str) == Some("input_text"))
        .filter_map(|span| span.get("text").and_then(Value::as_str).map(str::to_owned))
        .collect()
}

fn user_message_count(thread: &Thread, prompt: &str) -> usize {
    thread
        .turns
        .iter()
        .flat_map(|turn| &turn.items)
        .filter_map(|item| match item {
            ThreadItem::UserMessage { content, .. } => Some(content),
            _ => None,
        })
        .flatten()
        .filter(|item| matches!(item, AppServerUserInput::Text { text, .. } if text == prompt))
        .count()
}

async fn run_safety_retry(
    previous_prompt: Option<&str>,
    failing_draft: Option<&str>,
    committed_steer: Option<&str>,
) -> Result<()> {
    let (active_chunks, release_active_response) = gated_response_chunks("active-response");
    let mut release_active_response = Some(release_active_response);
    let (steered_chunks, release_steered_response) = gated_response_chunks("steered-response");
    let mut response_sequences = Vec::new();
    if previous_prompt.is_some() {
        response_sequences.push(response_chunks("previous-response"));
    }
    response_sequences.push(active_chunks);
    if committed_steer.is_some() {
        response_sequences.push(steered_chunks);
    }
    response_sequences.push(response_chunks("retry-response"));
    let expected_request_count = response_sequences.len();
    let (server, _completions) = start_streaming_sse_server(response_sequences).await;

    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
model = "{CURRENT_MODEL}"
model_provider = "{MODEL_PROVIDER_ID}"

[model_providers.{MODEL_PROVIDER_ID}]
name = "Safety retry test"
base_url = "{}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#,
            server.uri()
        ),
    )?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite_home = codex_home.path().to_path_buf();
    app.config.model = Some(CURRENT_MODEL.to_string());
    app.config.model_provider_id = MODEL_PROVIDER_ID.to_string();
    app.config.model_provider = ModelProviderInfo {
        name: "Safety retry test".to_string(),
        base_url: Some(format!("{}/v1", server.uri())),
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        ..ModelProviderInfo::default()
    };

    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let started = app_server.start_thread(&app.config).await?;
    let source_thread_id = started.session.thread_id;
    app.replace_chat_widget_with_app_server_thread(
        &mut tui,
        &mut app_server,
        started,
        ThreadAttachPresentation::SessionLineage,
        /*initial_user_message*/ None,
    )
    .await?;
    while app_event_rx.try_recv().is_ok() {}

    if let Some(previous_prompt) = previous_prompt {
        submit_prompt(&mut app, previous_prompt);
        let previous_turn = next_user_turn_event(&mut app_event_rx);
        app.submit_thread_op(&mut app_server, source_thread_id, previous_turn)
            .await?;
        wait_for_turn_completed(&mut app, &mut app_server, source_thread_id).await;
    }

    submit_prompt(&mut app, RETRY_PROMPT);
    let active_turn = next_user_turn_event(&mut app_event_rx);
    app.submit_thread_op(&mut app_server, source_thread_id, active_turn.clone())
        .await?;
    let active_turn_id = next_turn_started(&mut app, &mut app_server, source_thread_id).await;
    drive_until_request_count(
        &mut app,
        &mut app_server,
        &server,
        usize::from(previous_prompt.is_some()) + 1,
    )
    .await;

    if let Some(committed_steer) = committed_steer {
        submit_prompt(&mut app, committed_steer);
        let steer = next_user_turn_event(&mut app_event_rx);
        app.submit_thread_op(&mut app_server, source_thread_id, steer)
            .await?;
        let _ = release_active_response
            .take()
            .expect("active response should still be gated")
            .send(());
        drive_until_request_count(
            &mut app,
            &mut app_server,
            &server,
            usize::from(previous_prompt.is_some()) + 2,
        )
        .await;
        let source = app_server
            .thread_read(source_thread_id, /*include_turns*/ true)
            .await?;
        assert_eq!(user_message_count(&source, committed_steer), 1);
    }

    app.handle_app_server_event(
        &app_server,
        AppServerEvent::ServerNotification(ServerNotification::ModelSafetyBufferingUpdated(
            ModelSafetyBufferingUpdatedNotification {
                thread_id: source_thread_id.to_string(),
                turn_id: active_turn_id.clone(),
                model: CURRENT_MODEL.to_string(),
                use_cases: Vec::new(),
                reasons: Vec::new(),
                show_buffering_ui: true,
                faster_model: Some(FASTER_MODEL.to_string()),
            },
        )),
    )
    .await;
    drain_active_thread_events(&mut app);
    assert!(
        app.chat_widget
            .can_retry_safety_buffered_turn(&active_turn_id)
    );
    if let Some(draft) = failing_draft {
        app.chat_widget.apply_external_edit(draft.to_string());
        let source = app_server
            .thread_read(source_thread_id, /*include_turns*/ true)
            .await?;
        std::fs::remove_file(
            source
                .path
                .expect("source thread should have a rollout path"),
        )?;
    }

    let primary_thread_id = ThreadId::new();
    app.primary_thread_id = Some(primary_thread_id);
    Box::pin(app.retry_safety_buffered_turn(
        &mut tui,
        &mut app_server,
        SafetyBufferedRetry {
            thread_id: source_thread_id,
            turn_id: active_turn_id.clone(),
            model: FASTER_MODEL.to_string(),
            turn: active_turn.clone(),
            prompt: UserMessage::from(RETRY_PROMPT),
        },
    ))
    .await;
    assert_eq!(app.primary_thread_id, Some(primary_thread_id));
    assert_eq!(app.active_thread_id, Some(source_thread_id));
    assert_eq!(app.chat_widget.thread_id(), Some(source_thread_id));
    app.primary_thread_id = Some(source_thread_id);

    Box::pin(app.retry_safety_buffered_turn(
        &mut tui,
        &mut app_server,
        SafetyBufferedRetry {
            thread_id: source_thread_id,
            turn_id: active_turn_id,
            model: FASTER_MODEL.to_string(),
            turn: active_turn,
            prompt: UserMessage::from(RETRY_PROMPT),
        },
    ))
    .await;

    if let Some(draft) = failing_draft {
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            format!("{RETRY_PROMPT}\n{draft}")
        );
        assert_eq!(app.chat_widget.thread_id(), Some(source_thread_id));
        if let Some(release_active_response) = release_active_response.take() {
            let _ = release_active_response.send(());
        }
        let _ = release_steered_response.send(());
        app_server.shutdown().await?;
        server.shutdown().await;
        return Ok(());
    }

    drive_until_request_count(&mut app, &mut app_server, &server, expected_request_count).await;
    let mut replayed_history = String::new();
    while let Ok(event) = app_event_rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            replayed_history.push_str(&lines_to_single_string(
                &cell.transcript_lines(/*width*/ 80),
            ));
        }
    }
    assert!(!replayed_history.contains("Conversation interrupted"));

    let retry_thread_id = app.chat_widget.thread_id().expect("retry thread id");
    let source = app_server
        .thread_read(source_thread_id, /*include_turns*/ true)
        .await?;
    let retry = app_server
        .thread_read(retry_thread_id, /*include_turns*/ true)
        .await?;
    assert_ne!(retry_thread_id, source_thread_id);
    assert_eq!(
        source.turns.last().map(|turn| &turn.status),
        Some(&TurnStatus::Interrupted)
    );
    let expected_forked_from_id = previous_prompt.map(|_| source_thread_id.to_string());
    assert_eq!(
        retry.forked_from_id.as_deref(),
        expected_forked_from_id.as_deref()
    );
    let expected_retry_prompt = match committed_steer {
        Some(committed_steer) => format!("{RETRY_PROMPT}\n{committed_steer}"),
        None => RETRY_PROMPT.to_string(),
    };
    assert_eq!(user_message_count(&source, RETRY_PROMPT), 1);
    assert_eq!(user_message_count(&retry, &expected_retry_prompt), 1);
    if let Some(committed_steer) = committed_steer {
        assert_eq!(user_message_count(&source, committed_steer), 1);
    }
    if let Some(previous_prompt) = previous_prompt {
        assert_eq!(user_message_count(&source, previous_prompt), 1);
        assert_eq!(user_message_count(&retry, previous_prompt), 1);
    }

    let request_bodies = server
        .requests()
        .await
        .iter()
        .map(|request| serde_json::from_slice::<Value>(request))
        .collect::<serde_json::Result<Vec<_>>>()?;
    let retry_request = request_bodies
        .last()
        .expect("retry should issue a Responses API request");
    assert_eq!(retry_request["model"].as_str(), Some(FASTER_MODEL));
    assert_eq!(retry_request["reasoning"]["effort"].as_str(), Some("low"));
    let relevant_prompts = user_input_texts(retry_request)
        .into_iter()
        .filter(|text| {
            text == PREVIOUS_PROMPT
                || text == RETRY_PROMPT
                || committed_steer.is_some_and(|steer| text == steer)
        })
        .collect::<Vec<_>>();
    let mut expected_prompts = match previous_prompt {
        Some(previous_prompt) => vec![previous_prompt.to_string(), RETRY_PROMPT.to_string()],
        None => vec![RETRY_PROMPT.to_string()],
    };
    expected_prompts.extend(committed_steer.map(str::to_string));
    assert_eq!(relevant_prompts, expected_prompts);

    if let Some(release_active_response) = release_active_response.take() {
        let _ = release_active_response.send(());
    }
    let _ = release_steered_response.send(());
    app_server.shutdown().await?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn safety_retry_forks_after_the_previous_turn_and_uses_faster_settings() -> Result<()> {
    run_safety_retry(
        Some(PREVIOUS_PROMPT),
        /*failing_draft*/ None,
        /*committed_steer*/ None,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn safety_retry_preserves_a_committed_steer_from_the_interrupted_turn() -> Result<()> {
    run_safety_retry(
        Some(PREVIOUS_PROMPT),
        /*failing_draft*/ None,
        Some(COMMITTED_STEER),
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn safety_retry_starts_new_thread_for_first_turn_without_duplicating_prompt() -> Result<()> {
    run_safety_retry(
        /*previous_prompt*/ None, /*failing_draft*/ None, /*committed_steer*/ None,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn safety_retry_branch_failure_preserves_unsent_draft() -> Result<()> {
    run_safety_retry(
        Some(PREVIOUS_PROMPT),
        Some(UNSENT_DRAFT),
        /*committed_steer*/ None,
    )
    .await
}
