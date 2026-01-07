use crate::agent::AgentStatus;
use crate::codex::Codex;
use crate::error::Result as CodexResult;
use crate::protocol::Event;
use crate::protocol::Op;
use crate::protocol::SessionConfiguredEvent;
use crate::protocol::Submission;
use std::path::PathBuf;

pub struct CodexThread {
    codex: Codex,
    rollout_path: PathBuf,
    session_configured: SessionConfiguredEvent,
}

/// Conduit for the bidirectional stream of messages that compose a thread
/// (formerly called a conversation) in Codex.
impl CodexThread {
    pub(crate) fn new(
        codex: Codex,
        rollout_path: PathBuf,
        session_configured: SessionConfiguredEvent,
    ) -> Self {
        Self {
            codex,
            rollout_path,
            session_configured,
        }
    }

    pub async fn submit(&self, op: Op) -> CodexResult<String> {
        self.codex.submit(op).await
    }

    /// Use sparingly: this is intended to be removed soon.
    pub async fn submit_with_id(&self, sub: Submission) -> CodexResult<()> {
        self.codex.submit_with_id(sub).await
    }

    pub async fn next_event(&self) -> CodexResult<Event> {
        self.codex.next_event().await
    }

    pub async fn agent_status(&self) -> AgentStatus {
        self.codex.agent_status().await
    }

    pub fn rollout_path(&self) -> PathBuf {
        self.rollout_path.clone()
    }

    /// Snapshot of the initial session configuration for this thread.
    ///
    /// This reflects the values at thread creation time (e.g. model, policies,
    /// cwd). Some fields may become stale if the session changes after startup,
    /// so treat this as historical config rather than live state.
    pub fn session_configured(&self) -> SessionConfiguredEvent {
        self.session_configured.clone()
    }
}
