//! App-to-RPC coverage for a delegated voice answer without audio hardware.

use super::*;
use crate::app::tests::session_lifecycle_requests::RealtimeRequestBehavior;
use crate::app::tests::session_lifecycle_requests::recorded_params;
use crate::app::tests::session_lifecycle_requests::start_recording_realtime_speech_app_server;
use crate::app::tests::session_lifecycle_requests::start_recording_remote_app_server;
use crate::chatwidget::commit_realtime_history_events;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_protocol::models::MessagePhase;
use pretty_assertions::assert_eq;
use std::path::Path;

fn normalize_voice_snapshot_directory(rendered: &str, cwd: &Path) -> String {
    let cwd = cwd.display().to_string();
    let placeholder = "/tmp/project";
    let padded_placeholder = format!(
        "{placeholder}{}",
        " ".repeat(cwd.len().saturating_sub(placeholder.len()))
    );
    rendered.replace(&cwd, &padded_placeholder)
}

#[test]
fn voice_snapshot_directory_keeps_header_width_for_windows_paths() {
    let rendered = "│ directory: C:\\tmp\\project              │";
    assert_eq!(
        normalize_voice_snapshot_directory(rendered, Path::new("C:\\tmp\\project")),
        "│ directory: /tmp/project                │"
    );
}

fn test_agent_message(id: &str, text: &str) -> ThreadItem {
    ThreadItem::AgentMessage {
        id: id.into(),
        text: text.into(),
        phase: Some(MessagePhase::FinalAnswer),
        questions: None,
        memory_citation: None,
        delivery: None,
    }
}

fn test_user_message(id: &str, text: &str) -> ThreadItem {
    ThreadItem::UserMessage {
        id: id.into(),
        client_id: None,
        content: vec![UserInput::Text {
            text: text.into(),
            text_elements: Vec::new(),
        }],
    }
}

fn empty_thread_snapshot(app: &App, thread_id: ThreadId) -> ThreadEventSnapshot {
    ThreadEventSnapshot {
        delegated_turns: Vec::new(),
        session: Some(test_thread_session(thread_id, app.config.cwd.to_path_buf())),
        turns: Vec::new(),
        events: Vec::new(),
        active_reasoning_item: None,
        input_state: None,
    }
}

enum ItemEventKind {
    Started,
    Completed,
}

#[tokio::test]
async fn remote_voice_start_routes_v3_offer_and_effective_preference() -> Result<()> {
    use super::session_lifecycle_requests::HistoryCapabilities;
    for (capabilities, expected_voice) in [
        (HistoryCapabilities::Current, Some("juniper")),
        (HistoryCapabilities::ConfigReadUnsupported(-32601), None),
        (HistoryCapabilities::Current, Some("cove")),
        (HistoryCapabilities::VoiceCatalogCustom, Some("maple")),
        (HistoryCapabilities::VoiceCatalogUnavailable, Some("cove")),
        (HistoryCapabilities::ConfigReadUnknownVoice, None),
    ] {
        check_remote_voice_start(capabilities, expected_voice).await?;
    }
    Ok(())
}

