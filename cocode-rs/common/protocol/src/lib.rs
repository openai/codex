//! Protocol types for cocode multi-provider SDK.
//!
//! This crate provides the foundational types used across the cocode ecosystem:
//! - Model capabilities and reasoning levels
//! - Model configuration types
//! - Provider type definitions
//! - Shell and truncation policies
//! - MCP protocol types
//! - Permission and approval types
//! - Core loop configuration and events
//! - Tool execution types
//! - Query and compaction tracking
//! - Agent status and correlation tracking

pub mod agent_status;
pub mod attachment_config;
pub mod compact_config;
pub mod correlation;
pub mod execution;
pub mod features;
pub mod loop_config;
pub mod loop_event;
pub mod mcp_config;
pub mod model;
pub mod path_config;
pub mod permission;
pub mod plan_config;
pub mod protocol;
pub mod provider;
pub mod queue;
pub mod sandbox;
pub mod thinking;
pub mod tool_config;
pub mod tool_types;
pub mod tracking;

// Model types
pub use model::Capability;
pub use model::ConfigShellToolType;
pub use model::ModelInfo;
pub use model::ModelRole;
pub use model::ModelRoles;
pub use model::ModelSpec;
pub use model::ModelSpecParseError;
pub use model::ReasoningEffort;
pub use model::RoleSelection;
pub use model::RoleSelections;
pub use model::TruncationMode;
pub use model::TruncationPolicyConfig;
pub use model::nearest_effort;
pub use model::resolve_provider_type;

// Execution context types
pub use execution::AgentKind;
pub use execution::ExecutionIdentity;
pub use execution::InferenceContext;

// Provider types
pub use provider::ProviderInfo;
pub use provider::ProviderModel;
pub use provider::ProviderType;
pub use provider::WireApi;

// Feature types
pub use features::Feature;
pub use features::FeatureSpec;
pub use features::Features;
pub use features::Stage;
pub use features::all_features;
pub use features::feature_for_key;
pub use features::is_known_feature_key;

// Sandbox types
pub use sandbox::SandboxMode;

// Permission types
pub use permission::ApprovalRequest;
pub use permission::PermissionBehavior;
pub use permission::PermissionCheckResult;
pub use permission::PermissionMode;
pub use permission::PermissionResult;
pub use permission::RiskSeverity;
pub use permission::RiskType;
pub use permission::SecurityRisk;

// Loop config types
pub use loop_config::CacheBreakpoint;
pub use loop_config::CacheType;
pub use loop_config::FileRestorationPriority;
pub use loop_config::LoopConfig;
pub use loop_config::PromptCachingConfig;
pub use loop_config::SessionMemoryConfig;
pub use loop_config::StallDetectionConfig;
pub use loop_config::StallRecovery;

// Loop event types
pub use loop_event::AbortReason;
pub use loop_event::AgentProgress;
pub use loop_event::ApiErrorInfo;
pub use loop_event::AttachmentType;
pub use loop_event::CompactBoundaryMetadata;
pub use loop_event::CompactTelemetry;
pub use loop_event::CompactTrigger;
pub use loop_event::HookAdditionalContext;
pub use loop_event::HookEventType;
pub use loop_event::LoopError;
pub use loop_event::LoopEvent;
pub use loop_event::McpServerInfo;
pub use loop_event::McpStartupStatus;
pub use loop_event::MemoryAttachment;
pub use loop_event::PersistedToolResult;
pub use loop_event::RawStreamEvent;
pub use loop_event::RetryInfo;
pub use loop_event::TaskProgress;
pub use loop_event::TaskType;
pub use loop_event::TokenBreakdown;
pub use loop_event::TokenUsage;
pub use loop_event::TombstonedMessage;
pub use loop_event::ToolProgressInfo;
pub use loop_event::ToolResultContent;

// Tool types
pub use tool_types::ConcurrencySafety;
pub use tool_types::ContextModifier;
pub use tool_types::QueuedCommand;
pub use tool_types::ToolOutput;
pub use tool_types::ValidationError;
pub use tool_types::ValidationResult;

// Queue types (user input during streaming)
pub use queue::UserQueuedCommand;

// Tracking types
pub use tracking::AutoCompactTracking;
pub use tracking::FileChange;
pub use tracking::FileChangeType;
pub use tracking::FileReadInfo;
pub use tracking::QueryTracking;

