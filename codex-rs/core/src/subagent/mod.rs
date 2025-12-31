//! Subagent system for spawning specialized child agents.
//!
//! This module provides Claude Code compatible subagent functionality:
//! - Task tool for spawning agents (see tools/handlers/ext/subagent.rs)
//! - TaskOutput tool for retrieving background results
//! - Built-in Explore and Plan agents
//! - Custom agent definitions via YAML/MD files
//!
//! Note: The actual Task/TaskOutput handlers are in tools/handlers/ext/subagent.rs
//! to integrate with the main tool registry system.

mod approval;
mod background;
mod config_builder;
mod definition;
mod delegate;
mod error;
mod events;
mod events_bridge;
mod registry;
mod result;
mod stores;
mod transcript;

// Re-export public types
pub use approval::ApprovalRouter;
pub use background::BackgroundTask;
pub use background::BackgroundTaskStatus;
pub use background::BackgroundTaskStore;
pub use config_builder::SubagentConfig;
pub use config_builder::SubagentConfigBuilder;
pub use definition::AgentDefinition;
pub use definition::AgentRunConfig;
pub use definition::AgentSource;
pub use definition::ApprovalMode;
pub use definition::InputConfig;
pub use definition::InputDefinition;
pub use definition::InputType;
pub use definition::ModelConfig;
pub use definition::OutputConfig;
pub use definition::PromptConfig;
pub use definition::ThinkingLevel;
pub use definition::ToolAccess;
pub use definition::get_builtin_agents;
pub(crate) use delegate::run_subagent_delegate;
pub use error::SubagentErr;
pub use events::SubagentActivityEvent;
pub use events::SubagentEventType;
pub use events::SubagentProgress;
pub use events_bridge::SubagentEventBridge;
pub use registry::AgentRegistry;
pub use result::SubagentResult;
pub use result::SubagentStatus;
pub use result::TokenUsage;
pub use stores::ApprovedPlan;
pub use stores::SubagentStores;
pub use stores::cleanup_stores;
pub use stores::get_or_create_stores;
pub use stores::get_stores;
pub use transcript::AgentTranscript;
pub use transcript::TranscriptMessage;
pub use transcript::TranscriptStore;

// Re-export ToolFilter and constants from spec_ext (unified tool filtering)
pub use crate::tools::spec_ext::ALWAYS_BLOCKED_TOOLS;
pub use crate::tools::spec_ext::ToolFilter;
