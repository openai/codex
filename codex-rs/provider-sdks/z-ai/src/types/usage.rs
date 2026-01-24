//! Token usage types for Z.AI SDK.
//!
//! Types aligned with Python SDK `chat_completion.py`.

use serde::Deserialize;
use serde::Serialize;

/// Detailed breakdown of token usage for the input prompt.
///
/// From Python SDK `chat_completion.py:51`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptTokensDetails {
    /// Number of tokens reused from cache.
    #[serde(default)]
    pub cached_tokens: i32,
}

/// Detailed breakdown of token usage for the model completion.
///
/// From Python SDK `chat_completion.py:62`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompletionTokensDetails {
    /// Number of tokens used for reasoning steps.
    #[serde(default)]
    pub reasoning_tokens: i32,
}

/// Token usage information for completion.
///
/// From Python SDK `chat_completion.py:73`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompletionUsage {
    /// Number of tokens in the prompt.
    pub prompt_tokens: i32,
    /// Detailed breakdown of token usage for the input prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<PromptTokensDetails>,
    /// Number of tokens in the completion.
    pub completion_tokens: i32,
    /// Detailed breakdown of token usage for the model completion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens_details: Option<CompletionTokensDetails>,
    /// Total number of tokens used.
    pub total_tokens: i32,
}
