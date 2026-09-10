//! Structured transcript evidence shared by synchronous and asynchronous Guardian.
//!
//! Entry kinds preserve source attribution for consumer-specific retention and
//! rendering. User text and manual approvals stay complete for admission; other text is
//! capped during collection, with its original size retained for accounting.

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

/// Structured text evidence shared by sync Guardian and async scoring.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationTranscriptEntry {
    /// Semantic role used for consumer-specific retention and truncation.
    pub kind: ConversationTranscriptEntryKind,
    /// Complete authorization evidence, or other text bounded by per-entry limits.
    pub text: String,
    /// Size before truncation, retained for omission and truncation accounting.
    pub original_bytes: usize,
}