async fn check_remote_voice_start(
    capabilities: super::session_lifecycle_requests::HistoryCapabilities,
    expected_voice: Option<&str>,
) -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let (mut app_server, requests, proxy) =
        super::session_lifecycle_requests::start_recording_app_server_with_realtime_speech(
            &app.config,
            capabilities,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
            crate::app_server_session::ThreadParamsMode::Remote,
            RealtimeRequestBehavior::AcceptStart,
            codex_config::LoaderOverrides::default(),
        )
        .await?;
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(thread_id, app.config.cwd.to_path_buf()));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    Box::pin(app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::PersistRealtimeVoiceSelection {
            voice: codex_protocol::protocol::RealtimeVoice::Juniper,
        },
    ))
    .await?;
    if capabilities == super::session_lifecycle_requests::HistoryCapabilities::Current {
        assert_eq!(
            (
                app.config.realtime.voice,
                app.chat_widget.config_ref().realtime.voice
            ),
            (
                Some(codex_protocol::protocol::RealtimeVoice::Juniper),
                Some(codex_protocol::protocol::RealtimeVoice::Juniper)
            )
        );
    }
    assert_eq!(
        recorded_params(&requests, "config/batchWrite")[0]["edits"],
        serde_json::json!([{"keyPath": "realtime.voice", "value": "juniper", "mergeStrategy": "replace"}]),
    );
    if expected_voice != Some("juniper") {
        std::fs::write(
            app.config.codex_home.join("config.toml"),
            "[realtime]\ntype = \"conversational\"\n",
        )?;
    }
    // A reconnected TUI can retain a different local preference from its server.
    app.config.realtime.voice = Some(codex_protocol::protocol::RealtimeVoice::Maple);
    app.chat_widget
        .on_realtime_voice_saved(codex_protocol::protocol::RealtimeVoice::Maple);
    Box::pin(app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::CodexOp(Op::RealtimeConversationStart {
            thread_id,
            offer_sdp: String::from("v=0\r\n").into(),
        }),
    ))
    .await?;

    let starts = recorded_params(&requests, "thread/realtime/start");
    assert_eq!(starts.len(), 1);
    let start = &starts[0];
    assert_eq!(
        serde_json::json!({
            "threadId": start["threadId"],
            "clientManagedHandoffs": start["clientManagedHandoffs"],
            "includeStartupContext": start["includeStartupContext"],
            "outputModality": start["outputModality"],
            "version": start["version"],
            "voice": start["voice"],
            "transport": start["transport"],
        }),
        serde_json::json!({
            "threadId": thread_id.to_string(),
            "clientManagedHandoffs": true,
            "includeStartupContext": false,
            "outputModality": "audio",
            "version": "v3",
            "voice": expected_voice,
            "transport": {"type": "webrtc", "sdp": "v=0\r\n"},
        })
    );
    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn shutdown_stops_backend_voice_once_through_app_server() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let (mut app_server, requests, proxy) = start_recording_realtime_speech_app_server(
        &app.config,
        RealtimeRequestBehavior::AcceptSpeech,
    )
    .await?;
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(thread_id, app.config.cwd.to_path_buf()));
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, thread_id);

    app.shutdown_current_thread(&mut app_server).await;
    app.shutdown_current_thread(&mut app_server).await;

    assert_eq!(
        recorded_params(&requests, "thread/realtime/stop"),
        vec![serde_json::json!({"threadId": thread_id.to_string()})]
    );
    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn stalled_voice_stop_leaves_time_for_shutdown_unsubscribe() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let (mut app_server, requests, proxy) = start_recording_realtime_speech_app_server(
        &app.config,
        RealtimeRequestBehavior::AcceptSpeechAndStallStop,
    )
    .await?;
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(thread_id, app.config.cwd.to_path_buf()));
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, thread_id);

    tokio::time::timeout(
        crate::app::event_dispatch::SHUTDOWN_FIRST_EXIT_TIMEOUT,
        app.shutdown_current_thread(&mut app_server),
    )
    .await?;

    assert_eq!(
        recorded_params(&requests, "thread/realtime/stop"),
        vec![serde_json::json!({"threadId": thread_id.to_string()})]
    );
    assert_eq!(
        recorded_params(&requests, "thread/unsubscribe"),
        vec![serde_json::json!({"threadId": thread_id.to_string()})]
    );
    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn switching_agent_threads_stops_backend_voice_once_through_app_server() -> Result<()> {
    let (mut app, _events, mut ops) = make_test_app_with_channels().await;
    let (mut app_server, requests, proxy) = start_recording_realtime_speech_app_server(
        &app.config,
        RealtimeRequestBehavior::AcceptSpeech,
    )
    .await?;
    let source = ThreadId::new();
    app.active_thread_id = Some(source);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(source, app.config.cwd.to_path_buf()));
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, source);
    while ops.try_recv().is_ok() {}
    let turn_id = "pending-voice-turn";
    send_item(
        &mut app,
        source,
        turn_id,
        test_user_message(
            "spoken-input",
            "<realtime_delegation><input>question</input></realtime_delegation>",
        ),
        ItemEventKind::Started,
    );
    let answer = test_agent_message("pending-answer", "Answer to preserve after switching.");
    send_item(
        &mut app,
        source,
        turn_id,
        answer.clone(),
        ItemEventKind::Started,
    );
    send_item(
        &mut app,
        source,
        turn_id,
        answer.clone(),
        ItemEventKind::Completed,
    );
    app.handle_thread_event_now(ThreadBufferedEvent::Notification(Box::new(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: source.to_string(),
            turn: Turn {
                id: turn_id.into(),
                items: vec![answer.clone()],
                items_view: TurnItemsView::Summary,
                status: TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            },
        }),
    )));
    let speech = ops.try_recv()?;
    assert!(
        matches!(speech, Op::RealtimeConversationSpeech { .. }),
        "{speech:?}"
    );
    let mut source_channel = ThreadEventChannel::new_with_session(
        THREAD_EVENT_CHANNEL_CAPACITY,
        test_thread_session(source, app.config.cwd.to_path_buf()),
        Vec::new(),
    );
    source_channel.store.lock().await.active = true;
    app.active_thread_rx = source_channel.receiver.take();
    app.thread_event_channels.insert(source, source_channel);
    // Routed to the active channel, but not yet delivered to the old widget.
    app.enqueue_thread_notification(
        source,
        ServerNotification::ThreadRealtimeTranscriptDone(
            codex_app_server_protocol::ThreadRealtimeTranscriptDoneNotification {
                thread_id: source.to_string(),
                role: "user".into(),
                text: "queued before switch".into(),
            },
        ),
    )
    .await?;
    let target = ThreadId::new();
    app.thread_event_channels.insert(
        target,
        ThreadEventChannel::new_with_session(
            THREAD_EVENT_CHANNEL_CAPACITY,
            test_thread_session(target, app.config.cwd.to_path_buf()),
            Vec::new(),
        ),
    );
    let mut tui = crate::tui::test_support::make_test_tui()?;

    Box::pin(app.select_agent_thread(&mut tui, &mut app_server, target)).await?;

    assert_eq!(app.chat_widget.thread_id(), Some(target));
    assert_eq!(
        app.pending_realtime_speech_replay[&source],
        vec![(turn_id.into(), answer)]
    );
    assert_eq!(app.pending_realtime_transcript_replay[&source].len(), 1);
    assert_eq!(
        app.pending_realtime_transcript_replay[&source][0].text,
        "queued before switch"
    );
    assert_eq!(
        recorded_params(&requests, "thread/realtime/stop"),
        vec![serde_json::json!({"threadId": source.to_string()})]
    );
    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn switching_threads_keeps_the_source_voice_partial_only_on_reattach() {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let source = ThreadId::new();
    app.active_thread_id = Some(source);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(source, app.config.cwd.to_path_buf()));
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, source);
    app.chat_widget.handle_server_notification(
        ServerNotification::ThreadRealtimeTranscriptDelta(
            codex_app_server_protocol::ThreadRealtimeTranscriptDeltaNotification {
                thread_id: source.to_string(),
                role: "user".into(),
                delta: "spoken partial".into(),
            },
        ),
        /*replay_kind*/ None,
    );

    let side = ThreadId::new();
    let (side_widget, _, mut side_events, _) = make_chatwidget_manual_with_sender().await;
    app.active_thread_id = Some(side);
    app.replace_chat_widget(side_widget);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(side, app.config.cwd.to_path_buf()));
    assert_eq!(app.pending_realtime_transcript_replay[&source].len(), 1);
    while let Ok(event) = side_events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            assert!(
                cell.transcript_lines(/*width*/ 80)
                    .iter()
                    .all(|line| !line.to_string().contains("spoken partial"))
            );
        }
    }

    let (source_widget, _, mut source_events, _) = make_chatwidget_manual_with_sender().await;
    app.active_thread_id = Some(source);
    app.replace_chat_widget(source_widget);
    app.replay_thread_snapshot(
        empty_thread_snapshot(&app, source),
        /*resume_restored_queue*/ false,
    );
    assert!(!app.pending_realtime_transcript_replay.contains_key(&source));
    app.chat_widget.handle_server_notification(
        ServerNotification::ThreadRealtimeTranscriptDone(
            codex_app_server_protocol::ThreadRealtimeTranscriptDoneNotification {
                thread_id: source.to_string(),
                role: "user".into(),
                text: "spoken partial completed".into(),
            },
        ),
        /*replay_kind*/ None,
    );
    commit_realtime_history_events(&mut app.chat_widget, &mut source_events);
    let rendered = std::iter::from_fn(|| source_events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(
                cell.transcript_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(rendered.matches("spoken partial completed").count(), 1);
    insta::assert_snapshot!(
        "voice_partial_completed_after_thread_switch",
        normalize_voice_snapshot_directory(&rendered, &app.config.cwd)
    );
    while let Ok(event) = side_events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            assert!(
                cell.transcript_lines(/*width*/ 80)
                    .iter()
                    .all(|line| !line.to_string().contains("spoken partial"))
            );
        }
    }
}

