use crate::codex::Codex;
use crate::error::Result as CodexResult;
use crate::protocol::Event;
use crate::protocol::Op;
use crate::protocol::Submission;
use codex_utils_readiness::ReadinessFlag;
use std::sync::Arc;
use tokio::sync::oneshot;

pub struct CodexConversation {
    codex: Codex,
}

pub type TurnReadinessTx = oneshot::Sender<Arc<ReadinessFlag>>;

/// Conduit for the bidirectional stream of messages that compose a conversation
/// in Codex.
impl CodexConversation {
    pub(crate) fn new(codex: Codex) -> Self {
        Self { codex }
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

    pub async fn submit_with_readiness(
        &self,
        op: Op,
        readiness: Option<TurnReadinessTx>,
    ) -> CodexResult<String> {
        self.codex.submit_with_readiness(op, readiness).await
    }
}
