//! References to heavyweight trace payloads stored outside the reduced graph.

use serde::Deserialize;
use serde::Serialize;

/// Stable identifier for one raw payload inside a rollout bundle.
pub(crate) type RawPayloadId = String;

/// Reference to a raw request/response/log payload.
///
/// `RolloutTrace` stores these references so normal timeline and conversation
/// rendering does not require the browser or reducer output to inline every
/// upstream request, tool response, or terminal log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct RawPayloadRef {
    pub(crate) raw_payload_id: RawPayloadId,
    /// Payload role. This lets details UI choose syntax highlighting and labels
    /// without opening the payload file first.
    pub(crate) kind: RawPayloadKind,
    /// Path relative to the trace bundle root.
    ///
    /// The writer always materializes payloads as bundle-local files. Keeping
    /// this as a plain path avoids exposing storage modes we do not produce.
    pub(crate) path: String,
}

/// Coarse role of a raw payload.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub(crate) enum RawPayloadKind {
    InferenceRequest,
    /// Full upstream inference response or non-delta response stream summary.
    InferenceResponse,
    CompactionRequest,
    /// Trace-only checkpoint captured when processed replacement history is installed.
    CompactionCheckpoint,
    CompactionResponse,
    ToolInvocation,
    ToolResult,
    /// Raw runtime/protocol observation for an executing tool.
    ToolRuntimeEvent,
    /// Raw terminal runtime event or stream shard.
    TerminalRuntimeEvent,
    ProtocolEvent,
    /// One-shot metadata captured when a Codex session/thread starts.
    SessionMetadata,
    /// Runtime notification payload carried when a child agent reports back to its parent.
    AgentResult,
}
