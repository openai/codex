use std::sync::Arc;

use codex_core::CodexThread;
use codex_core::NewThread;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::unbounded_channel;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

const REALTIME_MIC_AUDIO_QUEUE_CAPACITY: usize = 64;

pub(crate) struct ChatWidgetOpSenders {
    pub(crate) codex_op_tx: UnboundedSender<Op>,
    pub(crate) realtime_audio_op_tx: Sender<Op>,
}

/// Spawn the agent bootstrapper and op forwarding loop, returning the
/// `UnboundedSender<Op>` used by the UI to submit operations.
pub(crate) fn spawn_agent(
    config: Config,
    app_event_tx: AppEventSender,
    server: Arc<ThreadManager>,
) -> ChatWidgetOpSenders {
    let (codex_op_tx, mut codex_op_rx) = unbounded_channel::<Op>();
    let (realtime_audio_op_tx, realtime_audio_op_rx) =
        channel::<Op>(REALTIME_MIC_AUDIO_QUEUE_CAPACITY);

    let app_event_tx_clone = app_event_tx;
    tokio::spawn(async move {
        let NewThread {
            thread,
            session_configured,
            ..
        } = match server.start_thread(config).await {
            Ok(v) => v,
            Err(err) => {
                let message = format!("Failed to initialize codex: {err}");
                tracing::error!("{message}");
                app_event_tx_clone.send(AppEvent::CodexEvent(Event {
                    id: "".to_string(),
                    msg: EventMsg::Error(err.to_error_event(None)),
                }));
                app_event_tx_clone.send(AppEvent::FatalExitRequest(message));
                tracing::error!("failed to initialize codex: {err}");
                return;
            }
        };

        // Forward the captured `SessionConfigured` event so it can be rendered in the UI.
        let ev = codex_protocol::protocol::Event {
            // The `id` does not matter for rendering, so we can use a fake value.
            id: "".to_string(),
            msg: codex_protocol::protocol::EventMsg::SessionConfigured(session_configured),
        };
        app_event_tx_clone.send(AppEvent::CodexEvent(ev));

        let thread_clone = thread.clone();
        spawn_bounded_op_forwarder(thread.clone(), realtime_audio_op_rx);
        tokio::spawn(async move {
            while let Some(op) = codex_op_rx.recv().await {
                let id = thread_clone.submit(op).await;
                if let Err(e) = id {
                    tracing::error!("failed to submit op: {e}");
                }
            }
        });

        while let Ok(event) = thread.next_event().await {
            let is_shutdown_complete = matches!(event.msg, EventMsg::ShutdownComplete);
            app_event_tx_clone.send(AppEvent::CodexEvent(event));
            if is_shutdown_complete {
                // ShutdownComplete is terminal for a thread; drop this receiver task so
                // the Arc<CodexThread> can be released and thread resources can clean up.
                break;
            }
        }
    });

    ChatWidgetOpSenders {
        codex_op_tx,
        realtime_audio_op_tx,
    }
}

/// Spawn agent loops for an existing thread (e.g., a forked thread).
/// Sends the provided `SessionConfiguredEvent` immediately, then forwards subsequent
/// events and accepts Ops for submission.
pub(crate) fn spawn_agent_from_existing(
    thread: std::sync::Arc<CodexThread>,
    session_configured: codex_protocol::protocol::SessionConfiguredEvent,
    app_event_tx: AppEventSender,
) -> ChatWidgetOpSenders {
    let (codex_op_tx, mut codex_op_rx) = unbounded_channel::<Op>();
    let (realtime_audio_op_tx, realtime_audio_op_rx) =
        channel::<Op>(REALTIME_MIC_AUDIO_QUEUE_CAPACITY);

    let app_event_tx_clone = app_event_tx;
    tokio::spawn(async move {
        // Forward the captured `SessionConfigured` event so it can be rendered in the UI.
        let ev = codex_protocol::protocol::Event {
            id: "".to_string(),
            msg: codex_protocol::protocol::EventMsg::SessionConfigured(session_configured),
        };
        app_event_tx_clone.send(AppEvent::CodexEvent(ev));

        let thread_clone = thread.clone();
        spawn_bounded_op_forwarder(thread.clone(), realtime_audio_op_rx);
        tokio::spawn(async move {
            while let Some(op) = codex_op_rx.recv().await {
                let id = thread_clone.submit(op).await;
                if let Err(e) = id {
                    tracing::error!("failed to submit op: {e}");
                }
            }
        });

        while let Ok(event) = thread.next_event().await {
            let is_shutdown_complete = matches!(event.msg, EventMsg::ShutdownComplete);
            app_event_tx_clone.send(AppEvent::CodexEvent(event));
            if is_shutdown_complete {
                // ShutdownComplete is terminal for a thread; drop this receiver task so
                // the Arc<CodexThread> can be released and thread resources can clean up.
                break;
            }
        }
    });

    ChatWidgetOpSenders {
        codex_op_tx,
        realtime_audio_op_tx,
    }
}

/// Spawn an op-forwarding loop for an existing thread without subscribing to events.
pub(crate) fn spawn_op_forwarder(thread: std::sync::Arc<CodexThread>) -> ChatWidgetOpSenders {
    let (codex_op_tx, mut codex_op_rx) = unbounded_channel::<Op>();
    let (realtime_audio_op_tx, realtime_audio_op_rx) =
        channel::<Op>(REALTIME_MIC_AUDIO_QUEUE_CAPACITY);

    spawn_bounded_op_forwarder(thread.clone(), realtime_audio_op_rx);
    tokio::spawn(async move {
        while let Some(op) = codex_op_rx.recv().await {
            if let Err(e) = thread.submit(op).await {
                tracing::error!("failed to submit op: {e}");
            }
        }
    });

    ChatWidgetOpSenders {
        codex_op_tx,
        realtime_audio_op_tx,
    }
}

fn spawn_bounded_op_forwarder(thread: std::sync::Arc<CodexThread>, mut op_rx: Receiver<Op>) {
    tokio::spawn(async move {
        while let Some(op) = op_rx.recv().await {
            if let Err(e) = thread.submit(op).await {
                tracing::error!("failed to submit realtime audio op: {e}");
            }
        }
    });
}
