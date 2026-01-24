//! Model capability types.

use serde::{Deserialize, Serialize};

/// Model capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Basic text generation.
    TextGeneration,
    /// Streaming response support.
    Streaming,
    /// Vision/image input support.
    Vision,
    /// Audio input support.
    Audio,
    /// Tool/function calling support.
    ToolCalling,
    /// Embedding generation.
    Embedding,
    /// Extended thinking/reasoning support.
    ExtendedThinking,
    /// Structured output (JSON mode).
    StructuredOutput,
    /// Reasoning summaries support.
    ReasoningSummaries,
    /// Parallel tool calls support.
    ParallelToolCalls,
}
