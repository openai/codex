//! Exercises attachment races through Core submissions and real sideband sockets.

use anyhow::Result;
use codex_api::RealtimeEvent;
use codex_features::Feature;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::ConversationStartTransport;
use codex_protocol::protocol::ConversationTextParams;
use codex_protocol::protocol::ConversationTextRole;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RealtimeAttachmentOptions;
use codex_protocol::protocol::RealtimeConversationVersion;
use codex_protocol::protocol::RealtimeOutputModality;
use codex_protocol::protocol::RealtimeSidebandTransport;
use codex_protocol::protocol::ThreadHistoryMode;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

struct Sideband {
    url: String,
    connections: mpsc::UnboundedReceiver<Connection>,
    task: tokio::task::JoinHandle<()>,
}

/// A caller-owned stream drives a real Core delegation and returns context without any callback socket.
#[tokio::test]
async fn initiated_sideband_delegates_without_callback() -> Result<()> {
    let api = start_mock_server().await;
    let response = core_test_support::responses::mount_sse_once(
        &api,
        core_test_support::responses::sse(vec![
            core_test_support::responses::ev_response_created("stream-handoff"),
            core_test_support::responses::ev_assistant_message(
                "answer",
                "Stream delegation completed",
            ),
            core_test_support::responses::ev_completed("stream-handoff"),
        ]),
    )
    .await;
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_model_info_override("gpt-5.5", |model| model.use_responses_lite = false)
        .with_config(|config| config.base_instructions = Some(String::new()))
        .build_with_auto_env(&api)
        .await?;
    // Any regression to the existing-call WebSocket path is both observed and fails attachment.
    let callback = TcpListener::bind("127.0.0.1:0").await?;
    let callback_url = format!("ws://{}", callback.local_addr()?);
    let (events, incoming) = mpsc::channel(32);
    let (outgoing, mut commands) = mpsc::channel(32);
    let (reply, ready) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationAttachStream {
            params: params("stream", &callback_url),
            transport: RealtimeSidebandTransport { incoming, outgoing },
            reply,
        })
        .await?;
    timeout(Duration::from_secs(10), ready).await???;
    events
        .send(
            json!({
                "type": "delegation.created",
                "offset_ms": 0,
                "item": {
                    "id": "stream-handoff",
                    "type": "delegation",
                    "target": "client",
                    "content": [{
                        "type": "input_text",
                        "text": "complete the stream delegation"
                    }]
                }
            })
            .to_string(),
        )
        .await?;
    timeout(Duration::from_secs(20), async {
        while let Some(command) = commands.recv().await {
            let command: Value = serde_json::from_str(&command)?;
            if command["type"] == "delegation.context.append"
                && command.to_string().contains("Stream delegation completed")
            {
                assert_eq!(command["delegation_item_id"], "stream-handoff");
                return anyhow::Ok(());
            }
        }
        anyhow::bail!("Core closed before returning delegation context")
    })
    .await??;
    assert_eq!(response.requests().len(), 1);
    assert!(
        timeout(Duration::from_millis(100), callback.accept())
            .await
            .is_err()
    );
    drop(events);
    timeout(Duration::from_secs(10), async {
        while commands.recv().await.is_some() {}
    })
    .await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

/// Inbound events, replacement, and detach stay live even when the host never drains output.
#[tokio::test]
async fn initiated_sideband_backpressure_keeps_events_and_lifecycle_live() -> Result<()> {
    let api = start_mock_server().await;
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .build_with_auto_env(&api)
        .await?;
    let mut transports = Vec::new();
    for session in ["original", "replacement"] {
        let (events, incoming) = mpsc::channel(/*buffer*/ 1);
        let (outgoing, commands) = mpsc::channel(/*buffer*/ 1);
        // Occupy the host's queue before attaching, and retain its receiver until after detach.
        outgoing.send("occupied".to_owned()).await?;
        let (reply, ready) = oneshot::channel();
        test.codex
            .submit(Op::RealtimeConversationAttachStream {
                params: params(session, "ws://unused.invalid"),
                transport: RealtimeSidebandTransport { incoming, outgoing },
                reply,
            })
            .await?;
        timeout(Duration::from_secs(/*secs*/ 10), ready).await???;

        // Core's text queue holds 64 items. Accepting 65 submissions before the
        // barrier below proves that the writer has dequeued text into a blocked send.
        for _ in 0..65 {
            test.codex
                .submit(Op::RealtimeConversationText(ConversationTextParams {
                    text: "blocked output".to_owned(),
                    role: ConversationTextRole::User,
                }))
                .await?;
        }
        // A mismatched detach is an acknowledged no-op on the submission loop.
        detach(&test, "not-selected").await?;
        events
            .send(json!({"type":"session.started", "session":{"id":session}}).to_string())
            .await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::RealtimeConversationRealtime(event)
                if matches!(&event.payload, RealtimeEvent::SessionUpdated { realtime_session_id, .. }
                    if realtime_session_id == session))
        })
        .await;
        transports.push((events, commands));
        if session == "original" {
            end(&test, session).await?;
        }
    }
    detach(&test, "replacement").await?;
    for (events, mut commands) in transports {
        timeout(Duration::from_secs(/*secs*/ 10), events.closed()).await?;
        assert_eq!(commands.recv().await, Some("occupied".to_owned()));
        assert_eq!(commands.recv().await, None);
    }
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

