use super::*;
use crate::app::safety_buffering::SafetyBufferedRetry;
use crate::app::session_lifecycle::ThreadAttachPresentation;
use crate::chatwidget::UserMessage;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ModelSafetyBufferingUpdatedNotification;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
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
const RETRY_GOAL: &str = "Preserve this goal across the retry";

#[derive(Clone, Copy, PartialEq, Eq)]
enum SafetyRetryScenario {
    Once,
    RetryTwice,
    InterruptedPrevious,
}

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

fn gated_response_chunks(
    response_id: &str,
    completed: Value,
) -> (Vec<StreamingSseChunk>, oneshot::Sender<()>) {
    let (release_tx, release_rx) = oneshot::channel();
    (
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![ev_response_created(response_id)]),
            },
            StreamingSseChunk {
                gate: Some(release_rx),
                body: responses::sse(vec![completed]),
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
    scenario: SafetyRetryScenario,
) -> Result<()> {
    let (active_chunks, release_active_response) = gated_response_chunks(
        "active-response",
        ev_completed_with_tokens("active-response", /*total_tokens*/ 100),
    );
    let mut release_active_response = Some(release_active_response);
    let (steered_chunks, release_steered_response) =
        gated_response_chunks("steered-response", ev_completed("steered-response"));
    let (previous_chunks, release_previous_response) =
        gated_response_chunks("previous-response", ev_completed("previous-response"));
    let (retry_chunks, release_retry_response) =
        gated_response_chunks("retry-response", ev_completed("retry-response"));
    let mut response_sequences = Vec::new();
    if previous_prompt.is_some() {
        if scenario == SafetyRetryScenario::InterruptedPrevious {
            response_sequences.push(previous_chunks);
        } else {
            response_sequences.push(response_chunks("previous-response"));
        }
    }
    response_sequences.push(active_chunks);
    if committed_steer.is_some() {
        response_sequences.push(steered_chunks);
    }
    if scenario == SafetyRetryScenario::RetryTwice {
        response_sequences.push(retry_chunks);
        response_sequences.push(response_chunks("second-retry-response"));
    } else {
        response_sequences.push(response_chunks("retry-response"));
    }
    response_sequences.push(vec![StreamingSseChunk {
        gate: None,
        body: responses::sse(vec![
            ev_response_created("goal-continuation-response"),
            ev_assistant_message("goal-continuation-message", "done"),
            ev_completed_with_tokens("goal-continuation-response", /*total_tokens*/ 1_000),
        ]),
    }]);
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

[features]
goals = true
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
    app.config
        .features
        .enable(Feature::Goals)
        .expect("test config should allow goals");

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
        if scenario == SafetyRetryScenario::InterruptedPrevious {
            let previous_turn_id =
                next_turn_started(&mut app, &mut app_server, source_thread_id).await;
            drive_until_request_count(
                &mut app,
                &mut app_server,
                &server,
                /*expected_request_count*/ 1,
            )
            .await;
            app_server
                .turn_interrupt(source_thread_id, previous_turn_id)
                .await?;
        } else {
            wait_for_turn_completed(&mut app, &mut app_server, source_thread_id).await;
        }
    }

    let state_db = codex_state::StateRuntime::init(
        app.config.sqlite_home.clone(),
        app.config.model_provider_id.clone(),
    )
    .await
    .expect("state db should initialize");
    state_db
        .thread_goals()
        .replace_thread_goal(
            source_thread_id,
            RETRY_GOAL,
            codex_state::ThreadGoalStatus::Active,
            /*token_budget*/ Some(1_000),
        )
        .await
        .expect("source goal should be stored");
    let source_goal_id = state_db
        .thread_goals()
        .get_thread_goal(source_thread_id)
        .await
        .expect("source goal should be readable")
        .expect("source goal")
        .goal_id;
    state_db
        .thread_goals()
        .account_thread_goal_usage(
            source_thread_id,
            /*time_delta_seconds*/ 12,
            /*token_delta*/ 50,
            codex_state::GoalAccountingMode::ActiveOrStopped,
            Some(source_goal_id.as_str()),
        )
        .await
        .expect("source goal usage should be recorded");

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
    while app_event_rx.try_recv().is_ok() {}

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

    let first_retry_thread_id = app.chat_widget.thread_id().expect("first retry thread id");
    if scenario == SafetyRetryScenario::RetryTwice {
        let first_retry_turn_id =
            next_turn_started(&mut app, &mut app_server, first_retry_thread_id).await;
        drive_until_request_count(
            &mut app,
            &mut app_server,
            &server,
            /*expected_request_count*/ 2,
        )
        .await;
        app.handle_app_server_event(
            &app_server,
            AppServerEvent::ServerNotification(ServerNotification::ModelSafetyBufferingUpdated(
                ModelSafetyBufferingUpdatedNotification {
                    thread_id: first_retry_thread_id.to_string(),
                    turn_id: first_retry_turn_id.clone(),
                    model: FASTER_MODEL.to_string(),
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
                .can_retry_safety_buffered_turn(&first_retry_turn_id)
        );
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let second_retry = loop {
            match app_event_rx.try_recv() {
                Ok(AppEvent::RetrySafetyBufferedTurn {
                    thread_id,
                    turn_id,
                    model,
                    turn,
                    prompt,
                }) => {
                    break SafetyBufferedRetry {
                        thread_id,
                        turn_id,
                        model,
                        turn,
                        prompt,
                    };
                }
                Ok(_) => continue,
                Err(err) => panic!("expected second safety-buffering retry event: {err}"),
            }
        };
        let AppCommand::UserTurn { items, .. } = &second_retry.turn else {
            panic!("second safety-buffering retry should retain the user turn");
        };
        assert!(items.iter().any(
            |item| matches!(item, AppServerUserInput::Text { text, .. } if text == RETRY_PROMPT)
        ));
        Box::pin(app.retry_safety_buffered_turn(&mut tui, &mut app_server, second_retry)).await;
    }

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
    assert_eq!(
        replayed_history.contains("Conversation interrupted"),
        scenario == SafetyRetryScenario::InterruptedPrevious
    );
    let forked_history = replayed_history
        .rsplit_once("Thread forked from")
        .expect("safety retry should render fork lineage")
        .1;
    assert_eq!(
        forked_history.matches(RETRY_PROMPT).count(),
        1,
        "the fork should render the retried prompt once: {forked_history}"
    );
    if committed_steer.is_some() {
        let rendered_retry = forked_history
            .lines()
            .skip_while(|line| !line.contains(RETRY_PROMPT))
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!("safety_retry_committed_steer_history", rendered_retry);
    }

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
    assert_eq!(
        retry.forked_from_id.as_deref(),
        Some(
            if scenario == SafetyRetryScenario::RetryTwice {
                first_retry_thread_id
            } else {
                source_thread_id
            }
            .to_string()
        )
        .as_deref()
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

    let source_goal = app_server
        .thread_goal_get(source_thread_id)
        .await?
        .goal
        .expect("source goal");
    let retry_goal = app_server
        .thread_goal_get(retry_thread_id)
        .await?
        .goal
        .expect("retry goal");
    let expected_source_tokens = if committed_steer.is_some() { 150 } else { 50 };
    assert_eq!(source_goal.objective, RETRY_GOAL);
    assert_eq!(source_goal.tokens_used, expected_source_tokens);
    assert_eq!(source_goal.time_used_seconds, 12);
    assert_eq!(retry_goal.objective, RETRY_GOAL);
    assert!(retry_goal.tokens_used >= expected_source_tokens);
    assert!(retry_goal.time_used_seconds >= 12);

    let request_bodies = server
        .requests()
        .await
        .iter()
        .map(|request| serde_json::from_slice::<Value>(request))
        .collect::<serde_json::Result<Vec<_>>>()?;
    let retry_request_index =
        usize::from(previous_prompt.is_some()) + usize::from(committed_steer.is_some()) + 1;
    let retry_request = request_bodies
        .get(retry_request_index)
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
    if previous_prompt.is_none() {
        assert!(
            !user_input_texts(retry_request)
                .iter()
                .any(|text| text.contains("<turn_aborted>")),
            "first-turn safety retry should not inherit an interruption marker"
        );
    }
    let goal_continuation_request_index = if scenario == SafetyRetryScenario::RetryTwice {
        let second_retry_request = request_bodies
            .get(retry_request_index + 1)
            .expect("second retry should issue a Responses API request");
        assert!(
            user_input_texts(second_retry_request)
                .iter()
                .any(|text| text == RETRY_PROMPT),
            "second safety retry should submit the original prompt"
        );
        assert!(
            !user_input_texts(second_retry_request)
                .iter()
                .any(|text| text.contains("<turn_aborted>")),
            "second safety retry should not inherit an interruption marker"
        );
        retry_request_index + 2
    } else {
        retry_request_index + 1
    };
    assert!(
        user_input_texts(&request_bodies[goal_continuation_request_index])
            .iter()
            .any(|text| text.contains(RETRY_GOAL)),
        "inherited goal continuation should resume after the explicit retry"
    );

    if let Some(release_active_response) = release_active_response.take() {
        let _ = release_active_response.send(());
    }
    let _ = release_steered_response.send(());
    let _ = release_previous_response.send(());
    let _ = release_retry_response.send(());
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
        SafetyRetryScenario::Once,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn safety_retry_preserves_a_committed_steer_from_the_interrupted_turn() -> Result<()> {
    run_safety_retry(
        Some(PREVIOUS_PROMPT),
        /*failing_draft*/ None,
        Some(COMMITTED_STEER),
        SafetyRetryScenario::Once,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn safety_retry_forks_first_turn_and_continues_without_duplicating_prompt() -> Result<()> {
    run_safety_retry(
        /*previous_prompt*/ None,
        /*failing_draft*/ None,
        /*committed_steer*/ None,
        SafetyRetryScenario::Once,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn safety_retry_can_retry_a_first_turn_a_second_time() -> Result<()> {
    run_safety_retry(
        /*previous_prompt*/ None,
        /*failing_draft*/ None,
        /*committed_steer*/ None,
        SafetyRetryScenario::RetryTwice,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn safety_retry_replays_older_interruption_notices() -> Result<()> {
    run_safety_retry(
        Some(PREVIOUS_PROMPT),
        /*failing_draft*/ None,
        /*committed_steer*/ None,
        SafetyRetryScenario::InterruptedPrevious,
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn safety_retry_branch_failure_preserves_unsent_draft() -> Result<()> {
    run_safety_retry(
        Some(PREVIOUS_PROMPT),
        Some(UNSENT_DRAFT),
        /*committed_steer*/ None,
        SafetyRetryScenario::Once,
    )
    .await
}