#[tokio::test]
async fn queued_voice_caption_after_switch_returns_once_to_its_source_thread() {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let source = ThreadId::new();
    app.active_thread_id = Some(source);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(source, app.config.cwd.to_path_buf()));
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, source);

    // Exercise a completion that arrives only after two switches without a prior delta.
    for _ in 0..2 {
        let (other_widget, _, _, _) = make_chatwidget_manual_with_sender().await;
        app.active_thread_id = Some(ThreadId::new());
        app.replace_chat_widget(other_widget);
        assert!(app.pending_realtime_transcript_replay.contains_key(&source));

        let (source_widget, _, _, _) = make_chatwidget_manual_with_sender().await;
        app.active_thread_id = Some(source);
        app.replace_chat_widget(source_widget);
        app.replay_thread_snapshot(
            empty_thread_snapshot(&app, source),
            /*resume_restored_queue*/ false,
        );
    }

    let other = ThreadId::new();
    let (other_widget, _, mut other_events, _) = make_chatwidget_manual_with_sender().await;
    app.active_thread_id = Some(other);
    app.replace_chat_widget(other_widget);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(other, app.config.cwd.to_path_buf()));

    app.enqueue_thread_notification(
        source,
        ServerNotification::ThreadRealtimeTranscriptDone(
            codex_app_server_protocol::ThreadRealtimeTranscriptDoneNotification {
                thread_id: source.to_string(),
                role: "assistant".into(),
                text: "arrived after two switches".into(),
            },
        ),
    )
    .await
    .unwrap();

    for notification in [
        ServerNotification::ThreadRealtimeTranscriptDelta(
            codex_app_server_protocol::ThreadRealtimeTranscriptDeltaNotification {
                thread_id: source.to_string(),
                role: "user".into(),
                delta: "last ".into(),
            },
        ),
        ServerNotification::ThreadRealtimeTranscriptDelta(
            codex_app_server_protocol::ThreadRealtimeTranscriptDeltaNotification {
                thread_id: source.to_string(),
                role: "user".into(),
                delta: "words".into(),
            },
        ),
        ServerNotification::ThreadRealtimeTranscriptDone(
            codex_app_server_protocol::ThreadRealtimeTranscriptDoneNotification {
                thread_id: source.to_string(),
                role: "user".into(),
                text: "last words".into(),
            },
        ),
    ] {
        app.enqueue_thread_notification(source, notification)
            .await
            .unwrap();
    }
    app.enqueue_thread_notification(
        source,
        ServerNotification::ThreadRealtimeTranscriptDelta(
            codex_app_server_protocol::ThreadRealtimeTranscriptDeltaNotification {
                thread_id: source.to_string(),
                role: "assistant".into(),
                delta: "discarded partial".into(),
            },
        ),
    )
    .await
    .unwrap();
    app.enqueue_thread_notification(
        source,
        ServerNotification::ThreadRealtimeTranscriptDone(
            codex_app_server_protocol::ThreadRealtimeTranscriptDoneNotification {
                thread_id: source.to_string(),
                role: "assistant".into(),
                text: String::new(),
            },
        ),
    )
    .await
    .unwrap();
    assert_eq!(app.pending_realtime_transcript_replay[&source].len(), 2);
    while let Ok(event) = other_events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            assert!(
                cell.transcript_lines(/*width*/ 80)
                    .iter()
                    .all(|line| !line.to_string().contains("last words"))
            );
        }
    }

    let (source_widget, _, mut source_events, _) = make_chatwidget_manual_with_sender().await;
    app.active_thread_id = Some(source);
    app.replace_chat_widget(source_widget);
    app.replay_thread_snapshot(
        empty_thread_snapshot(&app, source),
        /*resume_restored_queue*/ false,
    );
    commit_realtime_history_events(&mut app.chat_widget, &mut source_events);
    let rendered = std::iter::from_fn(|| source_events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(
                cell.transcript_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(rendered.matches("last words").count(), 1);
    assert_eq!(rendered.matches("arrived after two switches").count(), 1);
    assert!(!rendered.contains("last last words"));
}

#[tokio::test]
async fn replay_reconciles_only_matching_voice_captions_one_for_one() {
    let (mut app, _initial_events, _ops) = make_test_app_with_channels().await;
    let source = ThreadId::new();
    let (widget, _, mut events, _) = make_chatwidget_manual_with_sender().await;
    app.active_thread_id = Some(source);
    app.replace_chat_widget(widget);
    let records = [
        ("user", "show <code> & tests"),
        ("user", "show <code> & tests"),
        ("assistant", "spoken answer"),
        ("user", "typed words"),
        ("assistant", "different caption"),
    ]
    .into_iter()
    .map(|(role, text)| crate::chatwidget::RealtimeTranscriptRecord {
        role: role.to_string(),
        text: text.to_string(),
        complete: true,
        before_turn_id: None,
    })
    .collect();
    app.pending_realtime_transcript_replay
        .insert(source, records);
    let voice = Turn {
        id: "voice-turn".into(),
        items: vec![
            test_user_message(
                "voice-user",
                "<realtime_delegation><input>show &lt;code&gt; &amp; tests</input></realtime_delegation>",
            ),
            test_agent_message("voice-answer", "spoken answer"),
        ],
        items_view: TurnItemsView::Summary,
        status: TurnStatus::Completed,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    };
    let typed = Turn {
        id: "typed-turn".into(),
        items: vec![test_user_message("typed-user", "typed words")],
        ..voice.clone()
    };
    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            delegated_turns: vec![voice.id.clone()],
            session: Some(test_thread_session(source, app.config.cwd.to_path_buf())),
            turns: vec![voice, typed],
            events: Vec::new(),
            active_reasoning_item: None,
            input_state: None,
        },
        /*resume_restored_queue*/ false,
    );
    commit_realtime_history_events(&mut app.chat_widget, &mut events);
    let rendered = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(
                cell.transcript_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(rendered.matches("show <code> & tests").count(), 2);
    assert_eq!(rendered.matches("spoken answer").count(), 1);
    assert_eq!(rendered.matches("typed words").count(), 2);
    assert_eq!(rendered.matches("different caption").count(), 1);
    insta::assert_snapshot!(
        "voice_replay_reconciles_matching_captions",
        normalize_voice_snapshot_directory(&rendered, &app.config.cwd)
    );
}

