use crate::codex::Codex;
use crate::codex::Session;
use crate::error::Result as CodexResult;
use crate::protocol::Event;
use crate::protocol::Op;
use crate::protocol::Submission;
use std::path::PathBuf;
use std::sync::Arc;

pub struct CodexConversation {
    codex: Codex,
    rollout_path: PathBuf,
    session: Arc<Session>,
}

/// Conduit for the bidirectional stream of messages that compose a conversation
/// in Codex.
impl CodexConversation {
    pub(crate) fn new(codex: Codex, rollout_path: PathBuf, session: Arc<Session>) -> Self {
        Self {
            codex,
            rollout_path,
            session,
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

    pub fn rollout_path(&self) -> PathBuf {
        self.rollout_path.clone()
    }

    pub async fn flush_rollout(&self) -> CodexResult<()> {
        Ok(self.session.flush_rollout().await?)
    }

    pub async fn set_session_name(&self, name: Option<String>) -> CodexResult<()> {
        Ok(self.session.set_session_name(name).await?)
    }

    pub async fn model(&self) -> String {
        self.session.model().await
    }

    pub async fn save_session(
        &self,
        codex_home: &std::path::Path,
        name: &str,
    ) -> CodexResult<crate::SavedSessionEntry> {
        self.session.save_session(codex_home, name).await
    }
}
