use std::sync::Arc;

use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::NewConversation;
use codex_core::config::Config;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SessionConfiguredEvent;
use codex_protocol::ConversationId;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::mpsc::unbounded_channel;

pub(crate) struct AgentHandles {
    pub conversation_id: ConversationId,
    pub op_tx: UnboundedSender<Op>,
    pub event_rx: UnboundedReceiver<Event>,
}

fn spawn_op_forwarder(conversation: Arc<CodexConversation>) -> UnboundedSender<Op> {
    let (codex_op_tx, mut codex_op_rx) = unbounded_channel::<Op>();
    tokio::spawn(async move {
        while let Some(op) = codex_op_rx.recv().await {
            if let Err(err) = conversation.submit(op).await {
                tracing::error!("failed to submit op: {err}");
            }
        }
    });
    codex_op_tx
}

fn spawn_event_forwarder(
    conversation: Arc<CodexConversation>,
    session_configured: Arc<SessionConfiguredEvent>,
) -> UnboundedReceiver<Event> {
    let (event_tx, event_rx) = unbounded_channel::<Event>();
    tokio::spawn(async move {
        let initial = Event {
            id: String::new(),
            msg: EventMsg::SessionConfigured((*session_configured).clone()),
        };
        if event_tx.send(initial).is_err() {
            return;
        }

        loop {
            match conversation.next_event().await {
                Ok(event) => {
                    if event_tx.send(event).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    tracing::error!("failed to receive conversation event: {err}");
                    break;
                }
            }
        }
    });
    event_rx
}

/// Spawn the agent bootstrapper and op forwarding loop.
pub(crate) async fn spawn_agent(
    config: Config,
    server: Arc<ConversationManager>,
) -> Result<AgentHandles> {
    let NewConversation {
        conversation_id,
        conversation,
        session_configured,
    } = server
        .new_conversation(config)
        .await
        .wrap_err("failed to start Codex conversation")?;

    let session_configured = Arc::new(session_configured);
    let op_tx = spawn_op_forwarder(conversation.clone());
    let event_rx = spawn_event_forwarder(conversation, session_configured);

    Ok(AgentHandles {
        conversation_id,
        op_tx,
        event_rx,
    })
}

/// Spawn agent loops for an existing conversation (e.g., a forked conversation).
pub(crate) fn spawn_agent_from_existing(
    conversation: Arc<CodexConversation>,
    session_configured: SessionConfiguredEvent,
) -> AgentHandles {
    let conversation_id = session_configured.session_id;
    let session_configured = Arc::new(session_configured);
    let op_tx = spawn_op_forwarder(conversation.clone());
    let event_rx = spawn_event_forwarder(conversation, session_configured);

    AgentHandles {
        conversation_id,
        op_tx,
        event_rx,
    }
}

pub(crate) fn handles_from_existing_with_events(
    conversation: Arc<CodexConversation>,
    session_configured: Arc<SessionConfiguredEvent>,
    event_rx: UnboundedReceiver<Event>,
) -> AgentHandles {
    let conversation_id = session_configured.session_id;
    let op_tx = spawn_op_forwarder(conversation);
    AgentHandles {
        conversation_id,
        op_tx,
        event_rx,
    }
}