/// Terminal stop is a history barrier for accepted final speech.
#[tokio::test(flavor = "current_thread")]
async fn initiated_sideband_end_preserves_queued_transcript_in_history() -> Result<()> {
    let api = start_mock_server().await;
    let test = test_codex().build_with_auto_env(&api).await?;
    test.codex.ensure_rollout_materialized().await;
    for index in 0..8 {
        let session = format!("hangup-{index}");
        let completed_turn = index % 2 == 1;
        let text = if completed_turn {
            format!(
                "{}final words before hangup {index}",
                "completed words ".repeat(800)
            )
        } else {
            format!("final words before hangup {index}")
        };
        let (events, incoming) = mpsc::channel(32);
        let (outgoing, commands) = mpsc::channel(32);
        let (reply, ready) = oneshot::channel();
        let mut start_params = params(&session, "ws://unused.invalid");
        // V3 ignores the old unfinished-tail setting.
        start_params.flush_transcript_tail_on_session_end = !completed_turn;
        test.codex
            .submit(Op::RealtimeConversationAttachStream {
                params: start_params,
                transport: RealtimeSidebandTransport { incoming, outgoing },
                reply,
            })
            .await?;
        timeout(Duration::from_secs(10), ready).await???;
        let event = if completed_turn {
            json!({"type": "turn.done", "turn": {"role": "user", "transcript": text}})
        } else {
            json!({"type": "input_transcript.added", "item": {"text": text}})
        };
        events.try_send(event.to_string())?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::RealtimeConversationRealtime(_))
        })
        .await;
        end(&test, &session).await?;
        drop(events);
        drop(commands);
        let history = test.codex.load_history(/*include_archived*/ false).await?;
        let tails = history.items.iter().filter(|item| {
            matches!(item,
                codex_history::RolloutItem::ResponseItem(codex_history::ResponseItemEnvelope {
                    item: codex_protocol::models::ResponseItem::Message { role, content, .. }, ..
                }) if role == "user" && content.iter().any(|item| matches!(item,
                    codex_protocol::models::ContentItem::InputText { text: recorded }
                        if recorded.contains(&text)
                ))
            )
        }).count();
        assert_eq!(
            tails, 1,
            "terminal stop must persist the accepted final transcript exactly once"
        );
    }
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

/// A task-only delegation already parsed before stop is routed while fanout drains.
#[tokio::test]
async fn terminal_stop_routes_a_parsed_task_only_handoff() -> Result<()> {
    use super::realtime_conversation::realtime_history_store::HostedClockStore;
    use codex_protocol::realtime::RealtimeItemContent;
    use codex_thread_store::ThreadStore;
    use core_test_support::responses;

    let api = start_mock_server().await;
    let response = responses::mount_sse_once(&api, responses::sse_completed("queued-task")).await;
    let store = Arc::new(HostedClockStore::blocking_frame());
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_thread_store(store.clone())
        .with_model_info_override("gpt-5.5", |model| model.use_responses_lite = false)
        .build_with_auto_env(&api)
        .await?;
    let (events, incoming) = mpsc::channel(1);
    let (outgoing, _commands) = mpsc::channel(32);
    let (reply, ready) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationAttachStream {
            params: params("queued", "ws://unused.invalid"),
            transport: RealtimeSidebandTransport { incoming, outgoing },
            reply,
        })
        .await?;
    timeout(Duration::from_secs(10), ready).await???;
    let completed_transcript = "earlier speech ".repeat(400);
    events
        .send(
            json!({"type":"turn.done", "turn":{"role":"user", "transcript":completed_transcript}})
                .to_string(),
        )
        .await?;
    timeout(Duration::from_secs(10), store.wait_for_blocked_frame()).await?;
    events.send(json!({"type":"delegation.created", "item":{"id":"queued-task", "type":"delegation", "target":"client", "content":[{"type":"input_text", "text":"do this task"}]}}).to_string()).await?;
    // The one-slot input queue accepting two more events proves the delegation was parsed.
    for _ in 0..2 {
        events
            .send(json!({"type":"session.started", "session":{"id":"queued"}}).to_string())
            .await?;
    }
    let (reply, stopped) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationEnd {
            realtime_session_id: "queued".to_owned(),
            reply,
        })
        .await?;
    timeout(Duration::from_secs(10), events.closed()).await?;
    store.release_frame();
    timeout(Duration::from_secs(10), stopped).await???;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(response.requests().len(), 1);
    let items = store
        .load_realtime_session(test.session_configured.thread_id, "queued".to_owned())
        .await?
        .expect("started session");
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(&item.content,
                RealtimeItemContent::TurnInputAccepted { handoff_id, source_event_ids, .. }
                    if handoff_id == "queued-task" && source_event_ids.is_empty()
            ))
            .count(),
        1
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

/// Transport loss must arm recovery even when fanout cannot drain its bounded queue.
#[tokio::test(flavor = "current_thread")]
async fn sideband_loss_with_saturated_fanout_requires_recovery_without_end() -> Result<()> {
    use super::realtime_conversation::realtime_history_store::HostedClockStore;

    let api = start_mock_server().await;
    let store = Arc::new(HostedClockStore::blocking_frame());
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_thread_store(store.clone())
        .build_with_auto_env(&api)
        .await?;
    let (events, incoming) = mpsc::channel(1);
    let (outgoing, commands) = mpsc::channel(1);
    let (reply, ready) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationAttachStream {
            params: params("saturated", "ws://unused.invalid"),
            transport: RealtimeSidebandTransport { incoming, outgoing },
            reply,
        })
        .await?;
    timeout(Duration::from_secs(10), ready).await???;
    events
        .send(
            json!({"type":"turn.done", "turn":{"role":"user", "transcript":"unfinished"}})
                .to_string(),
        )
        .await?;
    timeout(Duration::from_secs(10), store.wait_for_blocked_frame()).await?;
    // Fanout's queue holds 256 events; the next event parks its producer, and
    // the final event occupies the one-slot caller queue.
    timeout(Duration::from_secs(10), async {
        for _ in 0..258 {
            events
                .send(json!({"type":"session.started", "session":{"id":"saturated"}}).to_string())
                .await?;
        }
        anyhow::Ok(())
    })
    .await??;
    tokio::time::pause();
    drop(events);
    drop(commands);
    // Let the transport watcher and watchdog run as their deadline approaches.
    for _ in 0..32 {
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
    }
    tokio::time::resume();
    let closed = timeout(Duration::from_secs(10), async {
        loop {
            if let EventMsg::RealtimeConversationClosed(closed) = test.codex.next_event().await?.msg
            {
                break anyhow::Ok(closed);
            }
        }
    })
    .await??;
    assert_eq!(closed.reason.as_deref(), Some("history_recovery_required"));
    store.release_frame();
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

