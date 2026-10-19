//! Real Responses-to-app-server-to-TUI coverage for interleaved assistant output and commands.

use super::*;
use crate::chatwidget::tests::helpers::normalize_completion_timestamps;
use app_test_support::MockResponsesConfig;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_model_provider_info::ModelProviderInfo;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;

const PREAMBLE: &str = "I’ll inspect the relevant streaming path first.";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn command_start_before_message_completion_does_not_duplicate_preamble() -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));

    let (release_completion, completion_gate) = oneshot::channel();
    let (model_server, _completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![
                    responses::ev_response_created("response-1"),
                    responses::ev_message_item_added("message-1", ""),
                    responses::ev_output_text_delta(PREAMBLE),
                    responses::ev_function_call(
                        "command-1",
                        "exec_command",
                        r#"{"cmd":"echo fixture-output","login":false}"#,
                    ),
                ]),
            },
            StreamingSseChunk {
                gate: Some(completion_gate),
                body: responses::sse(vec![responses::ev_assistant_message("message-1", PREAMBLE)]),
            },
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![responses::ev_completed("response-1")]),
            },
        ],
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![
                responses::ev_response_created("response-2"),
                responses::ev_assistant_message("message-2", "Finished."),
                responses::ev_completed("response-2"),
            ]),
        }],
    ])
    .await;

    let (mut app, mut app_events, _ops) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    MockResponsesConfig::new(model_server.uri())
        .with_sandbox_mode("danger-full-access")
        .write(codex_home.path())?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.cwd = codex_home.path().abs();
    app.config.sqlite = codex_state::SqliteConfig::new_for_testing(codex_home.path().abs());
    app.config
        .permissions
        .approval_policy
        .set(codex_protocol::protocol::AskForApproval::Never)?;
    app.config
        .permissions
        .set_permission_profile(codex_protocol::models::PermissionProfile::Disabled)?;
    app.config.model = Some("mock-model".to_string());
    app.config.model_provider_id = "mock_provider".to_string();
    app.config.model_provider = ModelProviderInfo {
        name: "Mock provider for test".to_string(),
        base_url: Some(format!("{}/v1", model_server.uri())),
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        ..ModelProviderInfo::default()
    };

    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let started = app_server.start_thread(&app.config).await?;
    let thread_id = started.session.thread_id;
    app.replace_chat_widget_with_app_server_thread(
        &mut tui,
        started,
        super::super::session_lifecycle::ThreadAttachPresentation::Fresh,
        /*initial_user_message*/ None,
    )
    .await?;
    while app_events.try_recv().is_ok() {}

    app.chat_widget
        .restore_user_message_to_composer("inspect the stream".into());
    app.chat_widget
        .handle_key_event(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
    let turn = std::iter::from_fn(|| app_events.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::CodexOp(turn @ Op::UserTurn { .. }) => Some(turn),
            _ => None,
        })
        .expect("composer submission should emit a user turn");
    app.submit_thread_op(&mut app_server, thread_id, turn)
        .await?;

    let mut release_completion = Some(release_completion);
    let mut completed_command = None;
    let mut observed_order = Vec::new();
    timeout(Duration::from_secs(/*secs*/ 15), async {
        loop {
            tokio::select! {
                event = app_server.next_event() => {
                    let event = event.expect("embedded app-server should stay connected");
                    let turn_completed = matches!(&event.event, AppServerEvent::ServerNotification(notification)
                        if matches!(notification.as_ref(), ServerNotification::TurnCompleted(_)));
                    if let AppServerEvent::ServerNotification(notification) = &event.event {
                        match notification.as_ref() {
                            ServerNotification::AgentMessageDelta(delta) if delta.delta == PREAMBLE => {
                                observed_order.push("agent delta");
                            }
                            ServerNotification::ItemStarted(started)
                                if matches!(&started.item, ThreadItem::CommandExecution { .. }) =>
                            {
                                observed_order.push("command started");
                                release_completion.take().expect("first command start").send(()).expect("completion gate should remain open");
                            }
                            ServerNotification::ItemCompleted(completed)
                                if matches!(&completed.item, ThreadItem::AgentMessage { text, .. } if text == PREAMBLE) =>
                            {
                                observed_order.push("agent completed");
                            }
                            ServerNotification::ItemCompleted(completed) => {
                                if let ThreadItem::CommandExecution {
                                    status,
                                    aggregated_output,
                                    exit_code,
                                    ..
                                } = &completed.item
                                {
                                    completed_command = Some((
                                        status.clone(),
                                        *exit_code,
                                        aggregated_output
                                            .as_deref()
                                            .map(str::trim)
                                            .map(str::to_owned),
                                    ));
                                }
                            }
                            _ => {}
                        }
                    }
                    app.handle_backend_event(&app_server, event).await;
                    app.drain_active_thread_events(&mut tui).await?;
                    if turn_completed {
                        break Ok::<_, color_eyre::Report>(());
                    }
                }
                event = app_events.recv() => {
                    let event = event.expect("TUI event stream should stay open");
                    app.handle_event(&mut tui, &mut app_server, event).await?;
                }
            }
        }
    })
    .await??;
    while let Ok(event) = app_events.try_recv() {
        app.handle_event(&mut tui, &mut app_server, event).await?;
    }

    assert_eq!(
        observed_order,
        vec!["agent delta", "command started", "agent completed"]
    );
    assert_eq!(
        completed_command,
        Some((
            CommandExecutionStatus::Completed,
            Some(0),
            Some("fixture-output".to_string()),
        ))
    );
    for replay in [false, true] {
        if replay {
            app.store_active_thread_receiver().await;
            let (receiver, snapshot) = app
                .activate_thread_for_replay(thread_id)
                .await
                .expect("thread should retain its buffered notifications");
            app.active_thread_rx = Some(receiver);
            app.render_thread_snapshot(
                &mut tui,
                &app_server,
                thread_id,
                snapshot,
                /*resume_restored_queue*/ false,
            )?;
            while let Ok(event) = app_events.try_recv() {
                app.handle_event(&mut tui, &mut app_server, event).await?;
            }
        }
        let transcript_snapshot = app
            .transcript_cells
            .iter()
            // Replay also restores the session header and submitted user prompt.
            .filter(|cell| {
                !cell.as_any().is::<crate::history_cell::SessionInfoCell>()
                    && !cell.as_any().is::<crate::history_cell::UserHistoryCell>()
            })
            .map(|cell| {
                let lines = cell.display_lines(/*width*/ 80);
                if cell.as_any().is::<crate::exec_cell::ExecCell>() {
                    lines.first().map(ToString::to_string).unwrap_or_default()
                } else {
                    normalize_completion_timestamps(
                        cell.as_ref(),
                        lines
                            .into_iter()
                            .map(|line| line.to_string())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        insta::allow_duplicates! {
            insta::assert_snapshot!(transcript_snapshot, @r"
            • I’ll inspect the relevant streaming path first.
            • Ran echo fixture-output
            • Finished.
              Worked for [duration] • [completion time]
            ");
        }
    }

    app_server.shutdown().await?;
    model_server.shutdown().await;
    Ok(())
}
