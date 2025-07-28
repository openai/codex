use std::sync::Arc;

use crate::Codex;
use crate::CodexSpawnOk;
use crate::config::Config;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::util::notify_on_sigint;
use tokio::sync::Notify;
use uuid::Uuid;

/// Represents an active Codex conversation, including the first event
/// (which is [`EventMsg::SessionConfigured`]).
pub struct CodexConversation {
    pub codex: Codex,
    pub session_id: Uuid,
    pub session_configured: Event,
    pub ctrl_c: Arc<Notify>,
}

/// Spawn a new [`Codex`] and initialize the session.
///
/// Returns the wrapped [`Codex`] **and** the `SessionInitialized` event that
/// is received as a response to the initial `ConfigureSession` submission so
/// that callers can surface the information to the UI.
pub async fn init_codex(config: Config) -> anyhow::Result<CodexConversation> {
    let ctrl_c = notify_on_sigint();
    let CodexSpawnOk {
        codex,
        init_id,
        session_id,
    } = Codex::spawn(config, ctrl_c.clone()).await?;

    // The first event must be `SessionInitialized`. Validate and forward it to
    // the caller so that they can display it in the conversation history.
    let event = codex.next_event().await?;
    if event.id != init_id
        || !matches!(
            &event,
            Event {
                id: _id,
                msg: EventMsg::SessionConfigured(_),
            }
        )
    {
        return Err(anyhow::anyhow!(
            "expected SessionInitialized but got {event:?}"
        ));
    }

    Ok(CodexConversation {
        codex,
        session_id,
        session_configured: event,
        ctrl_c,
    })
}