/// Realtime frames carry pending input over at logical end, including after detach.
#[tokio::test]
async fn frame_enrolled_v3_saves_input_even_when_legacy_flush_setting_is_false() -> Result<()> {
    use super::realtime_conversation::realtime_history_store::HostedClockStore;
    use codex_protocol::realtime::RealtimeItemContent;
    use codex_protocol::realtime::SessionInputReason;
    use codex_thread_store::ThreadStore;

    let api = start_mock_server().await;
    let store = Arc::new(HostedClockStore::default());
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_thread_store(store.clone())
        .build_with_auto_env(&api)
        .await?;
    let (events, incoming) = mpsc::channel(1);
    let (outgoing, _commands) = mpsc::channel(1);
    let (reply, ready) = oneshot::channel();
    let mut start = params("no-flush", "ws://unused.invalid");
    start.flush_transcript_tail_on_session_end = false;
    test.codex
        .submit(Op::RealtimeConversationAttachStream {
            params: start,
            transport: RealtimeSidebandTransport { incoming, outgoing },
            reply,
        })
        .await?;
    timeout(Duration::from_secs(10), ready).await???;
    events
        .send(
            json!({"type":"input_transcript.added", "item":{"text":"save this at hangup"}})
                .to_string(),
        )
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::RealtimeConversationRealtime(event)
            if matches!(event.payload, codex_protocol::protocol::RealtimeEvent::InputTranscriptDelta(_)))
    }).await;
    end(&test, "no-flush").await?;
    let items = store
        .load_realtime_session(test.session_configured.thread_id, "no-flush".to_owned())
        .await?
        .expect("started session");
    assert!(items.iter().any(|item| matches!(&item.content,
        RealtimeItemContent::SessionInputSaved { reason: SessionInputReason::SessionEnd, input, .. }
            if input.contains("save this at hangup")
    )));
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

/// Thread-scoped Stop recovers the latest open persisted session after Core reload.
#[tokio::test]
async fn thread_stop_recovers_and_finishes_the_latest_persisted_session() -> Result<()> {
    use super::realtime_conversation::realtime_history_store::HostedClockStore;
    use codex_protocol::realtime::RealtimeFrameChannels;
    use codex_protocol::realtime::RealtimeItem;
    use codex_protocol::realtime::RealtimeItemContent;
    use codex_protocol::realtime::RealtimeTranscriptRole;
    use codex_protocol::realtime::RealtimeVoiceTranscript;
    use codex_protocol::realtime::SessionInputReason;
    use codex_thread_store::ThreadStore;
    use core_test_support::responses;

    let api = start_mock_server().await;
    let model = responses::mount_sse_once(
        &api,
        responses::sse(vec![responses::ev_response_created("unexpected")]),
    )
    .await;
    let store = Arc::new(HostedClockStore::default());
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_thread_store(store.clone())
        .build_with_auto_env(&api)
        .await?;
    let thread_id = test.session_configured.thread_id;
    let session_id = "cold-stop";
    // Populate only persisted history: this Core runtime has never attached the call.
    store
        .seal_realtime_items(
            thread_id,
            vec![
                RealtimeItem {
                    id: "start".to_owned(),
                    realtime_session_id: session_id.to_owned(),
                    observed_at_ms: Some(1),
                    content: RealtimeItemContent::RealtimeSessionStarted,
                },
                RealtimeItem {
                    id: "speech".to_owned(),
                    realtime_session_id: session_id.to_owned(),
                    observed_at_ms: Some(2),
                    content: RealtimeItemContent::Frame {
                        channels: RealtimeFrameChannels {
                            voice_transcript: RealtimeVoiceTranscript {
                                role: RealtimeTranscriptRole::User,
                                text: "remember the smaller logo".to_owned(),
                            },
                        },
                    },
                },
            ],
        )
        .await?;
    test.codex.submit(Op::RealtimeConversationClose).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::RealtimeConversationClosed(_))
    })
    .await;
    test.codex.submit(Op::RealtimeConversationClose).await?;
    test.codex.shutdown_and_wait().await?;
    let items = store
        .load_realtime_session(thread_id, session_id.to_owned())
        .await?
        .expect("persisted session");
    assert_eq!(
        items.iter().filter(|item| matches!(&item.content,
            RealtimeItemContent::SessionInputSaved { reason: SessionInputReason::SessionEnd, input, source_event_ids, .. }
                if input.contains("remember the smaller logo") && source_event_ids == &["speech".to_owned()]
        )).count(),
        1
    );
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(
                item.content,
                RealtimeItemContent::RealtimeSessionClosed { .. }
            ))
            .count(),
        1
    );
    assert!(model.requests().is_empty());
    Ok(())
}