#[tokio::test]
async fn retained_caption_consumes_only_one_matching_answer_fallback_on_reattach() -> Result<()> {
    let (mut app, _initial_events, _ops) = make_test_app_with_channels().await;
    let source = ThreadId::new();
    let (widget, _, mut events, _) = make_chatwidget_manual_with_sender().await;
    app.active_thread_id = Some(source);
    app.replace_chat_widget(widget);
    app.pending_realtime_transcript_replay.insert(
        source,
        [crate::chatwidget::RealtimeTranscriptRecord {
            role: "assistant".into(),
            text: "Same   answer".into(),
            complete: true,
            before_turn_id: None,
        }]
        .into(),
    );
    let answer = |id: &str, text: &str| ThreadItem::AgentMessage {
        id: id.into(),
        text: text.into(),
        phase: Some(MessagePhase::FinalAnswer),
        questions: None,
        memory_citation: None,
        delivery: None,
    };
    app.pending_realtime_speech_replay.insert(
        source,
        vec![
            ("turn-1".into(), answer("answer-1", "[FINAL] Same answer")),
            ("turn-2".into(), answer("answer-2", "Same answer")),
            ("turn-3".into(), answer("answer-3", "Different answer")),
        ],
    );
    app.replay_thread_snapshot(
        empty_thread_snapshot(&app, source),
        /*resume_restored_queue*/ false,
    );
    let (mut app_server, _requests, proxy) = start_recording_remote_app_server(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    while let Ok(event) = events.try_recv() {
        Box::pin(app.handle_event(&mut tui, &mut app_server, event)).await?;
        app.chat_widget.pre_draw_tick();
    }
    let rendered = app
        .transcript_cells
        .iter()
        .flat_map(|cell| cell.display_lines(/*width*/ 80))
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(rendered.matches("Same answer").count(), 1);
    assert_eq!(rendered.matches("Same   answer").count(), 1);
    assert_eq!(rendered.matches("Different answer").count(), 1);
    assert!(!rendered.contains("[FINAL]"));
    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn inactive_voice_replay_is_bounded_and_discarded_with_its_thread() {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let ids = (0..17).map(|_| ThreadId::new()).collect::<Vec<_>>();
    for (index, thread_id) in ids.iter().enumerate() {
        app.retain_inactive_realtime_transcript(
            *thread_id,
            &ServerNotification::ThreadRealtimeTranscriptDone(
                codex_app_server_protocol::ThreadRealtimeTranscriptDoneNotification {
                    thread_id: thread_id.to_string(),
                    role: "assistant".into(),
                    text: format!("caption {index}"),
                },
            ),
        );
    }
    assert_eq!(app.realtime_replay_order.len(), 16);
    assert!(!app.pending_realtime_transcript_replay.contains_key(&ids[0]));
    assert!(
        app.pending_realtime_transcript_replay
            .contains_key(&ids[16])
    );

    app.discard_thread_local_state(ids[16]).await;
    assert!(
        !app.pending_realtime_transcript_replay
            .contains_key(&ids[16])
    );
    assert!(!app.realtime_replay_order.contains(&ids[16]));
    app.reset_thread_event_state();
    assert!(app.pending_realtime_transcript_replay.is_empty());
    assert!(app.pending_realtime_speech_replay.is_empty());
    assert!(app.realtime_replay_order.is_empty());
}

#[tokio::test]
async fn buffered_voice_items_reconcile_captions_after_thread_switch() {
    let (mut app, _initial_events, _ops) = make_test_app_with_channels().await;
    let source = ThreadId::new();
    let (widget, _, mut events, _) = make_chatwidget_manual_with_sender().await;
    app.active_thread_id = Some(source);
    app.replace_chat_widget(widget);
    app.pending_realtime_transcript_replay.insert(
        source,
        [
            ("user", "buffered question"),
            ("assistant", "buffered answer"),
        ]
        .into_iter()
        .map(|(role, text)| crate::chatwidget::RealtimeTranscriptRecord {
            role: role.to_string(),
            text: text.to_string(),
            complete: true,
            before_turn_id: None,
        })
        .collect(),
    );
    let items = [
        ThreadItem::UserMessage {
            id: "user-item".into(),
            client_id: None,
            content: vec![UserInput::Text {
                text: "<realtime_delegation><input>buffered question</input></realtime_delegation>"
                    .into(),
                text_elements: Vec::new(),
            }],
        },
        test_agent_message("answer-item", "buffered answer"),
    ];
    let events_to_replay = items
        .into_iter()
        .map(|item| {
            ThreadBufferedEvent::Notification(Box::new(ServerNotification::ItemCompleted(
                ItemCompletedNotification {
                    item,
                    thread_id: source.to_string(),
                    turn_id: "buffered-turn".into(),
                    completed_at_ms: 0,
                },
            )))
        })
        .collect();
    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            delegated_turns: vec!["buffered-turn".into()],
            session: Some(test_thread_session(source, app.config.cwd.to_path_buf())),
            turns: Vec::new(),
            events: events_to_replay,
            active_reasoning_item: None,
            input_state: None,
        },
        /*resume_restored_queue*/ false,
    );
    let rendered = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(
                cell.transcript_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(rendered.matches("buffered question").count(), 1);
    assert_eq!(rendered.matches("buffered answer").count(), 1);
    insta::assert_snapshot!(
        "voice_buffered_replay_reconciles_captions",
        normalize_voice_snapshot_directory(&rendered, &app.config.cwd)
    );
}

#[tokio::test]
async fn unrendered_buffered_items_do_not_consume_retained_captions() {
    let (mut app, _initial_events, _ops) = make_test_app_with_channels().await;
    let source = ThreadId::new();
    let (widget, _, mut events, _) = make_chatwidget_manual_with_sender().await;
    app.active_thread_id = Some(source);
    app.replace_chat_widget(widget);
    app.pending_realtime_transcript_replay.insert(
        source,
        [
            ("user", "started only"),
            ("user", "evicted prompt"),
            ("assistant", "interrupted answer"),
        ]
        .into_iter()
        .map(|(role, text)| crate::chatwidget::RealtimeTranscriptRecord {
            role: role.to_string(),
            text: text.to_string(),
            complete: true,
            before_turn_id: None,
        })
        .collect(),
    );
    let user_item = |id: &str, text: &str| ThreadItem::UserMessage {
        id: id.into(),
        client_id: None,
        content: vec![UserInput::Text {
            text: format!("<realtime_delegation><input>{text}</input></realtime_delegation>"),
            text_elements: Vec::new(),
        }],
    };
    let completion = |turn_id: &str, status: TurnStatus, items: Vec<ThreadItem>| {
        ThreadBufferedEvent::Notification(Box::new(ServerNotification::TurnCompleted(
            TurnCompletedNotification {
                thread_id: source.to_string(),
                turn: Turn {
                    id: turn_id.into(),
                    items,
                    items_view: TurnItemsView::Summary,
                    status,
                    error: None,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                },
            },
        )))
    };
    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            delegated_turns: vec!["started".into(), "evicted".into(), "interrupted".into()],
            session: Some(test_thread_session(source, app.config.cwd.to_path_buf())),
            turns: Vec::new(),
            events: vec![
                ThreadBufferedEvent::Notification(Box::new(ServerNotification::ItemStarted(
                    ItemStartedNotification {
                        item: user_item("started-user", "started only"),
                        thread_id: source.to_string(),
                        turn_id: "started".into(),
                        started_at_ms: 0,
                    },
                ))),
                completion(
                    "evicted",
                    TurnStatus::Completed,
                    vec![user_item("evicted-user", "evicted prompt")],
                ),
                completion(
                    "interrupted",
                    TurnStatus::Interrupted,
                    vec![test_agent_message(
                        "interrupted-agent",
                        "interrupted answer",
                    )],
                ),
            ],
            active_reasoning_item: None,
            input_state: None,
        },
        /*resume_restored_queue*/ false,
    );
    commit_realtime_history_events(&mut app.chat_widget, &mut events);
    let rendered = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(
                cell.transcript_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    for text in ["started only", "evicted prompt", "interrupted answer"] {
        assert_eq!(
            rendered.matches(text).count(),
            1,
            "{text} should survive replay"
        );
    }
}

