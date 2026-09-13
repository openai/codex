//! Offline editing, uncertain submissions, and stale events never consume local work.

use super::*;
use crate::app_server_session::ThreadParamsMode;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCRequest;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn lost_mutation_reply_preserves_work_without_resubmitting() -> Result<()> {
    let (mut app, mut events, mut ops) = make_test_app_with_channels().await;
    let id = ThreadId::new();
    app.active_thread_id = Some(id);
    app.primary_thread_id = Some(id);
    app.chat_widget
        .handle_thread_session(test_thread_session(id, app.config.cwd.to_path_buf()));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
    app.app_server_target = AppServerTarget::Remote {
        endpoint: endpoint.clone(),
    };
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        serve_reconnect_requests(tokio_tungstenite::accept_async(stream).await?, |request| {
            assert_eq!(request.method, "turn/start");
            std::future::ready(None)
        })
        .await
    });
    let mut session = AppServerSession::new(
        crate::connect_remote_app_server(endpoint).await?,
        ThreadParamsMode::Remote,
    );
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.chat_widget
        .restore_user_message_to_composer("only once".into());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let op = next_user_turn_op(&mut ops);
    app.chat_widget
        .set_queue_autosend_suppressed(/*suppressed*/ true);
    app.chat_widget
        .restore_user_message_to_composer("queued before disconnect".into());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.chat_widget
        .restore_user_message_to_composer("keep this draft".into());
    let overview = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(overview));
    app.chat_widget.show_selection_view(SelectionViewParams {
        items: vec![SelectionItem {
            name: "Stale server action".into(),
            actions: vec![Box::new(|tx| tx.send(AppEvent::CodexOp(Op::Interrupt)))],
            ..Default::default()
        }],
        ..Default::default()
    });
    assert!(matches!(
        app.handle_event(&mut tui, &mut session, AppEvent::CodexOp(op))
            .await?,
        AppRunControl::Continue
    ));
    assert!(app.reconnect.offline);
    assert!(!app.chat_widget.has_active_view());
    for key in [
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('D'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE),
    ] {
        assert!(matches!(
            app.handle_tui_event(&mut tui, &mut session, TuiEvent::Key(key))
                .await?,
            AppRunControl::Continue
        ));
    }
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "keep this draft!"
    );
    assert!(app.chat_widget.has_queued_follow_up_messages());
    app.chat_widget
        .set_queue_autosend_suppressed(/*suppressed*/ false);
    assert!(!app.chat_widget.maybe_send_next_queued_input());
    app.handle_event(&mut tui, &mut session, AppEvent::NewSession { name: None })
        .await?;
    assert_eq!(app.current_displayed_thread_id(), Some(id));
    while let Ok(event) = events.try_recv() {
        app.handle_event(&mut tui, &mut session, event).await?;
    }
    let history = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
        loop {
            if let Ok(text) =
                tokio::fs::read_to_string(app.config.codex_home.join("history.jsonl")).await
                && let Ok(entry) =
                    serde_json::from_str::<codex_message_history::HistoryEntry>(&text)
            {
                break entry;
            }
            tokio::time::sleep(Duration::from_millis(/*millis*/ 10)).await;
        }
    })
    .await?;
    assert_eq!(
        (history.session_id, history.text),
        (id.to_string(), "only once".into())
    );
    assert_snapshot!(
        "offline_draft",
        format!(
            "{}\n{}",
            lines_to_single_string(
                &app.transcript_cells
                    .last()
                    .unwrap()
                    .display_lines(/*width*/ 80)
            ),
            render_bottom_popup(&app.chat_widget, /*width*/ 80)
        )
    );
    let mut keymap = codex_config::types::TuiKeymap::default();
    keymap.chat.edit_queued_message = Some(codex_config::types::KeybindingsSpec::One(
        codex_config::types::KeybindingSpec("ctrl-x up".into()),
    ));
    let runtime =
        RuntimeKeymap::from_config(&keymap).map_err(|error| color_eyre::eyre::eyre!(error))?;
    app.chat_widget.apply_keymap_update(keymap, &runtime);
    app.keymap = runtime;
    for key in [
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    ] {
        app.handle_tui_event(&mut tui, &mut session, TuiEvent::Key(key))
            .await?;
    }
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "queued before disconnect"
    );
    assert!(!app.chat_widget.has_queued_follow_up_messages());
    let paste = "copied text ".repeat(/*n*/ 92);
    app.handle_tui_event(&mut tui, &mut session, TuiEvent::Paste(paste.clone()))
        .await?;
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        format!("queued before disconnect{paste}")
    );
    let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 80);
    let draft = rendered
        .split_once("› ")
        .unwrap()
        .1
        .split("\n\n")
        .next()
        .unwrap();
    assert_snapshot!("offline_expanded_paste", draft);
    assert!(ops.try_recv().is_err());
    for character in ['c', 'C', 'd', 'D'] {
        if character.eq_ignore_ascii_case(&'d') {
            app.handle_tui_event(
                &mut tui,
                &mut session,
                TuiEvent::Key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL)),
            )
            .await?;
        }
        assert!(matches!(
            app.handle_tui_event(
                &mut tui,
                &mut session,
                TuiEvent::Key(KeyEvent::new(
                    KeyCode::Char(character),
                    KeyModifiers::CONTROL
                ))
            )
            .await?,
            AppRunControl::Exit(ExitReason::UserRequested)
        ));
    }
    session.shutdown().await?;
    assert_eq!(server.await??, vec!["initialize", "turn/start"]);
    Ok(())
}

