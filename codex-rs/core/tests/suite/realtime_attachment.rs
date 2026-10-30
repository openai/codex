//! Exercises attachment races through Core submissions and real sideband sockets.

use anyhow::Result;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::ConversationStartTransport;
use codex_protocol::protocol::ConversationTextParams;
use codex_protocol::protocol::ConversationTextRole;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RealtimeConversationVersion;
use codex_protocol::protocol::RealtimeOutputModality;
use codex_protocol::protocol::ThreadHistoryMode;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;

struct Sideband {
    url: String,
    connections: mpsc::UnboundedReceiver<Connection>,
    task: tokio::task::JoinHandle<()>,
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
        .submit(Op::RealtimeConversationAttach { params, reply })
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
        .submit(Op::RealtimeConversationAttach { params, reply })
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