#[tokio::test]
async fn completed_voice_caption_survives_repeated_thread_replacement() {
    let (mut app, mut initial_events, _ops) = make_test_app_with_channels().await;
    let source = ThreadId::new();
    app.active_thread_id = Some(source);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(source, app.config.cwd.to_path_buf()));
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, source);
    app.chat_widget.handle_server_notification(
        ServerNotification::ThreadRealtimeTranscriptDone(
            codex_app_server_protocol::ThreadRealtimeTranscriptDoneNotification {
                thread_id: source.to_string(),
                role: "assistant".into(),
                text: "spoken complete".into(),
            },
        ),
        /*replay_kind*/ None,
    );
    commit_realtime_history_events(&mut app.chat_widget, &mut initial_events);
    let initial = std::iter::from_fn(|| initial_events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(
                cell.transcript_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(initial.matches("spoken complete").count(), 1);

    let typed = test_turn(
        "later-typed-turn",
        TurnStatus::Completed,
        vec![test_user_message("later-user", "Later typed question")],
    );
    app.chat_widget.handle_server_notification(
        turn_started_notification(source, &typed.id),
        /*replay_kind*/ None,
    );

    for cycle in 0..2 {
        let side = ThreadId::new();
        let (side_widget, _, mut side_events, _) = make_chatwidget_manual_with_sender().await;
        app.active_thread_id = Some(side);
        app.replace_chat_widget(side_widget);
        app.chat_widget
            .handle_thread_session_quiet(test_thread_session(side, app.config.cwd.to_path_buf()));
        assert_eq!(app.pending_realtime_transcript_replay[&source].len(), 1);
        while let Ok(event) = side_events.try_recv() {
            if let AppEvent::InsertHistoryCell(cell) = event {
                assert!(
                    cell.transcript_lines(/*width*/ 80)
                        .iter()
                        .all(|line| !line.to_string().contains("spoken complete"))
                );
            }
        }

        let (source_widget, _, mut source_events, _) = make_chatwidget_manual_with_sender().await;
        app.active_thread_id = Some(source);
        app.replace_chat_widget(source_widget);
        let mut snapshot = empty_thread_snapshot(&app, source);
        snapshot.turns.push(Turn {
            id: "earlier-typed-turn".into(),
            items: vec![test_agent_message("earlier-answer", "Earlier answer")],
            ..typed.clone()
        });
        snapshot.turns.push(typed.clone());
        app.replay_thread_snapshot(snapshot, /*resume_restored_queue*/ false);
        assert!(!app.pending_realtime_transcript_replay.contains_key(&source));
        commit_realtime_history_events(&mut app.chat_widget, &mut source_events);
        let rendered = std::iter::from_fn(|| source_events.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => Some(
                    cell.transcript_lines(/*width*/ 80)
                        .into_iter()
                        .map(|line| line.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(rendered.matches("spoken complete").count(), 1);
        assert!(
            rendered.find("Earlier answer").unwrap() < rendered.find("spoken complete").unwrap()
        );
        assert!(
            rendered.find("spoken complete").unwrap()
                < rendered.find("Later typed question").unwrap()
        );
        if cycle == 0 {
            insta::assert_snapshot!(
                "voice_completed_after_thread_switch",
                normalize_voice_snapshot_directory(&rendered, &app.config.cwd)
            );
        }
    }
}

#[tokio::test]
async fn inactive_caption_precedes_later_buffered_turn_without_start_event() {
    for include_delta in [false, true] {
        let (mut app, _initial_events, _ops) = make_test_app_with_channels().await;
        let source = ThreadId::new();
        app.retain_inactive_realtime_transcript(
            source,
            &ServerNotification::ThreadRealtimeTranscriptDone(
                codex_app_server_protocol::ThreadRealtimeTranscriptDoneNotification {
                    thread_id: source.to_string(),
                    role: "assistant".into(),
                    text: "Earlier spoken answer".into(),
                },
            ),
        );
        app.retain_inactive_realtime_transcript(
            source,
            &turn_started_notification(source, "later-turn"),
        );
        let (widget, _, mut events, _) = make_chatwidget_manual_with_sender().await;
        app.active_thread_id = Some(source);
        app.replace_chat_widget(widget);
        let mut snapshot = empty_thread_snapshot(&app, source);
        if include_delta {
            snapshot
                .events
                .push(ThreadBufferedEvent::Notification(Box::new(
                    ServerNotification::AgentMessageDelta(
                        codex_app_server_protocol::AgentMessageDeltaNotification {
                            thread_id: source.to_string(),
                            turn_id: "later-turn".into(),
                            item_id: "later-answer".into(),
                            delta: "Later typed question".into(),
                        },
                    ),
                )));
        }
        // The bounded replay buffer may have evicted both the start and item notifications.
        snapshot
            .events
            .push(ThreadBufferedEvent::Notification(Box::new(
                ServerNotification::TurnCompleted(TurnCompletedNotification {
                    thread_id: source.to_string(),
                    turn: test_turn(
                        "later-turn",
                        TurnStatus::Completed,
                        vec![test_agent_message("later-answer", "Later typed question")],
                    ),
                }),
            )));
        app.replay_thread_snapshot(snapshot, /*resume_restored_queue*/ false);
        let rendered = std::iter::from_fn(|| events.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => Some(
                    cell.transcript_lines(/*width*/ 80)
                        .into_iter()
                        .map(|line| line.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(rendered.matches("Earlier spoken answer").count(), 1);
        assert!(
            rendered.find("Earlier spoken answer").unwrap()
                < rendered.find("Later typed question").unwrap()
        );
    }
}

#[tokio::test]
async fn rejected_realtime_speech_restores_the_delegated_final_answer() -> Result<()> {
    let (mut app, mut events, mut ops) = make_test_app_with_channels().await;
    // This proxy forwards appendSpeech to an embedded server with no voice
    // session for the synthetic thread, which rejects the request.
    let (mut app_server, requests, proxy) = start_recording_remote_app_server(&app.config).await?;
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(thread_id, app.config.cwd.to_path_buf()));
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, thread_id);
    while ops.try_recv().is_ok() {}
    send_item(
        &mut app,
        thread_id,
        "rejected-turn",
        test_user_message(
            "voice-input",
            "<realtime_delegation><input>hello</input></realtime_delegation>",
        ),
        ItemEventKind::Started,
    );
    let answer = test_agent_message("rejected-final", "An answer to keep.");
    send_item(
        &mut app,
        thread_id,
        "rejected-turn",
        answer.clone(),
        ItemEventKind::Started,
    );
    send_item(
        &mut app,
        thread_id,
        "rejected-turn",
        answer.clone(),
        ItemEventKind::Completed,
    );
    app.handle_thread_event_now(ThreadBufferedEvent::Notification(Box::new(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: thread_id.to_string(),
            turn: Turn {
                id: "rejected-turn".to_string(),
                items: vec![answer],
                items_view: TurnItemsView::Summary,
                status: TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            },
        }),
    )));
    let speech = ops.try_recv()?;
    let delivery_id = match &speech {
        AppCommand::RealtimeConversationSpeech { delivery_id, .. } => *delivery_id,
        _ => unreachable!("voice completion queues speech"),
    };
    while events.try_recv().is_ok() {}
    let mut tui = crate::tui::test_support::make_test_tui()?;
    Box::pin(app.handle_event(&mut tui, &mut app_server, AppEvent::CodexOp(speech))).await?;
    assert_eq!(
        recorded_params(&requests, "thread/realtime/appendSpeech").len(),
        1
    );
    assert!(!app.chat_widget.has_pending_realtime_speech(delivery_id));
    let mut restored = 0;
    let mut rendered = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            let lines = cell
                .display_lines(/*width*/ 80)
                .into_iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>();
            if lines.iter().any(|line| line.contains("An answer to keep.")) {
                restored += 1;
            }
            rendered.extend(lines);
        }
    }
    assert_eq!(restored, 1);
    assert!(
        rendered
            .iter()
            .any(|line| line.contains("Voice conversation failed:"))
    );
    insta::assert_snapshot!(
        "rejected_realtime_speech",
        rendered
            .join("\n")
            .replace(&thread_id.to_string(), "<thread-id>")
    );
    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn switching_threads_retains_undelivered_voice_answer_after_replay_eviction() -> Result<()> {
    let (mut app, _events, mut ops) = make_test_app_with_channels().await;
    let (mut app_server, requests, proxy) = start_recording_realtime_speech_app_server(
        &app.config,
        RealtimeRequestBehavior::AcceptSpeech,
    )
    .await?;
    let original = ThreadId::new();
    app.active_thread_id = Some(original);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(original, app.config.cwd.to_path_buf()));
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, original);
    while ops.try_recv().is_ok() {}
    send_item(
        &mut app,
        original,
        "switched-turn",
        test_user_message(
            "voice-input",
            "<realtime_delegation><input>hello</input></realtime_delegation>",
        ),
        ItemEventKind::Started,
    );
    let answer = test_agent_message("switched-final", "Answer survives a thread switch.");
    send_item(
        &mut app,
        original,
        "switched-turn",
        answer.clone(),
        ItemEventKind::Started,
    );
    send_item(
        &mut app,
        original,
        "switched-turn",
        answer.clone(),
        ItemEventKind::Completed,
    );
    app.handle_thread_event_now(ThreadBufferedEvent::Notification(Box::new(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: original.to_string(),
            turn: Turn {
                id: "switched-turn".to_string(),
                items: vec![answer],
                items_view: TurnItemsView::Summary,
                status: TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            },
        }),
    )));
    let speech = ops.try_recv()?;
    let (side_widget, _, mut side_events, _) = make_chatwidget_manual_with_sender().await;
    let side = ThreadId::new();
    app.active_thread_id = Some(side);
    app.replace_chat_widget(side_widget);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(side, app.config.cwd.to_path_buf()));
    assert_eq!(app.pending_realtime_speech_replay[&original].len(), 1);
    // Reconnect discards the old AppEvent receiver before installing a new widget.
    let (replacement_tx, _replacement_rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = crate::app_event_sender::AppEventSender::new(replacement_tx);

    let mut tui = crate::tui::test_support::make_test_tui()?;
    Box::pin(app.handle_event(&mut tui, &mut app_server, AppEvent::CodexOp(speech))).await?;
    assert!(recorded_params(&requests, "thread/realtime/appendSpeech").is_empty());
    while let Ok(event) = side_events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            assert!(cell.display_lines(/*width*/ 80).iter().all(|line| {
                !line
                    .to_string()
                    .contains("Answer survives a thread switch.")
            }));
        }
    }

    let (original_widget, _, mut original_events, _) = make_chatwidget_manual_with_sender().await;
    app.active_thread_id = Some(original);
    app.replace_chat_widget(original_widget);
    // The ordinary bounded event buffer has lost ItemCompleted and TurnCompleted.
    app.replay_thread_snapshot(
        empty_thread_snapshot(&app, original),
        /*resume_restored_queue*/ false,
    );
    assert!(!app.pending_realtime_speech_replay.contains_key(&original));
    let mut restored = 0;
    while let Ok(event) = original_events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event
            && cell.display_lines(/*width*/ 80).iter().any(|line| {
                line.to_string()
                    .contains("Answer survives a thread switch.")
            })
        {
            restored += 1;
        }
    }
    assert_eq!(restored, 1);
    let replayed_item = test_agent_message("replayed-final", "Already in the thread snapshot.");
    let mut older_item = replayed_item.clone();
    if let ThreadItem::AgentMessage { text, .. } = &mut older_item {
        *text = "Older pending text.".to_string();
    }
    app.pending_realtime_speech_replay
        .insert(original, vec![("replayed-turn".to_string(), older_item)]);
    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            delegated_turns: Vec::new(),
            session: Some(test_thread_session(original, app.config.cwd.to_path_buf())),
            turns: vec![Turn {
                id: "replayed-turn".to_string(),
                items: vec![replayed_item],
                items_view: TurnItemsView::Summary,
                status: TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            }],
            events: Vec::new(),
            active_reasoning_item: None,
            input_state: None,
        },
        /*resume_restored_queue*/ false,
    );
    let mut snapshot_cells = 0;
    while let Ok(event) = original_events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            for line in cell.display_lines(/*width*/ 80) {
                assert!(!line.to_string().contains("Older pending text."));
                snapshot_cells +=
                    usize::from(line.to_string().contains("Already in the thread snapshot."));
            }
        }
    }
    assert_eq!(snapshot_cells, 1);
    assert!(!app.pending_realtime_speech_replay.contains_key(&original));

    // Switch before TurnCompleted exists at all. A hidden ItemCompleted must
    // survive without relying on a later turn event in the bounded buffer.
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, original);
    send_item(
        &mut app,
        original,
        "early-switch-turn",
        test_user_message(
            "early-voice-input",
            "<realtime_delegation><input>early</input></realtime_delegation>",
        ),
        ItemEventKind::Started,
    );
    let early_answer =
        test_agent_message("early-switch-final", "Hidden final before turn completion.");
    send_item(
        &mut app,
        original,
        "early-switch-turn",
        early_answer.clone(),
        ItemEventKind::Started,
    );
    send_item(
        &mut app,
        original,
        "early-switch-turn",
        early_answer,
        ItemEventKind::Completed,
    );
    assert!(ops.try_recv().is_err());
    let (side_widget, _, _, _) = make_chatwidget_manual_with_sender().await;
    app.active_thread_id = Some(side);
    app.replace_chat_widget(side_widget);
    assert_eq!(app.pending_realtime_speech_replay[&original].len(), 1);
    let (original_widget, _, mut early_events, _) = make_chatwidget_manual_with_sender().await;
    app.active_thread_id = Some(original);
    app.replace_chat_widget(original_widget);
    app.replay_thread_snapshot(
        empty_thread_snapshot(&app, original),
        /*resume_restored_queue*/ false,
    );
    let mut early_restored = 0;
    while let Ok(event) = early_events.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event
            && cell.display_lines(/*width*/ 80).iter().any(|line| {
                line.to_string()
                    .contains("Hidden final before turn completion.")
            })
        {
            early_restored += 1;
        }
    }
    assert_eq!(early_restored, 1);
    assert!(!app.pending_realtime_speech_replay.contains_key(&original));
    // Primary resume/fork uses a different replay entry point than a thread
    // switch; it must also drain the old widget's per-thread delivery stash.
    let resumed_answer = test_agent_message("primary-resume-final", "Recovered on primary resume.");
    app.pending_realtime_speech_replay.insert(
        original,
        vec![("primary-resume-turn".to_string(), resumed_answer)],
    );
    app.enqueue_primary_thread_session_with_presentation(
        test_thread_session(original, app.config.cwd.to_path_buf()),
        Vec::new(),
        crate::app::session_lifecycle::ThreadAttachPresentation::SessionLineage,
    )
    .await?;
    assert!(!app.pending_realtime_speech_replay.contains_key(&original));
    let primary_restored = std::iter::from_fn(|| early_events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(cell.raw_lines()),
            _ => None,
        })
        .flatten()
        .filter(|line| line.to_string() == "Recovered on primary resume.")
        .count();
    assert_eq!(primary_restored, 1);
    assert!(recorded_params(&requests, "thread/realtime/appendSpeech").is_empty());
    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

