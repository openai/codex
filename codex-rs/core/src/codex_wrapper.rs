use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use crate::config::Config;
use crate::protocol::AskForApproval;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::Op;
use crate::protocol::SandboxPolicy;
use crate::protocol::Submission;
use crate::util::notify_on_sigint;
use crate::Codex;
use tokio::sync::Notify;
use tracing::debug;

/// Spawn a new [`Codex`] and initialise the session.
///
/// Returns the wrapped [`Codex`] **and** the `SessionInitialized` event that
/// is received as a response to the initial `ConfigureSession` submission so
/// that callers can surface the information to the UI.
pub async fn init_codex(
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
    model_override: Option<String>,
) -> anyhow::Result<(CodexWrapper, Event, Arc<Notify>)> {
    let ctrl_c = notify_on_sigint();
    let config = Config::load().unwrap_or_default();
    debug!("loaded config: {config:?}");
    let codex = CodexWrapper::new(Codex::spawn(ctrl_c.clone())?);
    let init_id = codex
        .submit(Op::ConfigureSession {
            model: model_override.or_else(|| config.model.clone()),
            instructions: config.instructions,
            approval_policy,
            sandbox_policy,
        })
        .await?;

    // The first event must be `SessionInitialized`. Validate and forward it to
    // the caller so that they can display it in the conversation history.
    let event = codex.next_event().await?;
    if event.id != init_id
        || !matches!(
            &event,
            Event {
                id: _id,
                msg: EventMsg::SessionConfigured { .. },
            }
        )
    {
        return Err(anyhow::anyhow!(
            "expected SessionInitialized but got {event:?}"
        ));
    }

    Ok((codex, event, ctrl_c))
}

pub struct CodexWrapper {
    next_id: AtomicU64,
    codex: Codex,
}

impl CodexWrapper {
    fn new(codex: Codex) -> Self {
        Self {
            next_id: AtomicU64::new(0),
            codex,
        }
    }

    /// Returns the id of the Submission.
    pub async fn submit(&self, op: Op) -> crate::error::Result<String> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            .to_string();
        self.codex.submit(Submission { id: id.clone(), op }).await?;
        Ok(id)
    }

    pub async fn next_event(&self) -> crate::error::Result<Event> {
        self.codex.next_event().await
    }
}
