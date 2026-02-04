//! Agent loop driver for multi-turn conversations with LLM providers.

mod compaction;
mod driver;
mod fallback;
mod result;
mod session_memory_agent;

pub use compaction::CompactConfig;
pub use compaction::CompactionConfig;
pub use compaction::CompactionResult;
pub use compaction::CompactionTier;
pub use compaction::ContextRestoration;
pub use compaction::FileRestoration;
pub use compaction::InvokedSkillRestoration;
pub use compaction::SessionMemoryConfig;
pub use compaction::SessionMemorySummary;
pub use compaction::build_context_restoration;
pub use compaction::format_restoration_message;
pub use compaction::micro_compact_candidates;
pub use compaction::should_compact;
pub use compaction::try_session_memory_compact;

// Phase 2: Micro-compact execution and threshold status
pub use compaction::CLEARED_CONTENT_MARKER;
pub use compaction::COMPACTABLE_TOOLS;
pub use compaction::CONTENT_PREVIEW_LENGTH;
pub use compaction::MicroCompactResult;
pub use compaction::TaskInfo;
pub use compaction::TaskStatusRestoration;
pub use compaction::ThresholdStatus;
pub use compaction::ToolResultCandidate;
pub use compaction::build_compact_instructions;
pub use compaction::execute_micro_compact;
pub use compaction::format_restoration_with_tasks;

// Phase 3: Summary formatting and context restoration
pub use compaction::build_token_breakdown;
pub use compaction::create_compact_boundary_message;
pub use compaction::create_invoked_skills_attachment;
pub use compaction::format_summary_with_transcript;
pub use compaction::wrap_hook_additional_context;

// Re-export protocol types used in compaction
pub use compaction::CompactBoundaryMetadata;
pub use compaction::CompactTelemetry;
pub use compaction::CompactTrigger;
pub use compaction::HookAdditionalContext;
pub use compaction::MemoryAttachment;
pub use compaction::PersistedToolResult;
pub use compaction::TokenBreakdown;

// Re-export backwards-compatible constant names
pub use compaction::CONTEXT_RESTORATION_BUDGET;
pub use compaction::CONTEXT_RESTORATION_MAX_FILES;
pub use compaction::MIN_MICRO_COMPACT_SAVINGS;
pub use compaction::RECENT_TOOL_RESULTS_TO_KEEP;
pub use driver::AgentLoop;
pub use driver::AgentLoopBuilder;
pub use fallback::FallbackAttempt;
pub use fallback::FallbackConfig;
pub use fallback::FallbackState;
pub use result::LoopResult;
pub use result::StopReason;
pub use session_memory_agent::ExtractionResult;
pub use session_memory_agent::SessionMemoryExtractionAgent;

// Re-export LoopConfig and AgentStatus from cocode-protocol
pub use cocode_protocol::AgentStatus;
pub use cocode_protocol::LoopConfig;