/// A timeout fences an allowing steer hook and lets ordinary turn abort own cleanup.
#[cfg(not(target_os = "windows"))]
#[tokio::test(flavor = "current_thread")]
async fn hangup_timeout_aborts_a_turn_with_an_allowing_steer_hook() -> Result<()> {
    use super::realtime_conversation::realtime_history_store::HostedClockStore;
    use codex_core::TurnInputRequest;
    use codex_history::RolloutItem;
    use codex_protocol::realtime::RealtimeItemContent;
    use codex_protocol::user_input::UserInput;
    use codex_thread_store::LoadThreadHistoryParams;
    use codex_thread_store::ThreadStore;
    use core_test_support::responses;
    use core_test_support::streaming_sse::StreamingSseChunk;
    use core_test_support::streaming_sse::start_streaming_sse_server;

    core_test_support::skip_if_wine_exec!(Ok(()), "the hook requires host-native Python");
    let (release_response, response_gate) = oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![vec![
        StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![responses::ev_response_created("active")]),
        },
        StreamingSseChunk {
            gate: Some(response_gate),
            body: responses::sse(vec![responses::ev_completed("active")]),
        },
    ]])
    .await;
    let store = Arc::new(HostedClockStore::default());
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_thread_store(store.clone())
        .with_config(core_test_support::hooks::trust_discovered_hooks)
        .with_pre_build_hook(|home| {
            super::hooks::write_user_prompt_submit_hook(home, "never-block", "")
                .expect("write allowing prompt hook");
            let script = home.join("user_prompt_submit_hook.py");
            let original = std::fs::read_to_string(&script).expect("read hook");
            let gate = format!(
                "payload = json.load(sys.stdin)\nif 'steer-timeout' in payload.get('prompt', ''):\n    import time\n    Path(r\"{}\").touch()\n    while not Path(r\"{}\").exists():\n        time.sleep(0.01)",
                home.join("hook_started").display(),
                home.join("hook_release").display(),
            );
            std::fs::write(script, original.replace("payload = json.load(sys.stdin)", &gate))
                .expect("gate allowing steer hook");
        })
        .build_with_streaming_server(&server)
        .await?;
    let started = test
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "initial active turn".to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let turn_id = match started {
        codex_core::TurnInputSubmission::Started { turn_id, .. } => turn_id,
        other => anyhow::bail!("expected started turn, got {other:?}"),
    };
    timeout(
        Duration::from_secs(10),
        server.wait_for_request_count(/*count*/ 1),
    )
    .await?;
    let (events, incoming) = mpsc::channel(8);
    let (outgoing, _commands) = mpsc::channel(8);
    let (reply, ready) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationAttachStream {
            params: params("hook-timeout", "ws://unused.invalid"),
            transport: RealtimeSidebandTransport { incoming, outgoing },
            reply,
        })
        .await?;
    timeout(Duration::from_secs(10), ready).await???;
    events.send(json!({"type":"delegation.created", "item":{"id":"steer-timeout", "type":"delegation", "target":"client", "content":[{"type":"input_text", "text":"steer-timeout"}]}}).to_string()).await?;
    wait_for_event(&test.codex, |event| matches!(event,
        EventMsg::RealtimeConversationRealtime(event)
            if matches!(&event.payload, RealtimeEvent::HandoffRequested(handoff) if handoff.handoff_id == "steer-timeout")
    )).await;
    release_response.send(()).expect("first response gate");
    let started = test.home.path().join("hook_started");
    timeout(Duration::from_secs(10), async {
        while !started.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    let (reply, mut stopped) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationEnd {
            realtime_session_id: "hook-timeout".to_owned(),
            reply,
        })
        .await?;
    assert!(
        timeout(Duration::from_millis(100), &mut stopped)
            .await
            .is_err()
    );
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(31)).await;
    let result = timeout(Duration::from_secs(10), stopped).await;
    tokio::time::resume();
    assert!(result??.is_err(), "timeout must require recovery");
    std::fs::write(test.home.path().join("hook_release"), "")?;
    wait_for_event(&test.codex, |event| matches!(event, EventMsg::TurnAborted(aborted) if aborted.turn_id.as_deref() == Some(turn_id.as_str()))).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        server.requests().await.len(),
        1,
        "the released steer must not sample"
    );
    let history = store
        .load_latest_model_context(LoadThreadHistoryParams {
            thread_id: test.session_configured.thread_id,
            include_archived: false,
        })
        .await?;
    assert_eq!(history.items.iter().filter(|item| matches!(item,
        RolloutItem::EventMsg { event: EventMsg::TurnAborted(aborted), .. } if aborted.turn_id.as_deref() == Some(turn_id.as_str())
    ) || matches!(item,
        RolloutItem::EventMsg { event: EventMsg::TurnComplete(completed), .. } if completed.turn_id == turn_id
    )).count(), 1);
    let session = store
        .load_realtime_session(test.session_configured.thread_id, "hook-timeout".to_owned())
        .await?
        .expect("started session");
    assert!(!session.iter().any(|item| matches!(&item.content,
        RealtimeItemContent::TurnInputAccepted { handoff_id, .. } if handoff_id == "steer-timeout"
    )));
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

/// Reattaching an open logical call does not start a second session; a later cold stop closes it.
#[tokio::test]
async fn reconnect_restores_sealed_pending_speech_and_rejects_closed_reuse() -> Result<()> {
    use super::realtime_conversation::realtime_history_store::HostedClockStore;
    use codex_protocol::realtime::RealtimeItemContent;
    use codex_protocol::realtime::SessionInputReason;
    use codex_thread_store::ThreadStore;

    let api = start_mock_server().await;
    let store = Arc::new(HostedClockStore::default());
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_thread_store(store.clone())
        .build_with_auto_env(&api)
        .await?;
    for attach in 0..2 {
        let (events, incoming) = mpsc::channel(8);
        let (outgoing, _commands) = mpsc::channel(8);
        let (reply, ready) = oneshot::channel();
        test.codex
            .submit(Op::RealtimeConversationAttachStream {
                params: params("reconnect", "ws://unused.invalid"),
                transport: RealtimeSidebandTransport { incoming, outgoing },
                reply,
            })
            .await?;
        timeout(Duration::from_secs(10), ready).await???;
        if attach == 0 {
            events
                .send(json!({"type":"input_transcript.added", "item":{"text":"use the smaller logo"}}).to_string())
                .await?;
            events
                .send(json!({"type":"turn.done", "turn":{"role":"user","transcript":"use the smaller logo"}}).to_string())
                .await?;
            // Detach may discard unsealed deltas; recover only after the frame is durable.
            wait_for_event(&test.codex, |event| {
                matches!(event,
                    EventMsg::RealtimeConversationRealtime(event)
                        if matches!(&event.payload,
                            codex_protocol::protocol::RealtimeEvent::HistoryItemCompleted(item)
                                if matches!(&item.content, RealtimeItemContent::Frame { .. })
                        )
                )
            })
            .await;
        }
        detach(&test, "reconnect").await?;
    }
    end(&test, "reconnect").await?;
    end(&test, "reconnect").await?;
    let items = store
        .load_realtime_session(test.session_configured.thread_id, "reconnect".to_owned())
        .await?
        .expect("started session");
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item.content, RealtimeItemContent::RealtimeSessionStarted))
            .count(),
        1
    );
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(
                item.content,
                RealtimeItemContent::RealtimeSessionClosed { .. }
            ))
            .count(),
        1
    );
    let frame = items
        .iter()
        .find(|item| matches!(item.content, RealtimeItemContent::Frame { .. }))
        .expect("sealed frame");
    assert!(items.iter().any(|item| matches!(&item.content,
        RealtimeItemContent::SessionInputSaved { reason: SessionInputReason::SessionEnd, input, source_event_ids, .. }
            if input.contains("use the smaller logo") && source_event_ids.as_slice() == std::slice::from_ref(&frame.id)
    )));
    let (_events, incoming) = mpsc::channel(8);
    let (outgoing, _commands) = mpsc::channel(8);
    let (reply, ready) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationAttachStream {
            params: params("reconnect", "ws://unused.invalid"),
            transport: RealtimeSidebandTransport { incoming, outgoing },
            reply,
        })
        .await?;
    assert!(timeout(Duration::from_secs(10), ready).await??.is_err());
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

