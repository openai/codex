//! SpawnTask framework for long-running tasks.
//!
//! This module provides a generic framework for spawning tasks with
//! unified lifecycle management. Supports multiple task types through
//! the `SpawnTask` trait.
//!
//! # Task Types
//!
//! - **SpawnAgent**: Full Codex agent with loop driver
//! - **SpawnWorkflow**: YAML workflow executor (future)
//!
//! # Features
//!
//! - **Unified lifecycle**: One manager for all task types
//! - **Worktree support**: Framework-level git worktree for ALL types
//! - **Continue-on-error**: Iterations continue after single failure
//! - **File persistence**: Task metadata in ~/.codex/spawn-tasks/

use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;

pub mod agent;
pub mod command;
mod log_sink;
mod manager;
pub mod merge;
mod metadata;
pub mod plan_fork;
mod worktree;

pub use command::SpawnCommand;
pub use command::SpawnCommandArgs;
pub use command::parse_spawn_command;
pub use log_sink::LogFileSink;
pub use manager::SpawnTaskManager;
pub use merge::ConflictInfo;
pub use merge::MergeRequest;
pub use merge::build_merge_prompt;
pub use metadata::ExecutionResult;
pub use metadata::SpawnTaskMetadata;
pub use metadata::delete_metadata;
pub use metadata::list_metadata;
pub use metadata::load_metadata;
pub use metadata::metadata_path;
pub use metadata::save_metadata;
pub use metadata::tasks_dir;
pub use worktree::WorktreeInfo;
pub use worktree::WorktreeManager;

/// Task type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpawnTaskType {
    /// Full Codex agent with loop driver.
    Agent,
    /// YAML workflow executor (future).
    Workflow,
}

impl std::fmt::Display for SpawnTaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agent => write!(f, "agent"),
            Self::Workflow => write!(f, "workflow"),
        }
    }
}

/// Status of a spawn task (unified for all types).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpawnTaskStatus {
    /// Task is currently running.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed with error.
    Failed,
    /// Task was cancelled by user.
    Cancelled,
}

impl std::fmt::Display for SpawnTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Result of spawn task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnTaskResult {
    /// Task ID.
    pub task_id: String,
    /// Final status.
    pub status: SpawnTaskStatus,
    /// Iterations completed (if applicable).
    pub iterations_completed: i32,
    /// Iterations that failed (continue-on-error).
    pub iterations_failed: i32,
    /// Error message if failed.
    pub error_message: Option<String>,
}

/// Common trait for all spawnable task types.
///
/// This trait provides a generic interface for different task implementations
/// (SpawnAgent, SpawnWorkflow, etc.) while sharing unified lifecycle management.
pub trait SpawnTask: Send + Sync {
    /// Unique task identifier.
    fn task_id(&self) -> &str;

    /// Task type (Agent, Workflow, etc.).
    fn task_type(&self) -> SpawnTaskType;

    /// Set the working directory (called by manager when worktree is created).
    fn set_cwd(&mut self, cwd: PathBuf);

    /// Get the cancellation token for this task.
    fn cancellation_token(&self) -> &CancellationToken;

    /// Start execution (returns AbortOnDropHandle for auto-cleanup).
    ///
    /// Uses AbortOnDropHandle to ensure task is automatically aborted
    /// when the handle is dropped, consistent with codex-rs task patterns.
    fn spawn(self: Box<Self>) -> AbortOnDropHandle<SpawnTaskResult>;

    /// Get metadata for persistence.
    fn metadata(&self) -> SpawnTaskMetadata;
}
