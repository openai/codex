use codex_protocol::ConversationId;
use codex_protocol::protocol::EventMsg;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Status store for globally-tracked agents.
#[derive(Clone, Default)]
pub(crate) struct AgentBus {
    /// In-memory map of conversation id to the latest derived status.
    statuses: Arc<RwLock<HashMap<ConversationId, AgentStatus>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AgentStatus {
    PendingInit,
    Running,
    Completed(Option<String>),
    Errored(String),
    Shutdown,
    #[allow(dead_code)] // Used by upcoming multi-agent tooling.
    NotFound,
}

impl AgentBus {
    /// Fetch the last known status for `agent_id`, returning `NotFound` if unseen.
    #[allow(dead_code)] // Used by upcoming multi-agent tooling.
    pub(crate) async fn status(&self, agent_id: ConversationId) -> AgentStatus {
        let statuses = self.statuses.read().await;
        statuses
            .get(&agent_id)
            .cloned()
            .unwrap_or(AgentStatus::NotFound)
    }

    /// Derive and record agent status from a single emitted event.
    pub(crate) async fn on_event(&self, conversation_id: ConversationId, msg: &EventMsg) {
        let next_status = match msg {
            EventMsg::TaskStarted(_) => Some(AgentStatus::Running),
            EventMsg::TaskComplete(ev) => {
                Some(AgentStatus::Completed(ev.last_agent_message.clone()))
            }
            EventMsg::TurnAborted(ev) => Some(AgentStatus::Errored(format!("{:?}", ev.reason))),
            EventMsg::Error(ev) => Some(AgentStatus::Errored(ev.message.clone())),
            EventMsg::ShutdownComplete => Some(AgentStatus::Shutdown),
            _ => None,
        };
        if let Some(status) = next_status {
            self.record_status(&conversation_id, status).await;
        }
    }

    /// Force-set the tracked status for an agent conversation.
    pub(crate) async fn record_status(
        &self,
        conversation_id: &ConversationId,
        status: AgentStatus,
    ) {
        self.statuses.write().await.insert(*conversation_id, status);
    }
}
