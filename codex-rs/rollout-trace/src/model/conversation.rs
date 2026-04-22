use serde::Deserialize;
use serde::Serialize;

use crate::payload::RawPayloadId;

use super::AgentThreadId;
use super::CodeCellId;
use super::CodexTurnId;
use super::CompactionId;
use super::ConversationItemId;
use super::EdgeId;
use super::InferenceCallId;
use super::ModelVisibleCallId;
use super::ToolCallId;
use super::session::ExecutionWindow;

/// One logical transcript item or transcript boundary.
///
/// The reducer builds conversation items primarily from inference request and
/// response payloads. Runtime objects can be listed in `produced_by`, but they
/// must not rewrite what the item body says the model saw. Structural items,
/// such as compaction markers, live in the same ordered list so conversation
/// views can show where the live history changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationItem {
    pub item_id: ConversationItemId,
    pub thread_id: AgentThreadId,
    /// Runtime activation that first introduced this item locally, when known.
    pub codex_turn_id: Option<CodexTurnId>,
    pub first_seen_at_unix_ms: i64,
    pub role: ConversationRole,
    /// Codex channel for assistant/tool content, when the item is channel-specific.
    pub channel: Option<ConversationChannel>,
    pub kind: ConversationItemKind,
    pub body: ConversationBody,
    /// Protocol/model `call_id` for function/custom tool call and output items.
    pub call_id: Option<ModelVisibleCallId>,
    /// Runtime or control-plane objects that caused this conversation item to exist.
    pub produced_by: Vec<ProducerRef>,
}

/// Model-visible role assigned to a conversation item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationRole {
    System,
    Developer,
    User,
    Assistant,
    Tool,
}

/// Codex channel for model-visible content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationChannel {
    Analysis,
    Commentary,
    Final,
    /// Remote compaction summaries are reintroduced as assistant summary-channel content.
    Summary,
}

/// Responses item category after normalization into the reduced transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationItemKind {
    Message,
    Reasoning,
    FunctionCall,
    FunctionCallOutput,
    CustomToolCall,
    CustomToolCallOutput,
    /// Structural marker inserted where live history was replaced by compaction.
    CompactionMarker,
}

/// Ordered content parts for a reduced conversation item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationBody {
    /// Renderable model-visible parts. Raw payload refs are used when the bytes
    /// are too large or too structured for the normal conversation path.
    pub parts: Vec<ConversationPart>,
}

/// One model-visible part inside a conversation item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ConversationPart {
    Text {
        text: String,
    },
    /// A model-provided summary of content whose full form may also be present.
    ///
    /// Reasoning summaries are not interchangeable with raw reasoning text:
    /// both can be present in one payload, and replay/debug tooling needs to
    /// preserve which representation the model actually returned.
    Summary {
        text: String,
    },
    /// Opaque model-visible content that is intentionally not decoded here.
    ///
    /// Reasoning can be carried as `encrypted_content` with no readable text.
    /// Keeping that blob inline makes it part of item identity, unlike a raw
    /// payload reference whose ID changes every time the same item is replayed
    /// in a later inference request.
    Encoded {
        label: String,
        value: String,
    },
    /// Small JSON-ish body represented by a summary plus a raw ref.
    Json {
        summary: String,
        raw_payload_id: RawPayloadId,
    },
    Code {
        language: String,
        source: String,
    },
    /// Large or uncommon payload that should be lazy-loaded from details UI.
    PayloadRef {
        label: String,
        raw_payload_id: RawPayloadId,
    },
}

/// Explanation for where a conversation item came from.
///
/// This is deliberately plural at the call site: a function output can be both
/// model-visible conversation and the product of a runtime tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ProducerRef {
    UserInput,
    Inference { inference_call_id: InferenceCallId },
    Tool { tool_call_id: ToolCallId },
    CodeCell { code_cell_id: CodeCellId },
    InteractionEdge { edge_id: EdgeId },
    Compaction { compaction_id: CompactionId },
    Harness,
}

/// One outbound inference request and its response metadata.
///
/// Full upstream request/response bodies live behind raw payload refs. The
/// request/response item ID lists are the reduced, model-visible snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceCall {
    pub inference_call_id: InferenceCallId,
    pub thread_id: AgentThreadId,
    pub codex_turn_id: CodexTurnId,
    pub execution: ExecutionWindow,
    pub model: String,
    pub provider_name: String,
    /// Upstream request ID returned by HTTP/proxy/engine infrastructure.
    pub upstream_request_id: Option<String>,
    /// Complete ordered input snapshot sent with this request.
    pub request_item_ids: Vec<ConversationItemId>,
    /// Ordered output items produced by this response.
    pub response_item_ids: Vec<ConversationItemId>,
    /// Runtime tool calls whose model-visible call item came from this response.
    pub tool_call_ids_started_by_response: Vec<ToolCallId>,
    pub usage: Option<TokenUsage>,
    pub raw_request_payload_id: RawPayloadId,
    /// Full upstream response payload. `None` while running or after pre-stream failures.
    pub raw_response_payload_id: Option<RawPayloadId>,
}

/// Token usage summary for one inference call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
}
