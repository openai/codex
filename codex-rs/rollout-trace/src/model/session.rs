use serde::Deserialize;
use serde::Serialize;

use crate::raw_event::RawEventSeq;

use super::AgentPath;
use super::AgentThreadId;
use super::CodexTurnId;
use super::ConversationItemId;
use super::EdgeId;

/// Coarse terminal status for the rollout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutStatus {
    /// Writer has not seen a terminal rollout event.
    Running,
    /// Rollout ended normally.
    Completed,
    /// Rollout ended because an operation failed.
    Failed,
    /// Rollout was cancelled or otherwise stopped before normal completion.
    Aborted,
}

/// One Codex thread/session participating in the rollout.
///
/// Threads are agents in the multi-agent sense, but the root interactive
/// session is represented by the same object. Runtime objects live in top-level
/// maps and point back to their owning thread; only transcript order is stored
/// here because compaction/reconciliation makes it semantic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentThread {
    pub thread_id: AgentThreadId,
    /// Stable routing identity. Viewer/search should prefer this over nickname.
    pub agent_path: AgentPath,
    /// Presentation hint. It can collide and must not be used as identity.
    pub nickname: Option<String>,
    pub origin: AgentOrigin,
    /// Session lifecycle for this thread.
    ///
    /// Child threads can end independently from the root rollout, for example
    /// after a parent calls `close_agent`. Keeping this on the thread prevents
    /// those shutdowns from being mistaken for whole-rollout completion.
    pub execution: ExecutionWindow,
    /// Configured model presentation hint. Individual inference calls carry the actual upstream model.
    pub default_model: Option<String>,
    /// Logical conversation items first observed for this thread, in transcript order.
    pub conversation_item_ids: Vec<ConversationItemId>,
}

/// Provenance for a traced Codex thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum AgentOrigin {
    Root,
    Spawned {
        parent_thread_id: AgentThreadId,
        /// Interaction edge that carried the spawn task.
        spawn_edge_id: EdgeId,
        /// Stable path segment/task name selected by the parent/tool call.
        task_name: String,
        /// Selected agent role/type, for example `worker` or `explorer`.
        agent_role: String,
    },
}

/// Runtime interval for a typed trace object.
///
/// Wall-clock timestamps are for display and latency. Sequence numbers are the
/// causal ordering primitive and should be used to pair observations or break
/// same-millisecond ties.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionWindow {
    pub started_at_unix_ms: i64,
    pub started_seq: RawEventSeq,
    pub ended_at_unix_ms: Option<i64>,
    pub ended_seq: Option<RawEventSeq>,
    pub status: ExecutionStatus,
}

/// Coarse lifecycle status for a runtime object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// Object is still live or the trace ended before its terminal event.
    Running,
    /// Object completed successfully.
    Completed,
    /// Object reached an error state.
    Failed,
    /// Object was cancelled by user/policy/runtime before completion.
    Cancelled,
    /// Object was aborted when its owner/runtime stopped.
    Aborted,
}

/// One activation of the Codex runtime for one thread.
///
/// A Codex turn groups protocol/runtime work for one thread activation.
/// It is not a user/assistant message pair; conversation belongs in
/// `ConversationItem`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodexTurn {
    pub codex_turn_id: CodexTurnId,
    pub thread_id: AgentThreadId,
    pub execution: ExecutionWindow,
    /// Conversation items that directly triggered this activation, when known.
    pub input_item_ids: Vec<ConversationItemId>,
}
