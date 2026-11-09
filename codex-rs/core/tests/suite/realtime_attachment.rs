//! Exercises attachment races through Core submissions and real sideband sockets.

use anyhow::Result;
use codex_api::RealtimeEvent;
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
    let test = test_codex().build_with_auto_env(&api).await?;
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

/// Detach is a history barrier for accepted final speech, including when both caller halves disappear.
#[tokio::test(flavor = "current_thread")]
async fn initiated_sideband_detach_preserves_queued_transcript_in_history() -> Result<()> {
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
        // Completed turns must persist even when the optional unfinished-tail flush is off.
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
        drop(events);
        drop(commands);
        detach(&test, &session).await?;
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
            "detach must persist the accepted final transcript exactly once"
        );
    }
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
        flush_transcript_tail_on_session_end: false,
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
        voice: None,
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
        });
    let test = builder.build_with_auto_env(&api).await?;
    let mut original = Sideband::new().await?;
    let mut newer = Sideband::new().await?;
    let (original_id, mut original_messages) = attached(&test, &mut original, "original").await?;
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
    let test = test_codex().build_with_auto_env(&api).await?;
    let mut original = Sideband::new().await?;
    let mut newer = Sideband::new().await?;
    let (_, mut current) = attached(&test, &mut original, "original").await?;
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

#[test_case::test_case(true; "record unfinished tail")]
#[test_case::test_case(false; "drop unfinished tail")]
#[tokio::test]
async fn completed_turns_reach_history_before_the_next_handoff(flush_tail: bool) -> Result<()> {
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
    let test = test_codex().build_with_auto_env(&api).await?;
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
    detach(&test, "transcript").await?;
    test.submit_text_turn("continue").await?;
    let after_close = transcript(model.last_request().expect("text turn"));
    assert_eq!(after_close.len(), completed.len() + usize::from(flush_tail));
    assert_eq!(&after_close[..completed.len()], completed.as_slice());
    if flush_tail {
        assert!(after_close[completed.len()].contains(&format!(
            "<transcript_delta>assistant: {unfinished}</transcript_delta>"
        )));
    }
    Ok(())
}