/// An uncertain but committed voice input keeps its place ahead of later typed input on replay.
#[tokio::test]
async fn committed_handoff_retry_preserves_live_and_recovery_input_order() -> Result<()> {
    use super::realtime_conversation::realtime_history_store::HostedClockStore;
    use codex_history::RolloutItem;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use codex_thread_store::LoadThreadHistoryParams;
    use core_test_support::responses;

    let api = start_mock_server().await;
    let model = responses::mount_sse_sequence(
        &api,
        ["voice", "typed"].map(responses::sse_completed).into(),
    )
    .await;
    let store = Arc::new(HostedClockStore::failing_after_input_commit());
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_model_info_override("gpt-5.5", |model| model.use_responses_lite = false)
        .with_thread_store(store.clone())
        .build_with_auto_env(&api)
        .await?;
    let (events, incoming) = mpsc::channel(32);
    let (outgoing, _commands) = mpsc::channel(32);
    let (reply, ready) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationAttachStream {
            params: params("order", "ws://unused.invalid"),
            transport: RealtimeSidebandTransport { incoming, outgoing },
            reply,
        })
        .await?;
    timeout(Duration::from_secs(10), ready).await???;
    events
        .send(json!({"type":"delegation.created","item":{"id":"handoff-A","type":"delegation","target":"client","content":[{"type":"input_text","text":"voice A"}]}}).to_string())
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(store.input_attempts(), 2);
    test.submit_text_turn("typed B").await?;

    let label = |text: &str| {
        if text.contains("<input>voice A</input>") {
            Some("A")
        } else if text == "typed B" {
            Some("B")
        } else {
            None
        }
    };
    let live = model.requests()[1]
        .message_input_texts("user")
        .iter()
        .filter_map(|text| label(text))
        .collect::<Vec<_>>();
    let context = test
        .thread_store
        .load_latest_model_context(LoadThreadHistoryParams {
            thread_id: test.session_configured.thread_id,
            include_archived: true,
        })
        .await?;
    let recovery = context
        .items
        .iter()
        .filter_map(|item| match item {
            RolloutItem::ResponseItem(envelope) => match &envelope.item {
                ResponseItem::Message { role, content, .. } if role == "user" => Some(content),
                _ => None,
            },
            _ => None,
        })
        .flatten()
        .filter_map(|content| match content {
            ContentItem::InputText { text } => label(text),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!((live, recovery), (vec!["A", "B"], vec!["A", "B"]));
    detach(&test, "order").await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

/// A failed close is terminal for the current owner; retrying Stop cannot replay an uncertain write.
#[tokio::test]
async fn failed_close_requires_history_recovery_before_another_call() -> Result<()> {
    use super::realtime_conversation::realtime_history_store::HostedClockStore;
    use codex_core::TurnInputRequest;
    use codex_history::RolloutItem;
    use codex_protocol::error::CodexErrorDetails;
    use codex_protocol::realtime::RealtimeItemContent;
    use codex_protocol::user_input::UserInput;
    use codex_thread_store::ThreadStore;
    use core_test_support::wait_for_event_match;

    let api = start_mock_server().await;
    let store = Arc::new(HostedClockStore::failing_close());
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_thread_store(store.clone())
        .build_with_auto_env(&api)
        .await?;
    let (_events, incoming) = mpsc::channel(32);
    let (outgoing, _commands) = mpsc::channel(32);
    let (reply, ready) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationAttachStream {
            params: params("failed-close", "ws://unused.invalid"),
            transport: RealtimeSidebandTransport { incoming, outgoing },
            reply,
        })
        .await?;
    timeout(Duration::from_secs(10), ready).await???;

    let (reply, result) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationEnd {
            realtime_session_id: "failed-close".to_owned(),
            reply,
        })
        .await?;
    let error = timeout(Duration::from_secs(10), result)
        .await??
        .expect_err("an uncertain close must fail End");
    assert!(
        matches!(error.details(), CodexErrorDetails::Fatal(message) if message == "realtime history recovery required; reload the thread before continuing")
    );
    let closed = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RealtimeConversationClosed(closed) => Some(closed.clone()),
        _ => None,
    })
    .await;
    assert_eq!(closed.reason.as_deref(), Some("history_recovery_required"));

    test.codex.submit(Op::RealtimeConversationClose).await?;
    let repeated = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RealtimeConversationClosed(closed) => Some(closed.clone()),
        _ => None,
    })
    .await;
    assert_eq!(repeated, closed);
    assert_eq!(store.close_attempts(), 1);
    let history = store
        .load_latest_model_context(codex_thread_store::LoadThreadHistoryParams {
            thread_id: test.session_configured.thread_id,
            include_archived: false,
        })
        .await?;
    assert!(!history.items.iter().any(|item| matches!(item,
        RolloutItem::RealtimeItem(item)
            if matches!(item.content, RealtimeItemContent::RealtimeSessionClosed { .. })
    )));

    let (_, result) = start(&test, "next-call", "ws://unused.invalid").await?;
    let error = timeout(Duration::from_secs(10), result)
        .await??
        .expect_err("a new attachment must wait for recovery");
    assert!(matches!(error.details(), CodexErrorDetails::Fatal(_)));
    let error = test
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "continue".to_owned(),
            text_elements: Vec::new(),
        }]))
        .await
        .expect_err("turn input must wait for recovery");
    assert!(matches!(error.details(), CodexErrorDetails::Fatal(_)));
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

struct Connection {
    accept: oneshot::Sender<bool>,
    messages: mpsc::UnboundedReceiver<Value>,
    events: mpsc::UnboundedSender<Value>,
}

struct Connected {
    messages: mpsc::UnboundedReceiver<Value>,
    events: mpsc::UnboundedSender<Value>,
}

