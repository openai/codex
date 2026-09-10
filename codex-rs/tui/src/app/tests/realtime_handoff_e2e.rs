//! Real Core-to-app-server-to-TUI handoff coverage without an audio device.

use super::*;
use crate::chatwidget::UserMessage;
use app_test_support::MockResponsesConfig;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ServerNotification;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_model_provider_info::ModelProviderInfo;
use core_test_support::responses;
use core_test_support::responses::WebSocketConnectionConfig;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use serde_json::json;
use std::time::Duration;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delegated_core_events_keep_private_output_hidden_and_deliver_final_speech() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));

    let mut commentary =
        responses::ev_assistant_message("private-commentary", "PRIVATE COMMENTARY");
    commentary["item"]["phase"] = json!("commentary");
    let mut final_answer = responses::ev_assistant_message(
        "public-final",
        "[ANALYSIS] is the marker you asked about.",
    );
    final_answer["item"]["phase"] = json!("final_answer");
    let model_server = create_mock_responses_server_sequence_unchecked(vec![
        responses::sse(vec![
            responses::ev_response_created("delegated-response"),
            responses::ev_reasoning_item("private-reasoning", &["PRIVATE REASONING"], &[]),
            commentary,
            final_answer,
            responses::ev_completed("delegated-response"),
        ]),
        responses::sse(vec![
            responses::ev_response_created("typed-response"),
            responses::ev_assistant_message("typed-final", "Typed answer stays visible."),
            responses::ev_completed("typed-response"),
        ]),
    ])
    .await;
    Mock::given(method("POST"))
        .and(path("/v1/live"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Location", "/v1/live/rtc_tui_handoff")
                .set_body_string("v=answer\r\n"),
        )
        .mount(&model_server)
        .await;
    let realtime_server =
        responses::start_websocket_server_with_headers(vec![WebSocketConnectionConfig {
            requests: vec![
                vec![
                    json!({
                        "type": "session.started",
                        "session": { "id": "sess_tui_handoff", "instructions": "backend prompt" }
                    }),
                    json!({
                        "type": "delegation.created",
                        "offset_ms": 100,
                        "item": {
                            "id": "delegation_tui",
                            "type": "delegation",
                            "target": "client",
                            "content": [{"type": "input_text", "text": "explain the marker"}]
                        }
                    }),
                ],
                vec![],
                vec![],
                vec![],
            ],
            response_headers: Vec::new(),
            accept_delay: None,
            close_after_requests: false,
        }])
        .await;

    let (mut app, mut app_events, mut ops) = make_test_app_with_channels().await;
    let codex_home = tempfile::tempdir()?;
    MockResponsesConfig::new(&model_server.uri())
        .with_root_config(&format!(
            "experimental_realtime_ws_base_url = {:?}\nexperimental_realtime_webrtc_call_base_url = {:?}",
            realtime_server.uri(),
            format!("{}/v1", model_server.uri()),
        ))
        .with_extra_config("[realtime]\nversion = \"v3\"\ntype = \"conversational\"")
        .write(codex_home.path())?;
    codex_login::login_with_api_key(
        codex_home.path(),
        "sk-test-key",
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite = codex_state::SqliteConfig::new_for_testing(codex_home.path().abs());
    app.config.cli_auth_credentials_store_mode = AuthCredentialsStoreMode::File;
    app.config.model = Some("mock-model".to_string());
    app.config.model_provider_id = "mock_provider".to_string();
    app.config.model_provider = ModelProviderInfo {
        name: "Mock provider for test".to_string(),
        base_url: Some(format!("{}/v1", model_server.uri())),
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        ..ModelProviderInfo::default()
    };
    app.config.experimental_realtime_ws_base_url = Some(realtime_server.uri().to_string());
    app.config.experimental_realtime_webrtc_call_base_url =
        Some(format!("{}/v1", model_server.uri()));

    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let started = app_server.start_thread(&app.config).await?;
    let thread_id = started.session.thread_id;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, thread_id);
    while ops.try_recv().is_ok() {}
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app_server
        .thread_realtime_start(
            thread_id,
            "v=offer\r\n".to_string(),
            /*model*/ None,
            /*voice*/ None,
        )
        .await?;
    // The sideband fixture sends its delegation after the first outbound request.
    app_server
        .thread_realtime_append_speech(thread_id, "fixture prompt".to_string())
        .await?;

    let mut rendered = Vec::new();
    let speech = timeout(Duration::from_secs(15), async {
        loop {
            tokio::select! {
                event = app_server.next_event() => {
                    app.handle_app_server_event(
                        &app_server,
                        event.expect("embedded app-server should stay connected"),
                    ).await;
                    app.drain_active_thread_events(&mut tui).await?;
                }
                event = app_events.recv() => {
                    let event = event.expect("TUI event stream should stay open");
                    if let AppEvent::InsertHistoryCell(cell) = &event {
                        rendered.extend(cell.display_lines(/*width*/ 80).into_iter().map(|line| line.to_string()));
                    }
                    app.handle_event(&mut tui, &mut app_server, event).await?;
                }
                op = ops.recv() => {
                    let op = op.expect("TUI command stream should stay open");
                    if matches!(op, Op::RealtimeConversationSpeech { .. }) {
                        break Ok::<_, color_eyre::Report>(op);
                    }
                    app.handle_event(&mut tui, &mut app_server, AppEvent::CodexOp(op)).await?;
                }
            }
        }
    })
    .await??;
    assert!(
        matches!(&speech, Op::RealtimeConversationSpeech { text, .. } if text.as_str() == "[ANALYSIS] is the marker you asked about."),
        "unexpected speech: {speech:?}"
    );
    app.handle_event(&mut tui, &mut app_server, AppEvent::CodexOp(speech))
        .await?;
    assert_eq!(
        timeout(
            Duration::from_secs(5),
            realtime_server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 1),
        )
        .await?
        .body_json(),
        json!({
            "type": "session.context.append",
            "content": [{
                "type": "input_text",
                "text": "[ANALYSIS] is the marker you asked about."
            }],
            "channel": "speakable"
        })
    );
    let rendered = rendered.join("\n");
    assert!(!rendered.contains("PRIVATE REASONING"), "{rendered}");
    assert!(!rendered.contains("PRIVATE COMMENTARY"), "{rendered}");
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 24,
    );
    let mut buffer = Buffer::empty(area);
    app.chat_widget.render(area, &mut buffer);
    let visible = buffer
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(!visible.contains("PRIVATE REASONING"), "{visible}");
    assert!(!visible.contains("PRIVATE COMMENTARY"), "{visible}");
    assert!(
        ops.try_recv().is_err(),
        "a final answer must be sent only once"
    );

    // A typed request on the same live thread must follow the normal visible path.
    app.chat_widget
        .restore_user_message_to_composer(UserMessage::from("typed follow-up"));
    app.chat_widget
        .handle_key_event(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
    let typed_op = next_user_turn_op(&mut ops);
    app.handle_event(&mut tui, &mut app_server, AppEvent::CodexOp(typed_op))
        .await?;
    let mut typed_rendered = Vec::new();
    timeout(Duration::from_secs(10), async {
        loop {
            tokio::select! {
                event = app_server.next_event() => {
                    let event = event.expect("embedded app-server should stay connected");
                    let completed = matches!(&event, AppServerEvent::ServerNotification(notification)
                        if matches!(notification.as_ref(), ServerNotification::TurnCompleted(_)));
                    app.handle_app_server_event(&app_server, event).await;
                    app.drain_active_thread_events(&mut tui).await?;
                    if completed {
                        break Ok::<_, color_eyre::Report>(());
                    }
                }
                event = app_events.recv() => {
                    let event = event.expect("TUI event stream should stay open");
                    if let AppEvent::InsertHistoryCell(cell) = &event {
                        typed_rendered.extend(cell.display_lines(/*width*/ 80).into_iter().map(|line| line.to_string()));
                    }
                    app.handle_event(&mut tui, &mut app_server, event).await?;
                }
                op = ops.recv() => {
                    let op = op.expect("TUI command stream should stay open");
                    assert!(!matches!(op, Op::RealtimeConversationSpeech { .. }), "typed answer must not be spoken");
                    app.handle_event(&mut tui, &mut app_server, AppEvent::CodexOp(op)).await?;
                }
            }
        }
    })
    .await??;
    while let Ok(event) = app_events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = &event {
            typed_rendered.extend(
                cell.display_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string()),
            );
        }
        app.handle_event(&mut tui, &mut app_server, event).await?;
    }
    assert!(
        typed_rendered
            .join("\n")
            .contains("Typed answer stays visible."),
        "typed final must appear in history: {typed_rendered:?}"
    );
    assert!(ops.try_recv().is_err(), "typed answer must not be spoken");
    realtime_server.shutdown().await;
    app_server.shutdown().await?;
    Ok(())
}
