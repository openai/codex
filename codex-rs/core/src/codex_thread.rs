use crate::agent::AgentStatus;
use crate::codex::Codex;
use crate::error::Result as CodexResult;
use crate::protocol::Event;
use crate::protocol::Op;
use crate::protocol::Submission;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use serde_json::Value;
use std::path::PathBuf;
use tokio::sync::watch;

#[derive(Clone, Debug)]
pub struct ThreadConfigSnapshot {
    pub model: String,
    pub model_provider_id: String,
    pub approval_policy: AskForApproval,
    pub sandbox_policy: SandboxPolicy,
    pub cwd: PathBuf,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub reasoning_summary: ReasoningSummary,
    pub personality: Option<Personality>,
    pub session_source: SessionSource,
}

pub struct CodexThread {
    codex: Codex,
    rollout_path: Option<PathBuf>,
}

/// Conduit for the bidirectional stream of messages that compose a thread
/// (formerly called a conversation) in Codex.
impl CodexThread {
    pub(crate) fn new(codex: Codex, rollout_path: Option<PathBuf>) -> Self {
        Self {
            codex,
            rollout_path,
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

    pub(crate) fn subscribe_status(&self) -> watch::Receiver<AgentStatus> {
        self.codex.agent_status.clone()
    }

    pub fn rollout_path(&self) -> Option<PathBuf> {
        self.rollout_path.clone()
    }

    pub async fn config_snapshot(&self) -> ThreadConfigSnapshot {
        self.codex.thread_config_snapshot().await
    }

    /// Build a user turn using the thread's current default settings.
    pub async fn user_turn_with_defaults(
        &self,
        items: Vec<UserInput>,
        final_output_json_schema: Option<Value>,
    ) -> Op {
        let snapshot = self.config_snapshot().await;
        Op::UserTurn {
            use_thread_defaults: true,
            items,
            cwd: snapshot.cwd,
            approval_policy: snapshot.approval_policy,
            sandbox_policy: snapshot.sandbox_policy,
            model: snapshot.model,
            effort: snapshot.reasoning_effort,
            summary: snapshot.reasoning_summary,
            final_output_json_schema,
            collaboration_mode: None,
            personality: snapshot.personality,
        }
    }

    /// Submit a user turn using the thread's current default settings.
    pub async fn submit_user_turn_with_defaults(
        &self,
        items: Vec<UserInput>,
        final_output_json_schema: Option<Value>,
    ) -> CodexResult<String> {
        let op = self
            .user_turn_with_defaults(items, final_output_json_schema)
            .await;
        self.submit(op).await
    }
}
