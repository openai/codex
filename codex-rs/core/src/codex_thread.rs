use crate::agent::AgentStatus;
use crate::codex::Codex;
use crate::codex::SteerInputError;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::features::Feature;
use crate::file_watcher::WatchRegistration;
use crate::protocol::Event;
use crate::protocol::Op;
use crate::protocol::Submission;
use codex_protocol::config_types::Personality;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::user_input::UserInput;
use std::path::PathBuf;
use tokio::sync::Mutex;
use tokio::sync::watch;

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
    pub(crate) codex: Codex,
    rollout_path: Option<PathBuf>,
    out_of_band_elicitation_count: Mutex<u64>,
    _watch_registration: WatchRegistration,
}

/// Conduit for the bidirectional stream of messages that compose a thread
/// (formerly called a conversation) in Codex.
impl CodexThread {
    pub(crate) fn new(
        codex: Codex,
        rollout_path: Option<PathBuf>,
        watch_registration: WatchRegistration,
    ) -> Self {
        Self {
            codex,
            rollout_path,
            out_of_band_elicitation_count: Mutex::new(0),
            _watch_registration: watch_registration,
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

    pub(crate) async fn total_token_usage(&self) -> Option<TokenUsage> {
        self.codex.session.total_token_usage().await
    }

    /// Records a user-role session-prefix message without creating a new user turn boundary.
    pub(crate) async fn inject_user_message_without_turn(&self, message: String) {
        let pending_item = ResponseInputItem::Message {
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: message }],
        };
        let pending_items = vec![pending_item];
        let Err(items_without_active_turn) = self
            .codex
            .session
            .inject_response_items(pending_items)
            .await
        else {
            return;
        };

        let turn_context = self.codex.session.new_default_turn().await;
        let items: Vec<ResponseItem> = items_without_active_turn
            .into_iter()
            .map(ResponseItem::from)
            .collect();
        self.codex
            .session
            .record_conversation_items(turn_context.as_ref(), &items)
            .await;
    }

    pub fn rollout_path(&self) -> Option<PathBuf> {
        self.rollout_path.clone()
    }

    pub fn state_db(&self) -> Option<StateDbHandle> {
        self.codex.state_db()
    }

    pub async fn config_snapshot(&self) -> ThreadConfigSnapshot {
        self.codex.thread_config_snapshot().await
    }

    pub fn enabled(&self, feature: Feature) -> bool {
        self.codex.enabled(feature)
    }

    pub async fn increment_out_of_band_elicitation_count(&self) -> CodexResult<u64> {
        let mut guard = self.out_of_band_elicitation_count.lock().await;
        let was_zero = *guard == 0;
        *guard = guard.checked_add(1).ok_or_else(|| {
            CodexErr::Fatal("out-of-band elicitation count overflowed".to_string())
        })?;

        if was_zero
            && let Err(err) = self
                .codex
                .session
                .notify_out_of_band_elicitation_pause_state(true)
                .await
        {
            *guard -= 1;
            return Err(CodexErr::Fatal(format!(
                "failed to pause out-of-band elicitation state: {err:#}"
            )));
        }

        Ok(*guard)
    }

    pub async fn decrement_out_of_band_elicitation_count(&self) -> CodexResult<u64> {
        let mut guard = self.out_of_band_elicitation_count.lock().await;
        if *guard == 0 {
            return Err(CodexErr::InvalidRequest(
                "out-of-band elicitation count is already zero".to_string(),
            ));
        }

        *guard -= 1;
        let now_zero = *guard == 0;
        if now_zero
            && let Err(err) = self
                .codex
                .session
                .notify_out_of_band_elicitation_pause_state(false)
                .await
        {
            *guard += 1;
            return Err(CodexErr::Fatal(format!(
                "failed to resume out-of-band elicitation state: {err:#}"
            )));
        }

        Ok(*guard)
    }
}