// Extended config types
pub use attachment_config::AttachmentConfig;
pub use compact_config::CompactConfig;
pub use compact_config::FileRestorationConfig;
pub use compact_config::KeepWindowConfig;
pub use compact_config::SessionMemoryExtractionConfig;
// Keep window constants
pub use compact_config::DEFAULT_EXCLUDED_PATTERNS;
pub use compact_config::DEFAULT_KEEP_WINDOW_MAX_TOKENS;
pub use compact_config::DEFAULT_KEEP_WINDOW_MIN_TEXT_MESSAGES;
pub use compact_config::DEFAULT_KEEP_WINDOW_MIN_TOKENS;
// Session memory constants
pub use compact_config::DEFAULT_EXTRACTION_COOLDOWN_SECS;
pub use compact_config::DEFAULT_SESSION_MEMORY_MAX_TOKENS;
pub use compact_config::DEFAULT_SESSION_MEMORY_MIN_TOKENS;
// Session memory extraction agent constants
pub use compact_config::DEFAULT_EXTRACTION_MAX_SUMMARY_TOKENS;
pub use compact_config::DEFAULT_EXTRACTION_MIN_TOKENS_BETWEEN;
pub use compact_config::DEFAULT_EXTRACTION_MIN_TOKENS_TO_INIT;
pub use compact_config::DEFAULT_EXTRACTION_TOOL_CALLS_BETWEEN;
// Context restoration constants
pub use compact_config::DEFAULT_CONTEXT_RESTORE_BUDGET;
pub use compact_config::DEFAULT_CONTEXT_RESTORE_MAX_FILES;
pub use compact_config::DEFAULT_MAX_TOKENS_PER_FILE;
// Threshold control constants
pub use compact_config::DEFAULT_ERROR_THRESHOLD_OFFSET;
pub use compact_config::DEFAULT_MIN_BLOCKING_OFFSET;
pub use compact_config::DEFAULT_MIN_TOKENS_TO_PRESERVE;
pub use compact_config::DEFAULT_WARNING_THRESHOLD_OFFSET;
// Micro-compact constants
pub use compact_config::DEFAULT_MICRO_COMPACT_MIN_SAVINGS;
pub use compact_config::DEFAULT_MICRO_COMPACT_THRESHOLD;
pub use compact_config::DEFAULT_RECENT_TOOL_RESULTS_TO_KEEP;
// Full compact constants
pub use compact_config::DEFAULT_MAX_COMPACT_OUTPUT_TOKENS;
pub use compact_config::DEFAULT_MAX_SUMMARY_RETRIES;
pub use compact_config::DEFAULT_TOKEN_SAFETY_MARGIN;
pub use compact_config::DEFAULT_TOKENS_PER_IMAGE;
pub use path_config::PathConfig;
pub use plan_config::DEFAULT_PLAN_AGENT_COUNT;
pub use plan_config::DEFAULT_PLAN_EXPLORE_AGENT_COUNT;
pub use plan_config::MAX_AGENT_COUNT;
pub use plan_config::MIN_AGENT_COUNT;
pub use plan_config::PlanModeConfig;
pub use thinking::ThinkingLevel;
pub use tool_config::ApplyPatchToolType;
pub use tool_config::DEFAULT_MAX_RESULT_SIZE;
pub use tool_config::DEFAULT_MAX_TOOL_CONCURRENCY;
pub use tool_config::DEFAULT_RESULT_PREVIEW_SIZE;
pub use tool_config::ToolConfig;

// MCP config types
pub use mcp_config::McpAutoSearchConfig;
pub use mcp_config::McpConfig;
pub use mcp_config::McpToolCacheConfig;
// MCP config constants
pub use mcp_config::DEFAULT_AUTOSEARCH_CONTEXT_THRESHOLD;
pub use mcp_config::DEFAULT_AUTOSEARCH_MIN_CONTEXT_WINDOW;
pub use mcp_config::DEFAULT_CHARS_PER_TOKEN;
pub use mcp_config::DEFAULT_TOOL_CACHE_TTL_SECS;

// Agent status types
pub use agent_status::AgentStatus;

// Correlation types
pub use correlation::CorrelatedEvent;
pub use correlation::SubmissionId;
