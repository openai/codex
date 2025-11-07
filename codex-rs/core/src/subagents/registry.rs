use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_protocol::AgentId;
use codex_protocol::ConversationId;
use codex_protocol::protocol::SubagentLifecycleOrigin;
use codex_protocol::protocol::SubagentLifecycleStatus;
use serde::Serialize;
use tokio::sync::RwLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentOrigin {
    Spawn,
    Fork,
    SendMessage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    Queued,
    Running,
    Ready,
    Idle,
    Failed,
    Canceled,
}

#[derive(Clone, Debug, Serialize)]
pub struct SubagentMetadata {
    pub agent_id: AgentId,
    pub parent_agent_id: Option<AgentId>,
    pub session_id: ConversationId,
    pub parent_session_id: Option<ConversationId>,
    pub origin: SubagentOrigin,
    pub initial_message_count: usize,
    pub status: SubagentStatus,
    #[serde(skip_serializing)]
    pub created_at: SystemTime,
    #[serde(skip_serializing)]
    pub created_at_ms: i64,
    #[serde(skip_serializing)]
    pub session_key: String,
    pub label: Option<String>,
    pub summary: Option<String>,
    pub reasoning_header: Option<String>,
    pub pending_messages: usize,
    pub pending_interrupts: usize,
}

#[derive(Clone, Default)]
pub struct SubagentRegistry {
    inner: Arc<RwLock<HashMap<ConversationId, SubagentMetadata>>>,
}

impl SubagentMetadata {
    #[allow(clippy::too_many_arguments)]
    fn new(
        session_id: ConversationId,
        parent_session_id: Option<ConversationId>,
        agent_id: AgentId,
        parent_agent_id: Option<AgentId>,
        origin: SubagentOrigin,
        initial_message_count: usize,
        label: Option<String>,
        summary: Option<String>,
    ) -> Self {
        let created_at = SystemTime::now();
        Self {
            agent_id,
            parent_agent_id,
            session_id,
            parent_session_id,
            origin,
            initial_message_count,
            status: SubagentStatus::Queued,
            created_at,
            created_at_ms: unix_time_millis(created_at),
            session_key: session_id.to_string(),
            label,
            summary,
            reasoning_header: None,
            pending_messages: 0,
            pending_interrupts: 0,
        }
    }
}

impl SubagentMetadata {
    pub fn from_summary(summary: &codex_protocol::protocol::SubagentSummary) -> Self {
        let created_at = if summary.started_at_ms >= 0 {
            std::time::UNIX_EPOCH + std::time::Duration::from_millis(summary.started_at_ms as u64)
        } else {
            std::time::UNIX_EPOCH
                - std::time::Duration::from_millis(summary.started_at_ms.unsigned_abs())
        };
        SubagentMetadata {
            agent_id: summary.agent_id,
            parent_agent_id: summary.parent_agent_id,
            session_id: summary.session_id,
            parent_session_id: summary.parent_session_id,
            origin: SubagentOrigin::from(summary.origin),
            initial_message_count: 0,
            status: SubagentStatus::from(summary.status),
            created_at,
            created_at_ms: summary.started_at_ms,
            session_key: summary.session_id.to_string(),
            label: summary.label.clone(),
            summary: summary.summary.clone(),
            reasoning_header: summary.reasoning_header.clone(),
            pending_messages: summary.pending_messages,
            pending_interrupts: summary.pending_interrupts,
        }
    }
}

impl From<SubagentLifecycleStatus> for SubagentStatus {
    fn from(status: SubagentLifecycleStatus) -> Self {
        match status {
            SubagentLifecycleStatus::Queued => SubagentStatus::Queued,
            SubagentLifecycleStatus::Running => SubagentStatus::Running,
            SubagentLifecycleStatus::Ready => SubagentStatus::Ready,
            SubagentLifecycleStatus::Idle => SubagentStatus::Idle,
            SubagentLifecycleStatus::Failed => SubagentStatus::Failed,
            SubagentLifecycleStatus::Canceled => SubagentStatus::Canceled,
        }
    }
}

impl From<SubagentLifecycleOrigin> for SubagentOrigin {
    fn from(origin: SubagentLifecycleOrigin) -> Self {
        match origin {
            SubagentLifecycleOrigin::Spawn => SubagentOrigin::Spawn,
            SubagentLifecycleOrigin::Fork => SubagentOrigin::Fork,
            SubagentLifecycleOrigin::SendMessage => SubagentOrigin::SendMessage,
        }
    }
}

