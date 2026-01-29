//! Agent loop driver for multi-turn conversations with LLM providers.

mod compaction;
mod driver;
mod fallback;
mod result;

pub use compaction::{
    CONTEXT_RESTORATION_BUDGET, CONTEXT_RESTORATION_MAX_FILES, CompactionConfig, CompactionResult,
    CompactionTier, ContextRestoration, FileRestoration, MIN_MICRO_COMPACT_SAVINGS,
    RECENT_TOOL_RESULTS_TO_KEEP, SessionMemoryConfig, SessionMemorySummary,
    build_context_restoration, format_restoration_message, micro_compact_candidates,
    should_compact, try_session_memory_compact,
};
pub use driver::{AgentLoop, AgentLoopBuilder};
pub use fallback::{FallbackAttempt, FallbackConfig, FallbackState};
pub use result::{LoopResult, StopReason};

// Re-export LoopConfig from cocode-protocol
pub use cocode_protocol::LoopConfig;
