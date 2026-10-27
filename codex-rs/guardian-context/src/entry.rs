//! Structured transcript evidence shared by synchronous and asynchronous Guardian.
//!
//! Entry kinds preserve source attribution for consumer-specific retention and
//! rendering. Sync entries remain complete until request preparation; async collection
//! retains its existing caps. Original sizes remain available for accounting.

/// Semantic role of one parent-conversation transcript entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConversationTranscriptEntryKind {
    /// Actual parent-thread user message.
    User,
    /// Explicit developer message preserving a user-approved action.
    Developer,
    /// Assistant commentary or an inter-agent message.
    Assistant,
    /// Final assistant answer eligible for stronger async retention.
    ProtectedAssistant,
    /// Named tool invocation, including shell and web-search calls.
    ToolCall(String),
    /// Tool result with a call-derived, explicit, or generic fallback label.
    ToolOutput(String),
    /// Result from a Node REPL-backed tool that may receive a larger sync cap.
    NodeReplToolOutput(String),
    /// Plaintext model reasoning that the async consumer explicitly enables.
    Reasoning,
}

impl ConversationTranscriptEntryKind {
    /// Returns the role label currently used in Guardian transcript prompts.
    pub fn role(&self) -> &str {
        match self {
            Self::User => "user",
            Self::Developer => "developer",
            Self::Assistant | Self::ProtectedAssistant => "assistant",
            Self::ToolCall(role) | Self::ToolOutput(role) | Self::NodeReplToolOutput(role) => {
                role.as_str()
            }
            Self::Reasoning => "reasoning",
        }
    }
}

/// Collected text, a host-attributed record, or an opaque backend-native message.
#[derive(Clone, PartialEq)]
pub enum TranscriptContent {
    Text(String),
    /// Original scheduler input, compressed only during synchronous preparation.
    ScheduledMessage(ScheduledTranscriptMessage),
    /// Host-attributed text prepared by the profile, rendered at delivery.
    Record(crate::TranscriptRecord),
    NativeMessage(Box<codex_protocol::models::ResponseItem>),
}

impl std::fmt::Debug for TranscriptContent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Text(_) => "Text(..)",
            Self::ScheduledMessage(_) => "ScheduledMessage(..)",
            Self::Record(_) => "Record(..)",
            Self::NativeMessage(_) => "NativeMessage(..)",
        })
    }
}

/// Structured evidence shared by sync Guardian and async scoring.
#[derive(Clone, Debug, PartialEq)]
pub struct ConversationTranscriptEntry {
    /// Semantic role used for consumer-specific retention and truncation.
    pub kind: ConversationTranscriptEntryKind,
    /// Complete authorization evidence, or other text bounded by per-entry limits.
    pub content: TranscriptContent,
    /// Size before truncation, retained for omission and truncation accounting.
    pub original_bytes: usize,
    /// Original source revision and display order, established from host metadata.
    pub retained_source: Option<RetainedTranscriptSource>,
}

/// Display ordering is separate from source identity and cannot establish delivery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedTranscriptSource {
    /// Size of the complete role- and order-labeled source, before request admission.
    pub rendered_bytes: usize,
    pub order: String,
    pub source: codex_history::RetainedSource,
}

/// Original scheduler text and the provenance needed to compare heartbeat instructions.
#[derive(Clone, PartialEq)]
pub struct ScheduledTranscriptMessage {
    pub(crate) text: String,
    pub(crate) heartbeat: Option<crate::preparation::HeartbeatInstruction>,
}