impl Sideband {
    async fn new() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("ws://{}", listener.local_addr()?);
        let (connections_tx, connections) = mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let (accept, release) = oneshot::channel();
                let (messages_tx, messages) = mpsc::unbounded_channel();
                let (events, mut events_rx) = mpsc::unbounded_channel::<Value>();
                if connections_tx
                    .send(Connection {
                        accept,
                        messages,
                        events,
                    })
                    .is_err()
                {
                    return;
                }
                tokio::spawn(async move {
                    if release.await != Ok(true) {
                        // Read the handshake before rejecting it so closing the socket
                        // cannot turn the HTTP response into a Windows connection reset.
                        let _ = tokio_tungstenite::accept_hdr_async(
                            stream,
                            |_: &tokio_tungstenite::tungstenite::handshake::server::Request,
                             _: tokio_tungstenite::tungstenite::handshake::server::Response| {
                                Err(http::Response::builder()
                                    .status(http::StatusCode::GONE)
                                    .body(/*body*/ None)
                                    .expect("valid handshake rejection"))
                            },
                        )
                        .await;
                        return;
                    }
                    let Ok(mut socket) = tokio_tungstenite::accept_async(stream).await else {
                        return;
                    };
                    let _ = socket
                        .send(Message::Text(
                            json!({"type":"session.started", "session":{"id":"media-call"}})
                                .to_string()
                                .into(),
                        ))
                        .await;
                    loop {
                        tokio::select! {
                            Some(event) = events_rx.recv() => {
                                if socket.send(Message::Text(event.to_string().into())).await.is_err() { return; }
                            }
                            message = socket.next() => match message {
                                Some(Ok(Message::Text(text))) => { let _ = messages_tx.send(serde_json::from_str(&text).expect("sideband request should be JSON")); }
                                Some(Ok(Message::Close(_))) => { let _ = socket.close(/*msg*/ None).await; return; }
                                Some(Ok(_)) => {}
                                _ => return,
                            },
                        }
                    }
                });
            }
        });
        Ok(Self {
            url,
            connections,
            task,
        })
    }

    async fn connected(&mut self) -> Connection {
        timeout(Duration::from_secs(10), self.connections.recv())
            .await
            .expect("sideband should connect before timeout")
            .expect("sideband listener should remain open")
    }
}

impl Drop for Sideband {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn params(session: &str, url: &str) -> ConversationStartParams {
    ConversationStartParams {
        client_managed_handoffs: false,
        delegation_ack_filler: None,
        flush_transcript_tail_on_session_end: true,
        codex_responses_as_items: false,
        codex_response_item_prefix: None,
        codex_response_handoff_mode: Default::default(),
        backend_reasoning_status: false,
        codex_response_handoff_channel_prefixes: None,
        model: None,
        output_modality: RealtimeOutputModality::Audio,
        include_startup_context: false,
        initial_items: Vec::new(),
        realtime_start_instructions: None,
        realtime_end_instructions: None,
        prompt: None,
        realtime_session_id: Some(session.to_owned()),
        voice_session_id: None,
        transport: Some(ConversationStartTransport::ExistingCall {
            call_id: format!("rtc_{session}"),
            sideband_base_url: Some(url.to_owned()),
        }),
        version: Some(RealtimeConversationVersion::V3),
    }
}

async fn start(
    test: &TestCodex,
    session: &str,
    url: &str,
) -> Result<(String, oneshot::Receiver<codex_protocol::error::Result<()>>)> {
    let params = params(session, url);
    let (reply, result) = oneshot::channel();
    let id = test
        .codex
        .submit(Op::RealtimeConversationAttach {
            params,
            reply,
            options: RealtimeAttachmentOptions::default(),
        })
        .await?;
    Ok((id, result))
}

async fn detach(test: &TestCodex, session: &str) -> Result<()> {
    let (reply, result) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationDetach {
            realtime_session_id: session.to_owned(),
            reply,
        })
        .await?;
    timeout(Duration::from_secs(10), result).await???;
    Ok(())
}

async fn end(test: &TestCodex, session: &str) -> Result<()> {
    timeout(Duration::from_secs(10), async {
        loop {
            let (reply, result) = oneshot::channel();
            test.codex
                .submit(Op::RealtimeConversationEnd {
                    realtime_session_id: session.to_owned(),
                    reply,
                })
                .await?;
            match result.await? {
                Ok(()) => return anyhow::Ok(()),
                Err(error) if is_realtime_busy(&error) => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => return Err(error.into()),
            }
        }
    })
    .await??;
    Ok(())
}

fn is_realtime_busy(error: &codex_protocol::error::CodexErr) -> bool {
    matches!(error.details(), codex_protocol::error::CodexErrorDetails::Io(error)
        if error.kind() == std::io::ErrorKind::WouldBlock)
}

async fn attached(
    test: &TestCodex,
    server: &mut Sideband,
    session: &str,
) -> Result<(String, Connected)> {
    let (id, result) = start(test, session, &server.url).await?;
    let connection = server.connected().await;
    connection
        .accept
        .send(/*value*/ true)
        .expect("candidate should still be waiting for its handshake");
    timeout(Duration::from_secs(10), result).await???;
    Ok((
        id,
        Connected {
            messages: connection.messages,
            events: connection.events,
        },
    ))
}

async fn assert_text_routes_to(test: &TestCodex, connection: &mut Connected) -> Result<()> {
    test.codex
        .submit(Op::RealtimeConversationText(ConversationTextParams {
            text: "still selected".to_owned(),
            role: ConversationTextRole::User,
        }))
        .await?;
    timeout(Duration::from_secs(10), async {
        while let Some(message) = connection.messages.recv().await {
            if message.to_string().contains("still selected") {
                return;
            }
        }
        panic!("selected sideband closed");
    })
    .await?;
    Ok(())
}

