use std::sync::Arc;

use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::NewConversation;
use codex_core::config::Config;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

/// Spawn the agent bootstrapper and op forwarding loop, returning the
/// `UnboundedSender<Op>` used by the UI to submit operations.
pub(crate) fn spawn_agent(
    config: Config,
    app_event_tx: AppEventSender,
    server: Arc<ConversationManager>,
) -> UnboundedSender<Op> {
    let (codex_op_tx, mut codex_op_rx) = unbounded_channel::<Op>();

    let app_event_tx_clone = app_event_tx;
    tokio::spawn(async move {
        let NewConversation {
            conversation_id: _,
            conversation,
            session_configured,
        } = match server.new_conversation(config).await {
            Ok(v) => v,
            #[allow(clippy::print_stderr)]
            Err(err) => {
                let message = err.to_string();
                eprintln!("{message}");
                app_event_tx_clone.send(AppEvent::CodexEvent(Event {
                    id: "".to_string(),
                    msg: EventMsg::Error(err.to_error_event(None)),
                }));
                app_event_tx_clone.send(AppEvent::ExitRequest);
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

        let conversation_clone = conversation.clone();
        tokio::spawn(async move {
            while let Some(op) = codex_op_rx.recv().await {
                let id = conversation_clone.submit(op).await;
                if let Err(e) = id {
                    tracing::error!("failed to submit op: {e}");
                }
            }
        });

        while let Ok(event) = conversation.next_event().await {
            app_event_tx_clone.send(AppEvent::CodexEvent(event));
        }
    });

    codex_op_tx
}

/// Spawn agent loops for an existing conversation (e.g., a forked conversation).
/// Sends the provided `SessionConfiguredEvent` immediately, then forwards subsequent
/// events and accepts Ops for submission.
pub(crate) fn spawn_agent_from_existing(
    conversation: std::sync::Arc<CodexConversation>,
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

        let conversation_clone = conversation.clone();
        tokio::spawn(async move {
            while let Some(op) = codex_op_rx.recv().await {
                let id = conversation_clone.submit(op).await;
                if let Err(e) = id {
                    tracing::error!("failed to submit op: {e}");
                }
            }
        });

        while let Ok(event) = conversation.next_event().await {
            app_event_tx_clone.send(AppEvent::CodexEvent(event));
        }
    });

    codex_op_tx
}
