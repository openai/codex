//! Reduced rollout trace model.
//!
//! These types describe the deterministic replay output. They intentionally
//! separate model-visible conversation from runtime/debug objects.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

use crate::payload::RawPayloadId;
use crate::payload::RawPayloadRef;
mod conversation;
mod runtime;
mod session;

pub(crate) use conversation::*;
pub(crate) use runtime::*;
pub(crate) use session::*;

/// Codex conversation/session UUID.
pub(crate) type AgentThreadId = String;
/// Stable multi-agent routing path such as `/root` or `/root/search_docs`.
pub(crate) type AgentPath = String;
/// Runtime submission/activation UUID. This is not a chat turn.
pub(crate) type CodexTurnId = String;
/// Reduced transcript item ID assigned by the trace reducer.
pub(crate) type ConversationItemId = String;
/// Local ID for one outbound upstream inference request.
pub(crate) type InferenceCallId = String;
/// Reducer-owned ID for one runtime tool-call object.
pub(crate) type ToolCallId = String;
/// Responses `call_id` / custom-tool call ID visible in inference payloads.
pub(crate) type ModelVisibleCallId = String;
/// Tool invocation ID assigned inside the code-mode JavaScript runtime.
pub(crate) type CodeModeRuntimeToolId = String;
/// Reducer-owned ID for one model-authored `exec` JavaScript cell.
pub(crate) type CodeCellId = String;
/// Process/session ID returned by Codex's terminal runtime.
pub(crate) type TerminalId = String;
/// Reducer-owned ID for one command/write/poll operation against a terminal.
pub(crate) type TerminalOperationId = String;
/// Reducer-owned ID for one installed conversation-history checkpoint.
pub(crate) type CompactionId = String;
/// Reducer-owned ID for one upstream request that computes a compaction.
pub(crate) type CompactionRequestId = String;
/// Reducer-owned ID for one information-flow edge.
pub(crate) type EdgeId = String;
/// Reducer-owned ID for request/log correlation metadata.
#[allow(dead_code)]
pub(crate) type CorrelationId = String;

/// Canonical reduced graph for one Codex rollout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct RolloutTrace {
    pub(crate) schema_version: u32,
    /// Unique identity for this trace capture.
    ///
    /// `rollout_id` names the Codex rollout/session being observed. `trace_id`
    /// names the diagnostic artifact produced for that rollout, which keeps
    /// storage/replay identity separate from the product-level session identity.
    pub(crate) trace_id: String,
    /// CLI-visible rollout/run identity. Higher-level experiment/sample IDs wrap this object.
    pub(crate) rollout_id: String,
    pub(crate) started_at_unix_ms: i64,
    /// Wall-clock timestamp for terminal rollout status. `None` means running or partial trace.
    pub(crate) ended_at_unix_ms: Option<i64>,
    pub(crate) status: RolloutStatus,
    pub(crate) root_thread_id: AgentThreadId,
    pub(crate) threads: BTreeMap<AgentThreadId, AgentThread>,
    pub(crate) codex_turns: BTreeMap<CodexTurnId, CodexTurn>,
    pub(crate) conversation_items: BTreeMap<ConversationItemId, ConversationItem>,
    pub(crate) inference_calls: BTreeMap<InferenceCallId, InferenceCall>,
    /// Model-authored `exec` JavaScript cells keyed by reducer-owned cell ID.
    pub(crate) code_cells: BTreeMap<CodeCellId, CodeCell>,
    pub(crate) tool_calls: BTreeMap<ToolCallId, ToolCall>,
    /// Terminal runtime sessions keyed by process/session ID returned by the runtime.
    pub(crate) terminal_sessions: BTreeMap<TerminalId, TerminalSession>,
    /// Commands/writes/polls against terminals keyed by reducer-owned operation ID.
    pub(crate) terminal_operations: BTreeMap<TerminalOperationId, TerminalOperation>,
    /// Installed compaction checkpoints keyed by checkpoint ID.
    pub(crate) compactions: BTreeMap<CompactionId, Compaction>,
    /// Upstream remote compaction calls keyed by local request ID.
    pub(crate) compaction_requests: BTreeMap<CompactionRequestId, CompactionRequest>,
    /// Information-flow edges between threads, cells, tools, and runtime resources.
    pub(crate) interaction_edges: BTreeMap<EdgeId, InteractionEdge>,
    /// Raw JSON payloads keyed by raw-payload ID. Most point at files outside this object.
    pub(crate) raw_payloads: BTreeMap<RawPayloadId, RawPayloadRef>,
}

impl RolloutTrace {
    /// Builds an empty reduced trace that a reducer can populate.
    pub(crate) fn new(
        schema_version: u32,
        trace_id: String,
        rollout_id: String,
        root_thread_id: AgentThreadId,
        started_at_unix_ms: i64,
    ) -> Self {
        Self {
            schema_version,
            trace_id,
            rollout_id,
            started_at_unix_ms,
            ended_at_unix_ms: None,
            status: RolloutStatus::Running,
            root_thread_id,
            threads: BTreeMap::new(),
            codex_turns: BTreeMap::new(),
            conversation_items: BTreeMap::new(),
            inference_calls: BTreeMap::new(),
            code_cells: BTreeMap::new(),
            tool_calls: BTreeMap::new(),
            terminal_sessions: BTreeMap::new(),
            terminal_operations: BTreeMap::new(),
            compactions: BTreeMap::new(),
            compaction_requests: BTreeMap::new(),
            interaction_edges: BTreeMap::new(),
            raw_payloads: BTreeMap::new(),
        }
    }
}
