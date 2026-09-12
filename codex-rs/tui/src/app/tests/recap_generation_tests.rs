use super::*;
use crate::app::session_lifecycle::ThreadAttachPresentation;
use crate::app::tests::session_lifecycle_requests::recorded_params;
use crate::app::tests::session_lifecycle_requests::start_recording_remote_app_server;
use crate::app_event::RecapTrigger;
use codex_model_provider_info::ModelProviderInfo;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use serde_json::Value;

const MODEL: &str = "gpt-5.2";
const MODEL_PROVIDER_ID: &str = "recap-generation-test";

fn render_chat_widget(app: &App) -> String {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 24,
    );
    let mut buffer = Buffer::empty(area);
    app.chat_widget.render(area, &mut buffer);
    (area.y..area.bottom())
        .map(|y| {
            (area.x..area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn prepare_eligible_recap(app: &mut App, thread_id: ThreadId) {
    app.active_thread_id = Some(thread_id);
    app.transcript_cells
        .push(Arc::new(crate::history_cell::UserHistoryCell {
            message: "Finish the recap implementation".to_string(),
            spoken: false,
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        }));
    let ready_at = Instant::now();
    app.recap.note_focus_lost(ready_at);
    for _ in 0..3 {
        app.recap
            .note_turn_finished(&TurnStatus::Completed, ready_at);
    }
    // Advance the scheduler's clock instead of subtracting from the host's uptime.
    tokio::time::pause();
    tokio::time::advance(recap::RECAP_DELAY).await;
    tokio::time::resume();
}

#[tokio::test]
async fn recap_generation_uses_bounded_structured_request_and_inserts_result() -> Result<()> {
    let chunks = [
        ev_response_created("recap-response"),
        ev_assistant_message(
            "recap-message",
            r#"{"recap":"Finished parsing. Next: run focused tests."}"#,
        ),
        ev_completed("recap-response"),
    ]
    .into_iter()
    .map(|event| StreamingSseChunk {
        gate: None,
        body: responses::sse(vec![event]),
    })
    .collect();
    let (model_server, _completions) = start_streaming_sse_server(vec![chunks]).await;
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
model = "{MODEL}"
model_provider = "{MODEL_PROVIDER_ID}"

[model_providers.{MODEL_PROVIDER_ID}]
name = "Recap generation test"
base_url = "{}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#,
            model_server.uri()
        ),
    )?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite = codex_state::SqliteConfig::new_for_testing(codex_home.path().abs());
    app.config.model = Some(MODEL.to_string());
    app.config.model_provider_id = MODEL_PROVIDER_ID.to_string();
    app.config.model_provider = ModelProviderInfo {
        name: "Recap generation test".to_string(),
        base_url: Some(format!("{}/v1", model_server.uri())),
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        ..ModelProviderInfo::default()
    };

    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let started = app_server.start_thread(&app.config).await?;
    let thread_id = started.session.thread_id;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.replace_chat_widget_with_app_server_thread(
        &mut tui,
        started,
        ThreadAttachPresentation::SessionLineage,
        /*initial_user_message*/ None,
    )
    .await?;
    while app_event_rx.try_recv().is_ok() {}

    prepare_eligible_recap(&mut app, thread_id).await;

    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::CheckRecap { thread_id },
    )
    .await?;
    let started_event = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), app_event_rx.recv())
        .await?
        .expect("recap start event");
    assert!(matches!(started_event, AppEvent::RecapStarted { .. }));
    app.handle_event(&mut tui, &mut app_server, started_event)
        .await?;

    let generated_event = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
        loop {
            tokio::select! {
                event = app_event_rx.recv() => {
                    let event = event.expect("app event stream");
                    if matches!(event, AppEvent::RecapGenerated { .. }) {
                        break Ok::<_, color_eyre::Report>(event);
                    }
                    app.handle_event(&mut tui, &mut app_server, event).await?;
                }
                event = app_server.next_event() => {
                    let event = event.expect("app-server event stream");
                    app.handle_app_server_event(&app_server, event).await;
                }
            }
        }
    })
    .await??;
    app.handle_event(&mut tui, &mut app_server, generated_event)
        .await?;

    let recap = app
        .transcript_cells
        .last()
        .and_then(|cell| {
            cell.as_any()
                .downcast_ref::<crate::history_cell::ThreadRecapHistoryCell>()
        })
        .expect("recap history cell");
    assert_eq!(
        recap
            .raw_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        vec![
            "Conversation recap",
            "Finished parsing. Next: run focused tests.",
        ]
    );

    let model_requests = model_server.requests().await;
    assert_eq!(model_requests.len(), 1);
    let request: Value = serde_json::from_slice(&model_requests[0])?;
    let prompt = request["input"]
        .as_array()
        .and_then(|items| items.iter().rfind(|item| item["role"] == "user"))
        .and_then(|item| item["content"].as_array())
        .and_then(|content| content.iter().find(|item| item["type"] == "input_text"))
        .and_then(|item| item["text"].as_str())
        .expect("recap prompt");
    assert!(
        prompt.contains("Finish the recap implementation"),
        "prompt: {prompt}\nrequest: {request}"
    );
    assert!(prompt.len() <= recap::RECAP_PROMPT_MAX_BYTES);
    assert_eq!(request["text"]["format"]["type"], "json_schema");
    assert_eq!(request["tools"], serde_json::json!([]));

    app_server.shutdown().await?;
    model_server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn manual_recap_works_when_auto_recap_disabled() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    app.local_settings.tui.auto_recap = false;
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);
    app.transcript_cells
        .push(Arc::new(crate::history_cell::UserHistoryCell {
            message: "Summarize this conversation".to_string(),
            spoken: false,
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        }));
    let (mut app_server, _requests, proxy) = start_recording_remote_app_server(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;

    // An automatic request must not occupy the slot needed by a manual request.
    app.request_recap(&app_server, thread_id, RecapTrigger::Automatic);
    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::GenerateRecap { thread_id },
    )
    .await?;
    assert!(
        render_chat_widget(&app).contains("Generating conversation recap…"),
        "manual recap should show progress while generation is in flight"
    );
    let started_event = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), app_event_rx.recv())
        .await?
        .expect("manual recap start event");
    let (request, temporary_thread_id) = match &started_event {
        AppEvent::RecapStarted {
            thread_id: requested_thread_id,
            request_id,
            trigger,
            completed_turn_count,
            turn_revision,
            result: Ok(temporary_thread_id),
            ..
        } if *requested_thread_id == thread_id && *trigger == RecapTrigger::Manual => (
            recap::RecapRequest {
                thread_id: *requested_thread_id,
                request_id: *request_id,
                trigger: *trigger,
                completed_turn_count: *completed_turn_count,
                turn_revision: *turn_revision,
            },
            ThreadId::from_string(temporary_thread_id).expect("temporary recap thread ID"),
        ),
        other => panic!("expected a started manual recap, got {other:?}"),
    };

    app.handle_event(&mut tui, &mut app_server, started_event)
        .await?;

    assert_eq!(app.temporary_structured_requests.len(), 1);
    assert!(
        app.handle_generated_recap(
            request,
            temporary_thread_id,
            Err("temporary failure".to_string()),
        )
        .is_none()
    );
    assert!(
        !render_chat_widget(&app).contains("Generating conversation recap…"),
        "manual recap progress should clear when generation finishes"
    );

    let cell = match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected visible manual recap error, got {other:?}"),
    };
    let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 80));
    assert!(
        rendered.contains("Could not generate a recap. Please try again."),
        "rendered error: {rendered:?}"
    );

    // A completion quarantined during reconnect must not leave the next recap stuck as busy.
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::RemoteAppServerEndpoint::WebSocket {
            websocket_url: "ws://127.0.0.1:1".into(),
            auth_token: None,
        },
    };
    app.request_recap(&app_server, thread_id, RecapTrigger::Manual);
    let stale = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), app_event_rx.recv())
        .await?
        .expect("recap completion before disconnect");
    assert!(matches!(stale, AppEvent::RecapStarted { .. }));
    assert!(app.begin_reconnect());
    app.handle_event(&mut tui, &mut app_server, stale).await?;
    while app_event_rx.try_recv().is_ok() {}
    app.reconnect.offline = false;
    app.request_recap(&app_server, thread_id, RecapTrigger::Manual);
    let next = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), app_event_rx.recv()).await?;
    assert!(
        matches!(next, Some(AppEvent::RecapStarted { .. })),
        "{next:?}"
    );

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn auto_recap_opt_out_blocks_requests_and_cleans_up_pending_start() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    prepare_eligible_recap(&mut app, thread_id).await;
    let (mut app_server, requests, proxy) = start_recording_remote_app_server(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.local_settings.tui.auto_recap = false;
    app.schedule_recap_check(thread_id, Instant::now());
    app.request_recap(&app_server, thread_id, RecapTrigger::Automatic);
    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::CheckRecap { thread_id },
    )
    .await?;
    tokio::task::yield_now().await;

    assert!(app_event_rx.try_recv().is_err());
    assert_eq!(
        recorded_params(&requests, "thread/start"),
        Vec::<Value>::new()
    );
    let rendered = app
        .transcript_cells
        .iter()
        .flat_map(|cell| cell.display_lines(/*width*/ 80))
        .collect::<Vec<_>>();

    app.local_settings.tui.auto_recap = true;
    app.request_recap(&app_server, thread_id, RecapTrigger::Automatic);
    let started_event = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), app_event_rx.recv())
        .await?
        .expect("recap start event");
    assert!(matches!(started_event, AppEvent::RecapStarted { .. }));
    app.local_settings.tui.auto_recap = false;
    app.handle_event(&mut tui, &mut app_server, started_event)
        .await?;
    tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
        while recorded_params(&requests, "thread/unsubscribe").is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await?;

    // Finish the unsubscribe round trip before shutting down the recording proxy.
    app_server
        .thread_loaded_list(codex_app_server_protocol::ThreadLoadedListParams {
            cursor: None,
            limit: None,
        })
        .await?;
    assert_eq!(
        recorded_params(&requests, "turn/start"),
        Vec::<Value>::new()
    );
    assert!(app.temporary_structured_requests.is_empty());
    app_server.shutdown().await?;
    proxy.await??;
    insta::assert_snapshot!(
        "auto_recap_disabled_transcript",
        lines_to_single_string(&rendered)
    );
    Ok(())
}

