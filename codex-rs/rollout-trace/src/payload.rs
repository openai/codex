//! References to heavyweight trace payloads stored outside the reduced graph.

use serde::Deserialize;
use serde::Serialize;

/// Stable identifier for one raw payload inside a rollout bundle.
pub type RawPayloadId = String;

/// Reference to a raw request/response/log payload.
///
/// `RolloutTrace` stores these references so normal timeline and conversation
/// rendering does not require the browser or reducer output to inline every
/// upstream request, tool response, or terminal log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawPayloadRef {
    pub raw_payload_id: RawPayloadId,
    /// Payload role. This lets details UI choose syntax highlighting and labels
    /// without decoding the payload.
    pub kind: RawPayloadKind,
    /// Shard containing the retained bytes. Absent when the bundle budget omitted the body.
    pub shard: Option<u64>,
    #[serde(default, skip_serializing_if = "PayloadCapture::is_complete")]
    pub capture: PayloadCapture,
    /// Interleave these values with the stored JSON fragments to recover the original bytes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<RawPayloadRef>,
}

/// Describes a configured size limit. An I/O or encoding error still fails the writer.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PayloadCapture {
    #[default]
    Complete,
    Truncated {
        original_bytes: usize,
        retained_bytes: usize,
    },
    Omitted {
        original_bytes: usize,
    },
}

impl PayloadCapture {
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }
}

impl RawPayloadRef {
    pub(crate) fn references(&self) -> impl Iterator<Item = &Self> {
        let mut pending = vec![self];
        std::iter::from_fn(move || {
            let next = pending.pop()?;
            pending.extend(&next.parts);
            Some(next)
        })
    }

    pub fn is_complete(&self) -> bool {
        self.capture.is_complete() && self.parts.iter().all(RawPayloadRef::is_complete)
    }
}

/// Coarse role of a raw payload.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum RawPayloadKind {
    InferenceRequest,
    /// Core input and tool/output settings before provider-specific formatting.
    PreparedPrompt,
    /// One exact item in a prepared prompt or oversized provider input/output array.
    ModelItem,
    /// An inline image or oversized text part. Losing its bytes preserves its message and position.
    ContentValue,
    /// Provider-formatted context for replay of a continuation from an untraced warmup.
    WarmupContext,
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
