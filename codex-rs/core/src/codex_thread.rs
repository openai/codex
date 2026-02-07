use crate::agent::AgentStatus;
use crate::codex::Codex;
use crate::codex::SteerInputError;
use crate::error::Result as CodexResult;
use crate::protocol::Event;
use crate::protocol::Op;
use crate::protocol::Submission;
use codex_protocol::config_types::Personality;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::watch;
use tracing::warn;

use crate::state_db::StateDbHandle;

#[derive(Clone, Debug)]
pub struct ThreadConfigSnapshot {
    pub model: String,
    pub model_provider_id: String,
    pub approval_policy: AskForApproval,
    pub sandbox_policy: SandboxPolicy,
    pub cwd: PathBuf,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub personality: Option<Personality>,
    pub session_source: SessionSource,
}

pub struct CodexThread {
    codex: Codex,
    rollout_path_tracker: Option<Arc<Mutex<Option<PathBuf>>>>,
}

/// Conduit for the bidirectional stream of messages that compose a thread
/// (formerly called a conversation) in Codex.
impl CodexThread {
    pub(crate) fn new(
        codex: Codex,
        rollout_path_tracker: Option<Arc<Mutex<Option<PathBuf>>>>,
    ) -> Self {
        Self {
            codex,
            rollout_path_tracker,
        }
    }

    pub async fn submit(&self, op: Op) -> CodexResult<String> {
        self.codex.submit(op).await
    }

    pub async fn steer_input(
        &self,
        input: Vec<UserInput>,
        expected_turn_id: Option<&str>,
    ) -> Result<String, SteerInputError> {
        self.codex.steer_input(input, expected_turn_id).await
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

    pub(crate) fn subscribe_status(&self) -> watch::Receiver<AgentStatus> {
        self.codex.agent_status.clone()
    }

    pub fn rollout_path(&self) -> Option<PathBuf> {
        let Some(path_tracker) = &self.rollout_path_tracker else {
            return None;
        };
        match path_tracker.lock() {
            Ok(path) => path.clone(),
            Err(err) => {
                warn!("rollout path lock poisoned: {err}");
                None
            }
        }
    }

    /// Ensure this thread has a persisted rollout and return its path.
    /// Returns `None` for ephemeral sessions.
    pub(crate) async fn ensure_rollout_persisted(&self) -> CodexResult<Option<PathBuf>> {
        self.codex.ensure_rollout_persisted().await
    }

    pub fn state_db(&self) -> Option<StateDbHandle> {
        self.codex.state_db()
    }

    pub async fn config_snapshot(&self) -> ThreadConfigSnapshot {
        self.codex.thread_config_snapshot().await
    }
}
