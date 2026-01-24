//! SpawnAgent - Full Codex agent with loop driver.
//!
//! This module implements the SpawnAgent type which wraps a Codex session
//! with loop-based execution control.
//!
//! Note: Merge functionality has been moved to the framework level
//! (`crate::spawn_task::merge`) since it's generic across all task types.

mod agent;

pub use agent::SpawnAgent;
pub use agent::SpawnAgentContext;
pub use agent::SpawnAgentParams;

// Re-export merge types from framework level for backward compatibility
pub use super::merge::ConflictInfo;
pub use super::merge::MergeRequest;
pub use super::merge::build_merge_prompt;