#[tokio::test]
async fn recap_generation_uses_remote_workspace_cwd() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let remote_cwd = if cfg!(windows) {
        PathBuf::from(r"C:\remote\project")
    } else {
        PathBuf::from("/srv/remote/project")
    };
    let (app_server, requests, proxy) = start_recording_remote_app_server(&app.config).await?;
    let mut app_server = app_server.with_remote_cwd_override(Some(remote_cwd.clone()));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.transcript_cells
        .push(Arc::new(crate::history_cell::UserHistoryCell {
            message: "Finish the recap implementation".to_string(),
            spoken: false,
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        }));

    app.request_recap(&app_server, ThreadId::new(), RecapTrigger::Automatic);
    let started_event = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), app_event_rx.recv())
        .await?
        .expect("recap start event");
    app.handle_event(&mut tui, &mut app_server, started_event)
        .await?;

    let starts = recorded_params(&requests, "thread/start");
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0]["cwd"], remote_cwd.to_string_lossy().into_owned());
    assert_eq!(starts[0]["ephemeral"], true);
    assert_eq!(starts[0]["environments"], serde_json::json!([]));

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn temporary_recap_threads_disable_memories_and_remote_mcp_servers() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        "features.context_management.experimental_mode = true\n[mcp_servers.filesystem]\ncommand = 'true'\n",
    )?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite = codex_state::SqliteConfig::new_for_testing(codex_home.path().abs());
    let (app_server, requests, proxy) = start_recording_remote_app_server(&app.config).await?;

    let config = app.chat_widget.config_ref();
    let options = crate::temporary_structured_request::TemporaryStructuredThreadOptions {
        model: app.chat_widget.current_model().to_string(),
        model_provider: config.model_provider_id.clone(),
        cwd: config.cwd.display().to_string(),
        active_permission_profile: config
            .permissions
            .active_permission_profile()
            .map(|profile| profile.id),
        mcp_server_names: Vec::new(),
    };
    crate::temporary_structured_request::start_temporary_thread(
        &app_server.request_handle(),
        options,
    )
    .await?;

    let starts = recorded_params(&requests, "thread/start");
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0]["config"]["features.memories"], false);
    assert_eq!(starts[0]["config"]["features.context_management"], false);
    assert!(starts[0]["config"].get("features.memory_tool").is_none());
    assert_eq!(
        starts[0]["config"]["mcp_servers"]["filesystem"]["enabled"],
        false
    );

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn recap_check_rejects_a_non_displayed_thread() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let displayed_thread_id = ThreadId::new();
    prepare_eligible_recap(&mut app, displayed_thread_id).await;
    while app_event_rx.try_recv().is_ok() {}
    let (mut app_server, requests, proxy) = start_recording_remote_app_server(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;

    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::CheckRecap {
            thread_id: ThreadId::new(),
        },
    )
    .await?;
    tokio::task::yield_now().await;

    assert!(app_event_rx.try_recv().is_err());
    assert_eq!(
        recorded_params(&requests, "thread/start"),
        Vec::<Value>::new()
    );

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn recap_check_rejects_a_running_turn() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    prepare_eligible_recap(&mut app, thread_id).await;
    app.chat_widget
        .handle_thread_session(test_thread_session(thread_id, app.config.cwd.to_path_buf()));
    app.chat_widget.handle_server_notification(
        turn_started_notification(thread_id, "active-turn"),
        /*replay_kind*/ None,
    );
    while app_event_rx.try_recv().is_ok() {}
    let (mut app_server, requests, proxy) = start_recording_remote_app_server(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;

    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::CheckRecap { thread_id },
    )
    .await?;
    tokio::task::yield_now().await;

    assert!(app_event_rx.try_recv().is_err());
    assert_eq!(
        recorded_params(&requests, "thread/start"),
        Vec::<Value>::new()
    );

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}
