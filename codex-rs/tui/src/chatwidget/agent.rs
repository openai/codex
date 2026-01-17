//! Agent bootstrap and event forwarding glue for the TUI chat widget.
//!
//! This module bridges the UI event loop with the async Codex runtime. It
//! spawns background tasks that start or attach to a [`CodexThread`], forwards
//! UI-originated [`Op`] messages into the thread, and relays incoming protocol
//! [`Event`]s back to the UI via [`AppEvent`]. It owns no long-lived state of
//! its own; instead it wires together channels and tasks so the UI remains
//! responsive while the agent runs.
//!
//! Correctness relies on forwarding the initial `SessionConfigured` event
//! before normal event streaming begins so the UI can render session metadata
//! consistently, and on reporting initialization failures as fatal exits so the
//! UI can shut down cleanly. Event forwarding is best-effort and unbuffered at
//! the protocol level; the UI is responsible for ordering and deduping if it
//! aggregates multiple sources.
//!
//! The op channel is intentionally unbounded to avoid blocking the UI thread;
//! callers must drop the sender to shut the submission task down.
use std::sync::Arc;

use codex_core::CodexThread;
use codex_core::NewThread;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

/// Spawns the agent bootstrapper and op-forwarding loop for a new thread.
///
/// The returned sender is used by the UI to submit [`Op`]s. A background task
/// initializes the thread via [`ThreadManager::start_thread`], forwards the
/// resulting `SessionConfigured` event, and then streams events to the UI. A
/// second task drains the op channel and submits each op to the thread.
///
/// The op channel is unbounded and the submission loop runs until the channel is
/// closed, so callers must drop the sender when the UI is shutting down.
pub(crate) fn spawn_agent(
    config: Config,
    app_event_tx: AppEventSender,
    server: Arc<ThreadManager>,
) -> UnboundedSender<Op> {
    let (codex_op_tx, mut codex_op_rx) = unbounded_channel::<Op>();

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
                // Surface the error to the UI and request a clean shutdown.
                app_event_tx_clone.send(AppEvent::FatalExitRequest(message));
                tracing::error!("failed to initialize codex: {err}");
                return;
            }
        };

        // Forward the captured `SessionConfigured` event so it can be rendered in the UI.
        let ev = codex_core::protocol::Event {
            // The `id` does not matter for rendering, so we can use a fake value.
            id: "".to_string(),
            msg: codex_core::protocol::EventMsg::SessionConfigured(session_configured),
        };
        app_event_tx_clone.send(AppEvent::CodexEvent(ev));

        let thread_clone = thread.clone();
        tokio::spawn(async move {
            while let Some(op) = codex_op_rx.recv().await {
                let id = thread_clone.submit(op).await;
                if let Err(e) = id {
                    tracing::error!("failed to submit op: {e}");
                }
            }
        });

        while let Ok(event) = thread.next_event().await {
            app_event_tx_clone.send(AppEvent::CodexEvent(event));
        }
    });

    codex_op_tx
}

/// Spawns agent loops for an existing thread (for example, a forked thread).
///
/// The provided `SessionConfiguredEvent` is forwarded immediately so the UI can
/// render session metadata, then subsequent protocol events are streamed while
/// the op channel is drained and submitted to the thread.
///
/// This mirrors [`spawn_agent`] but skips thread creation; it assumes the
/// caller already owns a live [`CodexThread`].
pub(crate) fn spawn_agent_from_existing(
    thread: std::sync::Arc<CodexThread>,
    session_configured: codex_core::protocol::SessionConfiguredEvent,
    app_event_tx: AppEventSender,
) -> UnboundedSender<Op> {
    let (codex_op_tx, mut codex_op_rx) = unbounded_channel::<Op>();

    let app_event_tx_clone = app_event_tx;
    tokio::spawn(async move {
        // Forward the captured `SessionConfigured` event so it can be rendered in the UI.
        let ev = codex_core::protocol::Event {
            id: "".to_string(),
            msg: codex_core::protocol::EventMsg::SessionConfigured(session_configured),
        };
        app_event_tx_clone.send(AppEvent::CodexEvent(ev));

        let thread_clone = thread.clone();
        tokio::spawn(async move {
            while let Some(op) = codex_op_rx.recv().await {
                let id = thread_clone.submit(op).await;
                if let Err(e) = id {
                    tracing::error!("failed to submit op: {e}");
                }
            }
        });

        while let Ok(event) = thread.next_event().await {
            app_event_tx_clone.send(AppEvent::CodexEvent(event));
        }
    });

    codex_op_tx
}