#[tokio::test]
async fn disconnected_command_center_keeps_input_and_blocks_actions() -> Result<()> {
    let (mut app, mut events, _) = make_test_app_with_channels().await;
    let mut session = crate::start_embedded_app_server_for_picker(&app.config).await?;
    assert!(!app.begin_reconnect(), "embedded behavior stays unchanged");
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::resolve_remote_addr("ws://127.0.0.1:9")?,
    };
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    app.chat_widget.handle_paste("task draft".into());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.handle_tui_event(
        &mut tui,
        &mut session,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE)),
    )
    .await?;
    app.handle_tui_event(
        &mut tui,
        &mut session,
        TuiEvent::Paste("search query".into()),
    )
    .await?;
    assert!(app.begin_reconnect());
    for key in [
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE),
    ] {
        app.handle_tui_event(&mut tui, &mut session, TuiEvent::Key(key))
            .await?;
    }
    app.handle_tui_event(&mut tui, &mut session, TuiEvent::Paste("!".into()))
        .await?;
    app.handle_tui_event(
        &mut tui,
        &mut session,
        TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
    )
    .await?;
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok()).any(|event| matches!(
            event,
            AppEvent::NewAgentsOverviewSession { .. }
                | AppEvent::NewAgentsOverviewWorktree { .. }
                | AppEvent::OpenResumePicker
        ))
    );
    assert_snapshot!(
        "offline_command_center",
        render_bottom_popup(&app.chat_widget, /*width*/ 100)
    );
    session.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn remote_disconnect_retires_voice_before_reconnecting() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(thread_id, app.config.cwd.to_path_buf()));
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::resolve_remote_addr("ws://127.0.0.1:9")?,
    };
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, thread_id);
    assert!(app.chat_widget.is_current_realtime_attempt(
        thread_id, /*attempt_id*/ 0, /*input_generation*/ 0
    ));

    assert!(app.begin_reconnect());
    assert_eq!(
        (
            app.reconnect.offline,
            app.chat_widget.is_current_realtime_attempt(
                thread_id, /*attempt_id*/ 0, /*input_generation*/ 0
            ),
        ),
        (true, false)
    );
    assert!(app.begin_reconnect());
    assert!(!app.chat_widget.is_current_realtime_attempt(
        thread_id, /*attempt_id*/ 0, /*input_generation*/ 0
    ));
    Ok(())
}

// Common framing/bootstrap for both transports. Returning None deliberately loses a reply.
pub(super) async fn serve_reconnect_requests<S, F>(
    mut socket: tokio_tungstenite::WebSocketStream<S>,
    mut respond: impl FnMut(JSONRPCRequest) -> F,
) -> Result<Vec<String>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    F: std::future::Future<Output = Option<serde_json::Value>>,
{
    let mut methods = Vec::new();
    while let Some(Ok(frame)) = socket.next().await {
        let Message::Text(text) = frame else { continue };
        let JSONRPCMessage::Request(request) = serde_json::from_str(&text)? else {
            continue;
        };
        methods.push(request.method.clone());
        let request_id = request.id.clone();
        let response = match request.method.as_str() {
            "initialize" => Some(json!({"result": {"userAgent": "reconnect-test/2.0.0"}})),
            "account/read" => {
                Some(json!({"result": {"account": null, "requiresOpenaiAuth": false}}))
            }
            "model/list" => Some(json!({"result": {"data": [], "nextCursor": null}})),
            "collaborationMode/list" => Some(json!({"result": {"data": []}})),
            "configRequirements/read" => Some(json!({"result": {"requirements": null}})),
            _ => respond(request).await,
        };
        let Some(mut response) = response else {
            socket.close(None).await?;
            break;
        };
        response["id"] = json!(request_id);
        socket
            .send(Message::Text(response.to_string().into()))
            .await?;
    }
    Ok(methods)
}
#[tokio::test]
async fn lost_initial_thread_reply_keeps_startup_draft_offline() -> Result<()> {
    for disconnect_first in [false, true] {
        let (mut app, _events, mut ops) = make_test_app_with_channels().await;
        app.pending_startup_thread_start = true;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
        app.app_server_target = AppServerTarget::Remote {
            endpoint: endpoint.clone(),
        };
        app.chat_widget
            .restore_user_message_to_composer("startup draft".into());
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            serve_reconnect_requests(tokio_tungstenite::accept_async(stream).await?, |request| {
                assert_eq!(request.method, "thread/start");
                std::future::ready(None)
            })
            .await
        });
        let mut session = AppServerSession::new(
            crate::connect_remote_app_server(endpoint).await?,
            ThreadParamsMode::Remote,
        );
        let result = crate::app_server_session::start_thread_with_request_handle(
            session.request_handle(),
            &app.local_settings,
            app.config.clone(),
            ThreadParamsMode::Remote,
            /*remote_cwd_override*/ None,
            session.thread_tool_transport(),
        )
        .await;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        if disconnect_first {
            app.begin_reconnect();
        }
        app.handle_event(
            &mut tui,
            &mut session,
            AppEvent::StartupThreadStarted { result },
        )
        .await?;
        assert!(app.reconnect.offline);
        app.handle_tui_event(
            &mut tui,
            &mut session,
            TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        )
        .await?;
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            "startup draft"
        );
        assert!(
            !std::iter::from_fn(|| ops.try_recv().ok()).any(|op| matches!(op, Op::UserTurn { .. }))
        );
        session.shutdown().await?;
        assert_eq!(server.await??, vec!["initialize", "thread/start"]);
    }
    Ok(())
}