fn send_item(
    app: &mut App,
    thread_id: ThreadId,
    turn_id: &str,
    item: ThreadItem,
    kind: ItemEventKind,
) {
    let notification = match kind {
        ItemEventKind::Started => ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.to_string(),
            item,
            started_at_ms: 0,
        }),
        ItemEventKind::Completed => ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.to_string(),
            item,
            completed_at_ms: 0,
        }),
    };
    app.handle_thread_event_now(ThreadBufferedEvent::Notification(Box::new(notification)));
}

#[tokio::test]
async fn delegated_final_speech_reaches_app_server_once_and_stale_speech_is_rejected() -> Result<()>
{
    let (mut app, mut events, mut ops) = make_test_app_with_channels().await;
    let (mut app_server, requests, proxy) = start_recording_realtime_speech_app_server(
        &app.config,
        RealtimeRequestBehavior::AcceptSpeech,
    )
    .await?;
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(thread_id, app.config.cwd.to_path_buf()));
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, thread_id);
    while ops.try_recv().is_ok() {}
    let turn_id = "delegated-voice-turn";
    let delegation = test_user_message(
        "voice-input",
        "<realtime_delegation><input>hello</input></realtime_delegation>",
    );
    let answer = test_agent_message("voice-final", "Hello back.");
    send_item(
        &mut app,
        thread_id,
        turn_id,
        delegation.clone(),
        ItemEventKind::Started,
    );
    send_item(
        &mut app,
        thread_id,
        turn_id,
        delegation,
        ItemEventKind::Completed,
    );
    send_item(
        &mut app,
        thread_id,
        turn_id,
        answer.clone(),
        ItemEventKind::Started,
    );
    send_item(
        &mut app,
        thread_id,
        turn_id,
        answer.clone(),
        ItemEventKind::Completed,
    );
    send_item(
        &mut app,
        thread_id,
        turn_id,
        answer.clone(),
        ItemEventKind::Completed,
    );
    app.handle_thread_event_now(ThreadBufferedEvent::Notification(Box::new(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: thread_id.to_string(),
            turn: Turn {
                id: turn_id.to_string(),
                items: vec![answer],
                items_view: TurnItemsView::Summary,
                status: TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            },
        }),
    )));

    let speech = ops.try_recv()?;
    let delivery_id = match &speech {
        Op::RealtimeConversationSpeech { delivery_id, .. } => *delivery_id,
        _ => unreachable!("voice completion queues speech"),
    };
    assert!(
        matches!(&speech, Op::RealtimeConversationSpeech { text, .. } if text.as_str() == "Hello back."),
        "unexpected command: {speech:?}"
    );
    assert!(ops.try_recv().is_err());
    let mut tui = crate::tui::test_support::make_test_tui()?;
    Box::pin(app.handle_event(&mut tui, &mut app_server, AppEvent::CodexOp(speech.clone())))
        .await?;
    assert!(!app.chat_widget.has_pending_realtime_speech(delivery_id));
    assert_eq!(
        recorded_params(&requests, "thread/realtime/appendSpeech"),
        vec![serde_json::json!({"threadId": thread_id.to_string(), "text": "Hello back."})]
    );

    let typed = test_user_message("typed-input", "type instead");
    send_item(
        &mut app,
        thread_id,
        "typed-turn",
        typed,
        ItemEventKind::Started,
    );
    Box::pin(app.handle_event(&mut tui, &mut app_server, AppEvent::CodexOp(speech))).await?;
    assert_eq!(
        recorded_params(&requests, "thread/realtime/appendSpeech").len(),
        1
    );
    // A completed second turn may be queued when voice closes. The old command
    // must not reach the app server or duplicate the restored final history.
    let second_turn = "second-delegated-voice-turn";
    let second_input = test_user_message(
        "second-voice-input",
        "<realtime_delegation><input>again</input></realtime_delegation>",
    );
    send_item(
        &mut app,
        thread_id,
        second_turn,
        second_input,
        ItemEventKind::Started,
    );
    let second_answer = test_agent_message("second-final", "Second answer.");
    send_item(
        &mut app,
        thread_id,
        second_turn,
        second_answer.clone(),
        ItemEventKind::Started,
    );
    send_item(
        &mut app,
        thread_id,
        second_turn,
        second_answer.clone(),
        ItemEventKind::Completed,
    );
    app.handle_thread_event_now(ThreadBufferedEvent::Notification(Box::new(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: thread_id.to_string(),
            turn: Turn {
                id: second_turn.to_string(),
                items: vec![second_answer],
                items_view: TurnItemsView::Summary,
                status: TurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            },
        }),
    )));
    let queued = ops.try_recv()?;
    let queued_delivery_id = match &queued {
        Op::RealtimeConversationSpeech { delivery_id, .. } => *delivery_id,
        _ => unreachable!("second voice completion queues speech"),
    };
    assert!(
        app.chat_widget
            .has_pending_realtime_speech(queued_delivery_id)
    );
    app.chat_widget.reset_realtime_conversation();
    assert!(
        !app.chat_widget
            .has_pending_realtime_speech(queued_delivery_id)
    );
    Box::pin(app.handle_event(&mut tui, &mut app_server, AppEvent::CodexOp(queued))).await?;
    assert_eq!(
        recorded_params(&requests, "thread/realtime/appendSpeech").len(),
        1
    );
    let restored = events
        .try_recv()
        .ok()
        .into_iter()
        .chain(std::iter::from_fn(|| events.try_recv().ok()))
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(
                cell.display_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<String>(),
            ),
            _ => None,
        })
        .filter(|line| line.contains("Second answer."))
        .count();
    assert_eq!(restored, 1);
    while events.try_recv().is_ok() {}

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn rejected_voice_setting_preserves_current_selection() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let (mut app_server, _requests, proxy) = start_recording_remote_app_server(&app.config).await?;
    let previous = (
        app.config.realtime.voice,
        app.chat_widget.config_ref().realtime.voice,
    );
    std::fs::write(app.config.codex_home.join("config.toml"), "realtime = [")?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    Box::pin(app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::PersistRealtimeVoiceSelection {
            voice: codex_protocol::protocol::RealtimeVoice::Juniper,
        },
    ))
    .await?;
    assert_eq!(
        (
            app.config.realtime.voice,
            app.chat_widget.config_ref().realtime.voice
        ),
        previous,
    );
    let messages = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(
                cell.display_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(messages.contains("Failed to save voice"));
    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn remote_voice_config_read_failure_does_not_start_with_local_voice() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let (mut app_server, requests, proxy) = start_recording_remote_app_server(&app.config).await?;
    std::fs::write(app.config.codex_home.join("config.toml"), "realtime = [")?;
    let thread_id = ThreadId::new();
    let error = app
        .try_submit_active_thread_op_via_app_server(
            &mut app_server,
            thread_id,
            &AppCommand::RealtimeConversationStart {
                thread_id,
                offer_sdp: String::from("v=0\r\n").into(),
            },
        )
        .await
        .expect_err("invalid server config must not fall back to local voice");
    assert!(error.to_string().contains("config/read"));
    assert!(recorded_params(&requests, "thread/realtime/start").is_empty());
    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn embedded_voice_settings_follow_project_after_thread_switch() -> Result<()> {
    use codex_protocol::protocol::RealtimeVoice;
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    let projects = tempfile::tempdir()?;
    for voice in [RealtimeVoice::Maple, RealtimeVoice::Sol] {
        let cwd = projects.path().join(voice.wire_name());
        std::fs::create_dir_all(cwd.join(".codex"))?;
        std::fs::write(
            cwd.join(".codex/config.toml"),
            format!("[realtime]\nvoice = \"{}\"\n", voice.wire_name()),
        )?;
        crate::legacy_core::config::set_project_trust_level(
            &app.config.codex_home,
            &cwd,
            codex_protocol::config_types::TrustLevel::Trusted,
        )
        .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
    }
    app.config.realtime.voice = Some(RealtimeVoice::Maple);
    let (mut server, requests, proxy) = start_recording_realtime_speech_app_server(
        &app.config,
        RealtimeRequestBehavior::AcceptStart,
    )
    .await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    for voice in [
        RealtimeVoice::Maple,
        RealtimeVoice::Sol,
        RealtimeVoice::Maple,
    ] {
        let thread_id = ThreadId::new();
        let cwd = projects.path().join(voice.wire_name());
        app.thread_event_channels.insert(
            thread_id,
            ThreadEventChannel::new_with_session(
                THREAD_EVENT_CHANNEL_CAPACITY,
                test_thread_session(thread_id, cwd),
                Vec::new(),
            ),
        );
        Box::pin(app.select_agent_thread(&mut tui, &mut server, thread_id)).await?;
        app.open_realtime_settings(&server).await;
        let popup = crate::chatwidget::tests::helpers::render_bottom_popup(
            &app.chat_widget,
            /*width*/ 80,
        );
        assert!(
            popup.contains(&format!("{} (current)", voice.wire_name())),
            "{popup}"
        );
        app.try_submit_active_thread_op_via_app_server(
            &mut server,
            thread_id,
            &AppCommand::RealtimeConversationStart {
                thread_id,
                offer_sdp: String::from("v=0\r\n").into(),
            },
        )
        .await?;
        assert_eq!(
            recorded_params(&requests, "thread/realtime/start")
                .last()
                .unwrap()["voice"],
            serde_json::json!(voice),
        );
    }
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn remote_voice_picker_reads_server_preference() -> Result<()> {
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    std::fs::write(
        app.config.codex_home.join("config.toml"),
        "[realtime]\nvoice = \"sol\"\n",
    )?;
    app.chat_widget
        .set_realtime_voice(Some(codex_protocol::protocol::RealtimeVoice::Maple));
    let (server, requests, proxy) = start_recording_remote_app_server(&app.config).await?;
    app.open_realtime_settings(&server).await;
    assert_eq!(
        recorded_params(&requests, "thread/realtime/listVoices").len(),
        1
    );
    insta::assert_snapshot!(
        "remote_voice_picker",
        crate::chatwidget::tests::helpers::render_bottom_popup(&app.chat_widget, /*width*/ 80)
    );
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn remote_voice_picker_uses_server_catalog_and_falls_back_when_unavailable() -> Result<()> {
    use super::session_lifecycle_requests::HistoryCapabilities;
    for (capabilities, expected_catalog) in [
        (HistoryCapabilities::VoiceCatalogCustom, true),
        (HistoryCapabilities::VoiceCatalogUnavailable, false),
    ] {
        let (mut app, _events, _ops) = make_test_app_with_channels().await;
        let (server, requests, proxy) =
            super::session_lifecycle_requests::start_recording_app_server_with_realtime_speech(
                &app.config,
                capabilities,
                /*blocked_thread_list*/ None,
                /*failed_thread_name*/ None,
                crate::app_server_session::ThreadParamsMode::Remote,
                RealtimeRequestBehavior::Forward,
                codex_config::LoaderOverrides::default(),
            )
            .await?;
        app.open_realtime_settings(&server).await;
        let popup = crate::chatwidget::tests::helpers::render_bottom_popup(
            &app.chat_widget,
            /*width*/ 80,
        );
        assert_eq!(popup.contains("1. maple (current)"), expected_catalog);
        assert_eq!(popup.contains("  3. spruce"), !expected_catalog);
        if expected_catalog {
            insta::assert_snapshot!("remote_voice_picker_server_default", popup);
        }
        assert_eq!(
            recorded_params(&requests, "thread/realtime/listVoices").len(),
            1
        );
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn overridden_voice_save_keeps_effective_voice() -> Result<()> {
    for project_override in [false, true] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        let project = tempfile::tempdir()?;
        let overrides = if project_override {
            std::fs::create_dir_all(project.path().join(".codex"))?;
            std::fs::write(
                project.path().join(".codex/config.toml"),
                "[realtime]\nvoice = \"maple\"\n",
            )?;
            crate::legacy_core::config::set_project_trust_level(
                &app.config.codex_home,
                project.path(),
                codex_protocol::config_types::TrustLevel::Trusted,
            )
            .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
            app.config.cwd = AbsolutePathBuf::from_absolute_path(project.path())?;
            app.chat_widget
                .handle_thread_session_quiet(test_thread_session(
                    ThreadId::new(),
                    project.path().to_path_buf(),
                ));
            Vec::new()
        } else {
            vec![(
                "realtime.voice".to_string(),
                toml::Value::String("maple".to_string()),
            )]
        };
        let client = crate::start_embedded_app_server(
            codex_arg0::Arg0DispatchPaths::default(),
            app.config.clone(),
            overrides,
            codex_config::LoaderOverrides::without_managed_config_for_tests(),
            /*strict_config*/ false,
            codex_config::CloudConfigBundleLoader::default(),
            codex_feedback::CodexFeedback::new(),
            /*log_db*/ None,
            /*state_db*/ None,
            Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        )
        .await?;
        let server = AppServerSession::new(
            codex_app_server_client::AppServerClient::InProcess(client),
            crate::app_server_session::ThreadParamsMode::Embedded,
        );
        while events.try_recv().is_ok() {}
        app.persist_realtime_voice(&server, codex_protocol::protocol::RealtimeVoice::Juniper)
            .await;
        assert_eq!(
            (
                app.config.realtime.voice,
                app.chat_widget.config_ref().realtime.voice
            ),
            (
                Some(codex_protocol::protocol::RealtimeVoice::Maple),
                Some(codex_protocol::protocol::RealtimeVoice::Maple)
            ),
        );
        let message = std::iter::from_fn(|| events.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => Some(
                    cell.display_lines(/*width*/ 80)
                        .into_iter()
                        .map(|line| line.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(message.contains("saved but not applied"));
        if !project_override {
            insta::assert_snapshot!("overridden_voice_save", message);
        }
        server.shutdown().await?;
    }
    Ok(())
}