/// Cancellation releases a held handshake and obsolete queued work before selecting the next call.
#[tokio::test]
async fn cancelled_attachments_release_the_queue_and_preserve_the_next_call() -> Result<()> {
    let api = start_mock_server().await;
    let test = test_codex().build_with_auto_env(&api).await?;
    let mut original = Sideband::new().await?;
    let mut newer = Sideband::new().await?;
    let (cancel_original, cancellation) = watch::channel(false);
    let (reply, original_result) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationAttach {
            params: params("original", &original.url),
            reply,
            options: RealtimeAttachmentOptions {
                cancel: Some(cancellation),
                start_before: None,
                ..Default::default()
            },
        })
        .await?;
    // Keep the first upgrade pending until the replacement is already usable.
    let mut original_connection = original.connected().await;

    let (cancel_queued, cancellation) = watch::channel(false);
    let (reply, queued_result) = oneshot::channel();
    cancel_queued.send_replace(/*value*/ true);
    test.codex
        .submit(Op::RealtimeConversationAttach {
            params: params("queued", &newer.url),
            reply,
            options: RealtimeAttachmentOptions {
                cancel: Some(cancellation),
                start_before: None,
                ..Default::default()
            },
        })
        .await?;
    let (_cancel_expired, cancellation) = watch::channel(false);
    let (reply, expired_result) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationAttach {
            params: params("expired", &newer.url),
            reply,
            options: RealtimeAttachmentOptions {
                cancel: Some(cancellation),
                start_before: Some(SystemTime::now() - Duration::from_secs(1)),
                ..Default::default()
            },
        })
        .await?;
    let (cancel_newer, cancellation) = watch::channel(false);
    let (reply, newer_result) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationAttach {
            params: params("newer", &newer.url),
            reply,
            options: RealtimeAttachmentOptions {
                cancel: Some(cancellation),
                start_before: None,
                ..Default::default()
            },
        })
        .await?;
    // Losing the caller's cancellation sender does not cancel admitted work.
    drop(cancel_newer);
    cancel_original.send_replace(/*value*/ true);
    assert!(
        timeout(Duration::from_secs(10), original_result)
            .await??
            .is_err()
    );
    assert!(
        timeout(Duration::from_secs(10), queued_result)
            .await??
            .is_err()
    );
    assert!(
        timeout(Duration::from_secs(10), expired_result)
            .await??
            .is_err()
    );

    let connection = newer.connected().await;
    connection
        .accept
        .send(/*value*/ true)
        .expect("handshake pending");
    timeout(Duration::from_secs(10), newer_result).await???;
    let started = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::RealtimeConversationStarted(_))
    })
    .await;
    let EventMsg::RealtimeConversationStarted(started) = started else {
        unreachable!("wait_for_event selected the Started event");
    };
    assert_eq!(started.realtime_session_id.as_deref(), Some("newer"));
    assert_eq!(started.version, RealtimeConversationVersion::V3);
    assert!(matches!(
        newer.connections.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
    original_connection
        .accept
        .send(/*value*/ true)
        .expect("original upgrade remains held until cancellation completes");
    assert_eq!(
        timeout(Duration::from_secs(10), original_connection.messages.recv()).await?,
        None
    );
    detach(&test, "original").await?;
    let mut current = Connected {
        messages: connection.messages,
        events: connection.events,
    };
    assert_text_routes_to(&test, &mut current).await?;
    detach(&test, "newer").await?;
    assert_eq!(
        timeout(Duration::from_secs(10), current.messages.recv()).await?,
        None
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

/// A deadline without a cancellation receiver releases a held handshake so the next call can connect.
#[tokio::test]
async fn attachment_deadline_releases_a_pending_handshake() -> Result<()> {
    let api = start_mock_server().await;
    let test = test_codex().build_with_auto_env(&api).await?;
    let mut sideband = Sideband::new().await?;
    let (reply, result) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationAttach {
            params: params("expired", &sideband.url),
            reply,
            options: RealtimeAttachmentOptions {
                cancel: None,
                start_before: Some(SystemTime::now() + Duration::from_secs(60)),
                ..Default::default()
            },
        })
        .await?;
    let mut connection = sideband.connected().await;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(61)).await;
    let expired = timeout(Duration::from_secs(10), result).await;
    tokio::time::resume();
    assert!(expired??.is_err());
    connection
        .accept
        .send(/*value*/ true)
        .expect("handshake still held");
    assert_eq!(
        timeout(Duration::from_secs(10), connection.messages.recv()).await?,
        None
    );
    let (_, mut current) = attached(&test, &mut sideband, "current").await?;
    assert_text_routes_to(&test, &mut current).await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

/// Starts are sequential; completion follows the handshake and a delayed stop A leaves B usable.
pub(super) async fn attachment_replacement_scenario() -> Result<Vec<ResponsesRequest>> {
    let api = start_mock_server().await;
    let response = core_test_support::responses::mount_sse_once(
        &api,
        core_test_support::responses::sse(vec![
            core_test_support::responses::ev_response_created("handoff"),
            core_test_support::responses::ev_assistant_message("answer", "Selected call handled"),
            core_test_support::responses::ev_completed("handoff"),
        ]),
    )
    .await;
    let mut builder = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_model_info_override("gpt-5.5", |model| {
            model.use_responses_lite = false;
        })
        .with_config(|config| {
            config.base_instructions = Some(String::new());
            config
                .features
                .enable(Feature::RealtimeFramesLocal)
                .expect("enable local realtime frames");
        });
    let test = builder.build_with_auto_env(&api).await?;
    let mut original = Sideband::new().await?;
    let mut newer = Sideband::new().await?;
    let (original_id, mut original_messages) = attached(&test, &mut original, "original").await?;
    let (_, busy) = start(&test, "newer", &newer.url).await?;
    assert!(is_realtime_busy(
        &timeout(Duration::from_secs(10), busy)
            .await??
            .expect_err("replacement settles first")
    ));
    end(&test, "original").await?;
    let (newer_id, mut completion) = start(&test, "newer", &newer.url).await?;
    let connection = newer.connected().await;
    assert_eq!(
        timeout(Duration::from_secs(10), original_messages.messages.recv()).await?,
        None
    );
    assert!(matches!(
        completion.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    let (reply, stopped) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationDetach {
            realtime_session_id: "original".to_owned(),
            reply,
        })
        .await?;
    connection.accept.send(true).expect("handshake pending");
    timeout(Duration::from_secs(10), completion).await???;
    timeout(Duration::from_secs(10), stopped).await???;
    detach(&test, "original").await?;
    let mut messages = Connected {
        messages: connection.messages,
        events: connection.events,
    };
    assert_text_routes_to(&test, &mut messages).await?;
    messages.events.send(json!({"type":"delegation.created", "offset_ms": 0, "item": {"id":"handoff", "type":"delegation", "target":"client", "content":[{"type":"input_text", "text":"selected connection handoff"}]}}))?;
    let lifecycle = timeout(Duration::from_secs(10), async {
        let mut lifecycle = Vec::new();
        loop {
            let event = test.codex.next_event().await?;
            match event.msg {
                EventMsg::RealtimeConversationStarted(_) => lifecycle.push((event.id, "started")),
                EventMsg::RealtimeConversationClosed(_) => lifecycle.push((event.id, "closed")),
                EventMsg::TurnComplete(_) => break,
                _ => {}
            }
        }
        anyhow::Ok(lifecycle)
    })
    .await??;
    assert_eq!(
        lifecycle,
        vec![
            (original_id.clone(), "started"),
            (original_id, "closed"),
            (newer_id, "started")
        ]
    );
    test.codex.shutdown_and_wait().await?;
    Ok(response.requests())
}

/// A rejected replacement closes A and reports the failure; a later attempt can connect.
#[tokio::test]
async fn failed_attachment_leaves_core_disconnected() -> Result<()> {
    let api = start_mock_server().await;
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_config(|config| {
            config
                .features
                .enable(Feature::RealtimeFramesLocal)
                .expect("enable local realtime frames");
        })
        .build_with_auto_env(&api)
        .await?;
    let mut original = Sideband::new().await?;
    let mut newer = Sideband::new().await?;
    let (_, mut current) = attached(&test, &mut original, "original").await?;
    let (_, busy) = start(&test, "newer", &newer.url).await?;
    assert!(timeout(Duration::from_secs(10), busy).await??.is_err());
    end(&test, "original").await?;
    let (_, result) = start(&test, "newer", &newer.url).await?;
    let connection = newer.connected().await;
    assert_eq!(
        timeout(Duration::from_secs(10), current.messages.recv()).await?,
        None
    );
    connection.accept.send(false).expect("handshake pending");
    assert!(timeout(Duration::from_secs(10), result).await??.is_err());
    let (_, mut retry) = attached(&test, &mut newer, "newer").await?;
    assert_text_routes_to(&test, &mut retry).await?;
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[test_case::test_case(ThreadHistoryMode::Legacy, true; "legacy record unfinished tail")]
#[test_case::test_case(ThreadHistoryMode::Legacy, false; "legacy setting ignored for v3")]
#[test_case::test_case(ThreadHistoryMode::Paginated, true; "frames record unfinished tail")]
#[tokio::test]
async fn completed_turns_reach_history_before_the_next_handoff(
    history_mode: ThreadHistoryMode,
    flush_tail: bool,
) -> Result<()> {
    use codex_protocol::protocol::RealtimeEvent;
    use core_test_support::responses;
    use core_test_support::wait_for_event;

    let api = start_mock_server().await;
    let replies = ["handoff", "text"].map(|id| {
        responses::sse(vec![
            responses::ev_response_created(id),
            responses::ev_completed(id),
        ])
    });
    let model = responses::mount_sse_sequence(&api, replies.into()).await;
    let test = test_codex()
        .with_history_mode(history_mode)
        .build_with_auto_env(&api)
        .await?;
    let mut sideband = Sideband::new().await?;
    let mut params = params("transcript", &sideband.url);
    params.flush_transcript_tail_on_session_end = flush_tail;
    let (reply, attached) = oneshot::channel();
    test.codex
        .submit(Op::RealtimeConversationAttach {
            params,
            reply,
            options: RealtimeAttachmentOptions::default(),
        })
        .await?;
    let connection = sideband.connected().await;
    connection
        .accept
        .send(/*value*/ true)
        .expect("pending handshake");
    timeout(Duration::from_secs(10), attached).await???;
    let send = |event| connection.events.send(event);

    let speech = format!("{}&", "u".repeat(12 * 1024));
    let final_text = format!("{speech}!");
    send(json!({"type":"input_transcript.added", "item":{"text":speech}}))?;
    send(json!({"type":"turn.done", "turn":{"role":"assistant","transcript":"yes"}}))?;
    send(json!({"type":"turn.done", "turn":{"role":"user","transcript":final_text}}))?;
    wait_for_event(&test.codex, |event| matches!(event, EventMsg::RealtimeConversationRealtime(event) if matches!(event.payload, RealtimeEvent::InputTranscriptDone(_)))).await;
    assert!(model.requests().is_empty());

    // The handoff repeats the user input. Only the new partial belongs in its transcript.
    let partial = "p".repeat(12 * 1024);
    send(json!({"type":"output_transcript.added", "item":{"text":partial}}))?;
    send(
        json!({"type":"delegation.created","item":{"id":"handoff","type":"delegation","target":"client","content":[{"type":"input_text","text":final_text}]}}),
    )?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let transcript = |request: ResponsesRequest| {
        request
            .message_input_texts("user")
            .into_iter()
            .filter(|text| text.contains("<transcript_delta>"))
            .collect::<Vec<_>>()
    };
    let completed = transcript(model.single_request());
    assert_eq!(completed.len(), 2);
    let expected = format!(
        "<transcript_delta>user: {}\nassistant: yes</transcript_delta>",
        final_text.replace('&', "&amp;")
    );
    assert!(completed[0].contains("<input></input>") && completed[0].contains(&expected));
    assert!(completed[1].contains(&format!(
        "<transcript_delta>assistant: {partial}</transcript_delta>"
    )));

    let unfinished = "a".repeat(12 * 1024);
    send(json!({"type":"output_transcript.added", "item":{"text":unfinished}}))?;
    wait_for_event(&test.codex, |event| matches!(event, EventMsg::RealtimeConversationRealtime(event) if matches!(event.payload, RealtimeEvent::OutputTranscriptDelta(_)))).await;
    end(&test, "transcript").await?;
    test.submit_text_turn("continue").await?;
    let after_close = transcript(model.last_request().expect("text turn"));
    assert_eq!(after_close.len(), completed.len() + 1);
    assert_eq!(&after_close[..completed.len()], completed.as_slice());
    assert!(after_close[completed.len()].contains(&format!(
        "<transcript_delta>assistant: {unfinished}</transcript_delta>"
    )));
    Ok(())
}
