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

/// Select whole JSON values whose bytes can be stored independently. Only model-facing
/// payloads use this layout. Other consumers can continue reading their single raw record.
pub(crate) fn capture_parts(
    kind: RawPayloadKind,
    body: &[u8],
    limit: usize,
) -> anyhow::Result<Vec<(std::ops::Range<usize>, RawPayloadKind)>> {
    use serde_json::value::RawValue;
    use std::collections::BTreeMap;

    if !matches!(
        kind,
        RawPayloadKind::PreparedPrompt
            | RawPayloadKind::ModelItem
            | RawPayloadKind::InferenceRequest
            | RawPayloadKind::InferenceResponse
            | RawPayloadKind::WarmupContext
            | RawPayloadKind::CompactionCheckpoint
    ) {
        return Ok(Vec::new());
    }
    let text = std::str::from_utf8(body)?;
    if !matches!(text.trim_start().as_bytes().first(), Some(b'{' | b'['))
        || kind != RawPayloadKind::PreparedPrompt
            && text.len() <= limit
            && !text.contains("data:image/")
    {
        return Ok(Vec::new());
    }
    let mut pending = vec![(String::new(), serde_json::from_str::<&RawValue>(text)?)];
    let mut ranges = Vec::new();
    while let Some((field, raw)) = pending.pop() {
        let value = raw.get();
        let mut select = |value: &str, kind| {
            let start = value.as_ptr() as usize - text.as_ptr() as usize;
            ranges.push((start..start + value.len(), kind));
        };
        match value.as_bytes()[0] {
            b'[' if (kind == RawPayloadKind::PreparedPrompt && field == "input")
                || (value.len() > limit
                    && matches!(
                        field.as_str(),
                        "input" | "output_items" | "input_history" | "replacement_history"
                    )) =>
            {
                for item in serde_json::from_str::<Vec<&RawValue>>(value)? {
                    select(item.get(), RawPayloadKind::ModelItem);
                }
            }
            b'"' if (field == "image_url" && value.starts_with("\"data:image/"))
                || (field == "text" && value.len() > limit) =>
            {
                select(value, RawPayloadKind::ContentValue);
            }
            b'{' | b'['
                if value.len() > limit
                    || value.contains("data:image/")
                    || (kind == RawPayloadKind::PreparedPrompt && field.is_empty()) =>
            {
                if value.starts_with('{') {
                    pending.extend(serde_json::from_str::<BTreeMap<String, &RawValue>>(value)?);
                } else {
                    pending.extend(
                        serde_json::from_str::<Vec<&RawValue>>(value)?
                            .into_iter()
                            .map(|item| ("[]".into(), item)),
                    );
                }
            }
            _ => {}
        }
    }
    ranges.sort_unstable_by_key(|(range, _)| range.start);
    Ok(ranges)
}