impl SubagentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn register_spawn(
        &self,
        session_id: ConversationId,
        parent_session_id: Option<ConversationId>,
        parent_agent_id: Option<AgentId>,
        agent_id: AgentId,
        initial_message_count: usize,
        label: Option<String>,
        summary: Option<String>,
    ) -> SubagentMetadata {
        let metadata = SubagentMetadata::new(
            session_id,
            parent_session_id,
            agent_id,
            parent_agent_id,
            SubagentOrigin::Spawn,
            initial_message_count,
            label,
            summary,
        );
        self.insert_if_absent(metadata).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn register_fork(
        &self,
        session_id: ConversationId,
        parent_session_id: ConversationId,
        parent_agent_id: Option<AgentId>,
        agent_id: AgentId,
        initial_message_count: usize,
        label: Option<String>,
        summary: Option<String>,
    ) -> SubagentMetadata {
        let metadata = SubagentMetadata::new(
            session_id,
            Some(parent_session_id),
            agent_id,
            parent_agent_id,
            SubagentOrigin::Fork,
            initial_message_count,
            label,
            summary,
        );
        self.insert_if_absent(metadata).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn register_resume(
        &self,
        session_id: ConversationId,
        parent_session_id: ConversationId,
        parent_agent_id: Option<AgentId>,
        agent_id: AgentId,
        initial_message_count: usize,
        label: Option<String>,
        summary: Option<String>,
    ) -> SubagentMetadata {
        let metadata = SubagentMetadata::new(
            session_id,
            Some(parent_session_id),
            agent_id,
            parent_agent_id,
            SubagentOrigin::SendMessage,
            initial_message_count,
            label,
            summary,
        );
        self.insert_if_absent(metadata).await
    }

    pub async fn update_status(
        &self,
        session_id: &ConversationId,
        status: SubagentStatus,
    ) -> Option<SubagentMetadata> {
        let mut guard = self.inner.write().await;
        if let Some(entry) = guard.get_mut(session_id) {
            entry.status = status;
            return Some(entry.clone());
        }
        None
    }

    pub async fn update_reasoning_header(
        &self,
        session_id: &ConversationId,
        header: String,
    ) -> Option<SubagentMetadata> {
        let mut guard = self.inner.write().await;
        if let Some(entry) = guard.get_mut(session_id) {
            entry.reasoning_header = Some(header);
            return Some(entry.clone());
        }
        None
    }

    pub async fn get(&self, session_id: &ConversationId) -> Option<SubagentMetadata> {
        let guard = self.inner.read().await;
        guard.get(session_id).cloned()
    }

    pub async fn update_label_and_summary(
        &self,
        session_id: &ConversationId,
        label: Option<String>,
        summary: Option<String>,
    ) -> Option<SubagentMetadata> {
        let mut guard = self.inner.write().await;
        if let Some(entry) = guard.get_mut(session_id) {
            entry.label = label;
            entry.summary = summary;
            return Some(entry.clone());
        }
        None
    }

    pub async fn update_inbox_counts(
        &self,
        session_id: &ConversationId,
        pending_messages: usize,
        pending_interrupts: usize,
    ) -> Option<SubagentMetadata> {
        let mut guard = self.inner.write().await;
        if let Some(entry) = guard.get_mut(session_id) {
            entry.pending_messages = pending_messages;
            entry.pending_interrupts = pending_interrupts;
            return Some(entry.clone());
        }
        None
    }

    pub async fn list(&self) -> Vec<SubagentMetadata> {
        let guard = self.inner.read().await;
        let mut entries: Vec<SubagentMetadata> = guard.values().cloned().collect();
        entries.sort_by(|a, b| {
            a.created_at_ms
                .cmp(&b.created_at_ms)
                .then_with(|| a.session_key.cmp(&b.session_key))
        });
        entries
    }

    pub async fn remove(&self, session_id: &ConversationId) -> Option<SubagentMetadata> {
        let mut guard = self.inner.write().await;
        guard.remove(session_id)
    }

    /// Insert a fully-formed metadata entry (used when adopting children into a new
    /// parent session during a fork). This does not adjust timestamps or keys.
    pub async fn register_imported(&self, metadata: SubagentMetadata) -> SubagentMetadata {
        let mut guard = self.inner.write().await;
        guard.insert(metadata.session_id, metadata.clone());
        metadata
    }

    pub async fn prune<F>(&self, mut predicate: F) -> Vec<ConversationId>
    where
        F: FnMut(&SubagentMetadata) -> bool,
    {
        let mut guard = self.inner.write().await;
        let ids: Vec<ConversationId> = guard
            .iter()
            .filter_map(|(id, meta)| if predicate(meta) { Some(*id) } else { None })
            .collect();
        for id in &ids {
            guard.remove(id);
        }
        ids
    }
    async fn insert_if_absent(&self, metadata: SubagentMetadata) -> SubagentMetadata {
        let mut guard = self.inner.write().await;
        if let Some(existing) = guard.get(&metadata.session_id) {
            return existing.clone();
        }
        let session_id = metadata.session_id;
        guard.insert(session_id, metadata.clone());
        metadata
    }
}

fn unix_time_millis(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as i64,
        Err(err) => -(err.duration().as_millis() as i64),
    }
}
