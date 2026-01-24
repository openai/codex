# SpawnTask Design Document

> SpawnTask framework for spawning long-running tasks with unified lifecycle management in codex-rs.

**Related Documents:**
- [Overview & Implementation Guide](./loop-spawn-overview.md)
- [Loop Driver Design](./loop-driver.md)

---

## Table of Contents

1. [Overview](#1-overview)
2. [SpawnTask Trait](#2-spawntask-trait)
3. [SpawnTaskMetadata](#3-spawntaskmetadata)
4. [SpawnTaskManager](#4-spawntaskmanager)
5. [WorktreeManager (Framework-Level)](#5-worktreemanager-framework-level)
6. [SpawnAgent Implementation](#6-spawnagent-implementation)
7. [Module Exports](#7-module-exports)
8. [System Reminder Integration](#8-system-reminder-integration)
9. [LLM-Driven Merge Command](#9-llm-driven-merge-command)
10. [Protocol Events](#10-protocol-events)
11. [TUI Integration](#11-tui-integration)
12. [Auto PR Creation](#12-auto-pr-creation)

---

## 1. Overview

SpawnTask is a **generic framework** for spawning long-running tasks with unified lifecycle management. It supports multiple task types through the `SpawnTask` trait.

| Feature | Description |
|---------|-------------|
| **Extensible Design** | `SpawnTask` trait with multiple implementations |
| **Unified Lifecycle** | One manager for all task types (start/kill/list/drop) |
| **Worktree Support** | Framework-level git worktree for ALL spawn types |
| **Persistent Metadata** | Stored in `~/.codex/spawn-tasks/` |

### Current Task Types

| Type | Description | Implementation |
|------|-------------|----------------|
| **SpawnAgent** | Full Codex agent with loop driver | Phase 1 (current) |
| **SpawnWorkflow** | YAML workflow executor | Future |

### Command Syntax

```bash
# SpawnAgent (default) - worktree enabled by default
/spawn --name task1 --iter 5 implement user auth   # 5 iterations
/spawn --name task1 --time 1h fix all bugs         # 1 hour duration
/spawn --name task1 --iter 5 --noworktree do task  # Disable worktree

# Future: SpawnWorkflow (same worktree behavior)
/spawn --workflow file.yaml --name task2           # With worktree (default)
/spawn --workflow file.yaml --name task2 --noworktree  # Without worktree

# Management commands (unified for ALL task types)
/spawn --list                                      # List all tasks
/spawn --status task1                              # Show details
/spawn --kill task1                                # Stop running
/spawn --drop task1                                # Delete metadata
```

**Name rules:** No spaces allowed (only `a-z`, `0-9`, `-`, `_`)
**Query:** All remaining text after flags is treated as the query (no quotes needed)

---

## 2. SpawnTask Trait

**File:** `codex-rs/core/src/spawn_task/mod.rs`

```rust
use std::path::PathBuf;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use crate::utils::AbortOnDropHandle;  // Reuse existing pattern from codex-rs

/// Task type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpawnTaskType {
    /// Full Codex agent with loop driver.
    Agent,
    /// YAML workflow executor (future).
    Workflow,
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

/// Result of spawn task execution.
#[derive(Debug, Clone)]
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
#[async_trait]
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
```

---

## 3. SpawnTaskMetadata

**File:** `codex-rs/core/src/spawn_task/metadata.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::loop_driver::LoopCondition;
use crate::spawn_task::{SpawnTaskStatus, SpawnTaskType};

/// Metadata for a spawn task, persisted to disk.
///
/// This is a unified metadata structure for ALL task types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnTaskMetadata {
    // === Core fields (all task types) ===
    /// Unique task identifier.
    pub task_id: String,
    /// Task type (Agent, Workflow, etc.).
    pub task_type: SpawnTaskType,
    /// Current status.
    pub status: SpawnTaskStatus,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Completion timestamp (if finished).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    /// Working directory (original project path or worktree path).
    pub cwd: PathBuf,
    /// Error message (if failed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,

    // === Agent-specific fields ===
    /// Loop condition for execution (Agent only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_condition: Option<LoopCondition>,
    /// Original user query (Agent only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_query: Option<String>,
    /// Number of iterations completed (Agent only).
    #[serde(default)]
    pub iterations_completed: i32,
    /// Number of iterations that failed (continue-on-error).
    #[serde(default)]
    pub iterations_failed: i32,

    // === Workflow-specific fields (future) ===
    /// Workflow file path (Workflow only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_path: Option<PathBuf>,

    // === Worktree fields (GENERIC - all task types) ===
    /// Path to git worktree directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<PathBuf>,
    /// Git branch name created for this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_name: Option<String>,
    /// Base branch the worktree was created from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,

    // === Log file (for same-process task event logging) ===
    /// Log file path for task execution events.
    /// Path: ~/.codex/spawn-tasks/logs/<task_id>.log
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_file: Option<PathBuf>,

    // === Execution results ===
    /// Execution result details (populated on completion).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_result: Option<ExecutionResult>,
}

/// Execution result details for a completed spawn task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Whether the task completed successfully.
    pub success: bool,
    /// Git commit hashes created during execution.
    #[serde(default)]
    pub commits: Vec<String>,
    /// Files modified during execution.
    #[serde(default)]
    pub files_modified: Vec<String>,
    /// Summary of work done (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

impl SpawnTaskMetadata {
    /// Mark as completed.
    pub fn mark_completed(&mut self, iterations: i32, failed: i32) {
        self.status = SpawnTaskStatus::Completed;
        self.completed_at = Some(Utc::now());
        self.iterations_completed = iterations;
        self.iterations_failed = failed;
    }

    /// Mark as failed.
    pub fn mark_failed(&mut self, iterations: i32, failed: i32, error: String) {
        self.status = SpawnTaskStatus::Failed;
        self.completed_at = Some(Utc::now());
        self.iterations_completed = iterations;
        self.iterations_failed = failed;
        self.error_message = Some(error);
    }

    /// Mark as cancelled.
    pub fn mark_cancelled(&mut self, iterations: i32, failed: i32) {
        self.status = SpawnTaskStatus::Cancelled;
        self.completed_at = Some(Utc::now());
        self.iterations_completed = iterations;
        self.iterations_failed = failed;
    }

    /// Update iteration count (for progress persistence).
    pub fn update_iterations(&mut self, completed: i32, failed: i32) {
        self.iterations_completed = completed;
        self.iterations_failed = failed;
    }

    /// Set worktree info (called by manager).
    pub fn set_worktree_info(&mut self, worktree_path: PathBuf, branch_name: String, base_branch: String) {
        self.worktree_path = Some(worktree_path);
        self.branch_name = Some(branch_name);
        self.base_branch = Some(base_branch);
    }
}

/// Storage location for spawn task metadata.
const SPAWN_TASKS_DIR: &str = "spawn-tasks";

/// Get the spawn tasks directory.
pub fn tasks_dir(codex_home: &Path) -> PathBuf {
    codex_home.join(SPAWN_TASKS_DIR)
}

/// Get metadata file path for a task.
pub fn metadata_path(codex_home: &Path, task_id: &str) -> PathBuf {
    tasks_dir(codex_home).join(format!("{task_id}.json"))
}

/// Save spawn task metadata to disk.
pub async fn save_metadata(
    codex_home: &Path,
    metadata: &SpawnTaskMetadata,
) -> anyhow::Result<()> {
    let dir = tasks_dir(codex_home);
    fs::create_dir_all(&dir).await?;

    let path = metadata_path(codex_home, &metadata.task_id);
    let content = serde_json::to_string_pretty(metadata)?;
    fs::write(&path, content).await?;

    Ok(())
}

/// Load spawn task metadata from disk.
pub async fn load_metadata(
    codex_home: &Path,
    task_id: &str,
) -> anyhow::Result<SpawnTaskMetadata> {
    let path = metadata_path(codex_home, task_id);
    let content = fs::read_to_string(&path).await?;
    let metadata: SpawnTaskMetadata = serde_json::from_str(&content)?;
    Ok(metadata)
}

/// List all spawn task metadata.
pub async fn list_metadata(codex_home: &Path) -> anyhow::Result<Vec<SpawnTaskMetadata>> {
    let dir = tasks_dir(codex_home);

    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();
    let mut entries = fs::read_dir(&dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "json") {
            if let Ok(content) = fs::read_to_string(&path).await {
                if let Ok(metadata) = serde_json::from_str(&content) {
                    result.push(metadata);
                }
            }
        }
    }

    // Sort by creation time (newest first)
    result.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(result)
}

/// Delete task metadata.
pub async fn delete_metadata(
    codex_home: &Path,
    task_id: &str,
) -> anyhow::Result<()> {
    let path = metadata_path(codex_home, task_id);
    fs::remove_file(&path).await?;
    Ok(())
}
```

---

## 4. SpawnTaskManager

**File:** `codex-rs/core/src/spawn_task/manager.rs`

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::spawn_task::{
    SpawnTask, SpawnTaskResult, SpawnTaskStatus,
    metadata::{
        SpawnTaskMetadata, delete_metadata, list_metadata, load_metadata, save_metadata,
    },
    worktree::{WorktreeManager, WorktreeInfo},
};

/// Running task entry in the registry.
struct RunningTask {
    metadata: SpawnTaskMetadata,
    cancellation_token: CancellationToken,
    handle: AbortOnDropHandle<SpawnTaskResult>,  // Auto-cleanup on drop
}

/// Manager for spawn tasks.
///
/// Provides unified start/stop/list operations for ALL task types.
/// Handles worktree creation/cleanup at the framework level.
pub struct SpawnTaskManager {
    /// In-memory registry of running tasks.
    tasks: Arc<RwLock<HashMap<String, RunningTask>>>,
    /// Path to codex home (~/.codex/).
    codex_home: PathBuf,
    /// Path to project root.
    project_root: PathBuf,
    /// Worktree manager (GENERIC - for all task types).
    worktree_manager: WorktreeManager,
    /// Maximum number of concurrent spawn tasks (default: 5).
    max_concurrent_tasks: i32,
}

impl SpawnTaskManager {
    /// Default maximum concurrent tasks.
    const DEFAULT_MAX_CONCURRENT: i32 = 5;

    /// Create a new spawn task manager with default concurrency limit.
    pub fn new(codex_home: PathBuf, project_root: PathBuf) -> Self {
        Self::with_max_concurrent(codex_home, project_root, Self::DEFAULT_MAX_CONCURRENT)
    }

    /// Create a new spawn task manager with custom concurrency limit.
    pub fn with_max_concurrent(
        codex_home: PathBuf,
        project_root: PathBuf,
        max_concurrent_tasks: i32,
    ) -> Self {
        let worktree_manager = WorktreeManager::new(
            codex_home.clone(),
            project_root.clone(),
        );

        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            codex_home,
            project_root,
            worktree_manager,
            max_concurrent_tasks,
        }
    }

    /// Start a new spawn task.
    ///
    /// # Arguments
    /// * `task` - The task to start (implements SpawnTask trait)
    /// * `use_worktree` - Whether to create a git worktree (default: true)
    /// * `base_branch` - Base branch for worktree (optional)
    ///
    /// # Worktree Behavior (GENERIC - for ALL task types)
    /// - If `use_worktree` is true (default), creates a worktree BEFORE spawning
    /// - The task's cwd is set to the worktree path
    /// - On `drop()`, the worktree is cleaned up
    ///
    /// # Errors
    /// Returns error if task with same ID already exists.
    pub async fn start(
        &self,
        mut task: Box<dyn SpawnTask>,
        use_worktree: bool,
        base_branch: Option<&str>,
    ) -> anyhow::Result<String> {
        let task_id = task.task_id().to_string();

        // Check if task already exists
        {
            let tasks = self.tasks.read().await;
            if tasks.contains_key(&task_id) {
                anyhow::bail!("Task '{task_id}' already exists");
            }
        }

        // Check concurrency limit
        {
            let tasks = self.tasks.read().await;
            let running_count = tasks.values()
                .filter(|t| t.metadata.status == SpawnTaskStatus::Running)
                .count() as i32;
            if running_count >= self.max_concurrent_tasks {
                anyhow::bail!(
                    "Maximum concurrent tasks ({}) reached. Use /spawn --list to see running tasks.",
                    self.max_concurrent_tasks
                );
            }
        }

        // Check for stale metadata
        if let Ok(existing) = load_metadata(&self.codex_home, &task_id).await {
            if existing.status == SpawnTaskStatus::Running {
                anyhow::bail!(
                    "Task '{task_id}' has stale running status. Use /spawn --drop {task_id} first."
                );
            }
        }

        // Get initial metadata
        let mut metadata = task.metadata();

        // ⭐ WORKTREE: Framework-level handling for ALL task types
        let worktree_info = if use_worktree {
            info!(task_id = %task_id, "Creating git worktree for task");
            match self.worktree_manager.create_worktree(&task_id, base_branch).await {
                Ok(info) => {
                    // Update task's cwd to worktree path
                    task.set_cwd(info.worktree_path.clone());
                    // Update metadata with worktree info
                    metadata.set_worktree_info(
                        info.worktree_path.clone(),
                        info.branch_name.clone(),
                        info.base_branch.clone(),
                    );
                    Some(info)
                }
                Err(e) => {
                    anyhow::bail!("Failed to create worktree: {e}");
                }
            }
        } else {
            None
        };

        // Save initial metadata
        if let Err(e) = save_metadata(&self.codex_home, &metadata).await {
            warn!(task_id = %task_id, error = %e, "Failed to save initial metadata");
        }

        // Get cancellation token before spawning
        let token = task.cancellation_token().clone();

        // Spawn the task
        let handle = task.spawn();

        // Register in memory
        {
            let mut tasks = self.tasks.write().await;
            tasks.insert(task_id.clone(), RunningTask {
                metadata,
                cancellation_token: token,
                handle,
            });
        }

        info!(
            task_id = %task_id,
            use_worktree = %use_worktree,
            "Spawn task started"
        );

        Ok(task_id)
    }

    /// Kill a running task.
    pub async fn kill(&self, task_id: &str) -> anyhow::Result<()> {
        let task = {
            let mut tasks = self.tasks.write().await;
            tasks.remove(task_id)
        };

        match task {
            Some(task) => {
                info!(task_id = %task_id, "Killing spawn task");

                // Cancel the token
                task.cancellation_token.cancel();

                // Wait for handle to complete (with timeout)
                tokio::select! {
                    result = task.handle => {
                        match result {
                            Ok(result) => {
                                info!(
                                    task_id = %task_id,
                                    status = ?result.status,
                                    "Task killed successfully"
                                );
                            }
                            Err(e) => {
                                warn!(task_id = %task_id, error = %e, "Task panicked");
                            }
                        }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                        warn!(task_id = %task_id, "Task did not stop within timeout");
                    }
                }

                Ok(())
            }
            None => {
                if load_metadata(&self.codex_home, task_id).await.is_ok() {
                    anyhow::bail!("Task '{task_id}' is not running (use /spawn --drop to remove metadata)")
                } else {
                    anyhow::bail!("Task '{task_id}' not found")
                }
            }
        }
    }

    /// List all tasks (running + persisted).
    pub async fn list(&self) -> anyhow::Result<Vec<SpawnTaskMetadata>> {
        let mut all = list_metadata(&self.codex_home).await?;

        // Update running status from in-memory registry
        let tasks = self.tasks.read().await;
        for metadata in &mut all {
            if tasks.contains_key(&metadata.task_id) {
                metadata.status = SpawnTaskStatus::Running;
            }
        }

        Ok(all)
    }

    /// Get status of a specific task.
    pub async fn status(&self, task_id: &str) -> anyhow::Result<SpawnTaskMetadata> {
        let mut metadata = load_metadata(&self.codex_home, task_id).await?;

        let tasks = self.tasks.read().await;
        if tasks.contains_key(task_id) {
            metadata.status = SpawnTaskStatus::Running;
        }

        Ok(metadata)
    }

    /// Delete task metadata and cleanup worktree.
    ///
    /// # Errors
    /// Returns error if task is still running.
    pub async fn drop(&self, task_id: &str) -> anyhow::Result<()> {
        // Check if running
        {
            let tasks = self.tasks.read().await;
            if tasks.contains_key(task_id) {
                anyhow::bail!("Task '{task_id}' is still running. Use /spawn --kill first.");
            }
        }

        // Load metadata to check for worktree
        if let Ok(metadata) = load_metadata(&self.codex_home, task_id).await {
            // ⭐ WORKTREE: Framework-level cleanup for ALL task types
            if metadata.worktree_path.is_some() {
                info!(task_id = %task_id, "Cleaning up worktree");
                if let Err(e) = self.worktree_manager.cleanup_worktree(task_id).await {
                    warn!(task_id = %task_id, error = %e, "Failed to cleanup worktree");
                    // Continue with metadata deletion even if worktree cleanup fails
                }
            }
        }

        delete_metadata(&self.codex_home, task_id).await?;
        info!(task_id = %task_id, "Task metadata deleted");

        Ok(())
    }
}
```

---

## 5. WorktreeManager (Framework-Level)

**File:** `codex-rs/core/src/spawn_task/worktree.rs`

**Key Insight:** Worktree is a **spawn framework feature**, not task-specific. All spawn types use the same worktree management.

```rust
use std::path::{Path, PathBuf};
use tokio::process::Command;  // ⭐ Use async Command to avoid blocking runtime
use anyhow::Result;
use tracing::{info, warn};

/// Information about a created worktree.
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    /// Task ID this worktree belongs to.
    pub task_id: String,
    /// Path to the worktree directory.
    pub worktree_path: PathBuf,
    /// Git branch name.
    pub branch_name: String,
    /// Base branch it was created from.
    pub base_branch: String,
}

/// Manager for git worktrees used by spawn tasks.
///
/// ⭐ GENERIC: This manager handles worktrees for ALL spawn task types
/// (SpawnAgent, SpawnWorkflow, future types).
pub struct WorktreeManager {
    /// Path to ~/.codex/
    codex_home: PathBuf,
    /// Path to the original project.
    project_root: PathBuf,
}

impl WorktreeManager {
    pub fn new(codex_home: PathBuf, project_root: PathBuf) -> Self {
        Self { codex_home, project_root }
    }

    /// Create a git worktree for a spawn task.
    ///
    /// Creates a new branch with the task_id as name and a worktree
    /// at ~/.codex/spawn-tasks/worktrees/{task_id}/
    ///
    /// Works for ANY spawn task type (Agent, Workflow, etc.)
    pub async fn create_worktree(
        &self,
        task_id: &str,
        base_branch: Option<&str>,
    ) -> Result<WorktreeInfo> {
        let worktree_path = self.worktrees_dir().join(task_id);
        let branch_name = task_id.to_string();

        // Check if worktree already exists
        if worktree_path.exists() {
            anyhow::bail!("Worktree for task '{}' already exists", task_id);
        }

        // Detect base branch if not specified (async)
        let base = match base_branch {
            Some(b) => b.to_string(),
            None => self.detect_default_branch().await,
        };

        info!(
            task_id = %task_id,
            base_branch = %base,
            worktree_path = %worktree_path.display(),
            "Creating git worktree"
        );

        // Create worktree with new branch (async to avoid blocking runtime)
        let output = Command::new("git")
            .current_dir(&self.project_root)
            .args([
                "worktree", "add",
                worktree_path.to_str().unwrap(),
                "-b", &branch_name,
                &base,
            ])
            .output()
            .await?;  // ⭐ Non-blocking await

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create worktree: {}", stderr);
        }

        Ok(WorktreeInfo {
            task_id: task_id.to_string(),
            worktree_path,
            branch_name,
            base_branch: base,
        })
    }

    /// Remove a git worktree and its branch.
    ///
    /// Works for ANY spawn task type (Agent, Workflow, etc.)
    pub async fn cleanup_worktree(&self, task_id: &str) -> Result<()> {
        let worktree_path = self.worktrees_dir().join(task_id);

        info!(task_id = %task_id, "Removing git worktree");

        // Remove worktree (async)
        let output = Command::new("git")
            .current_dir(&self.project_root)
            .args(["worktree", "remove", "--force", worktree_path.to_str().unwrap()])
            .output()
            .await?;  // ⭐ Non-blocking await

        if !output.status.success() {
            warn!(
                task_id = %task_id,
                stderr = %String::from_utf8_lossy(&output.stderr),
                "Failed to remove worktree (may already be removed)"
            );
        }

        // Delete the branch (async)
        let _ = Command::new("git")
            .current_dir(&self.project_root)
            .args(["branch", "-D", task_id])
            .output()
            .await;

        Ok(())
    }

    fn worktrees_dir(&self) -> PathBuf {
        self.codex_home.join("spawn-tasks").join("worktrees")
    }

    /// Detect the default branch (async to avoid blocking runtime).
    async fn detect_default_branch(&self) -> String {
        let output = Command::new("git")
            .current_dir(&self.project_root)
            .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
            .output()
            .await;  // Non-blocking

        if let Ok(output) = output {
            if output.status.success() {
                let s = String::from_utf8_lossy(&output.stdout);
                if let Some(branch) = s.trim().strip_prefix("refs/remotes/origin/") {
                    return branch.to_string();
                }
            }
        }

        "main".to_string()
    }
}
```

### Storage Structure

```
~/.codex/spawn-tasks/
├── task1.json                    # Metadata (any task type)
├── task2.json
├── logs/                         # ⭐ Log files (for same-process event logging)
│   ├── task1.log                 # Events from task1
│   └── task2.log                 # Events from task2
└── worktrees/                    # Git worktrees (for ANY task type)
    ├── task1/                    # Worktree for task1 (e.g., SpawnAgent)
    │   ├── .git
    │   └── (project files)
    └── task2/                    # Worktree for task2 (e.g., SpawnWorkflow)
        ├── .git
        └── (project files)
```

---

## 5.1 LogFileSink (Same-Process Event Logging)

**File:** `codex-rs/core/src/spawn_task/log_sink.rs`

**Context:** SpawnAgent uses `tokio::spawn` (same process, async task). Cannot use traditional stdout/stderr redirection. Instead, we use an Event Sink pattern to write task events to a dedicated log file.

```rust
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Thread-safe log file writer for SpawnTask events.
///
/// Since SpawnAgent runs in the same process (via tokio::spawn),
/// we cannot redirect stdout/stderr. Instead, we explicitly log
/// events to a dedicated file per task.
pub struct LogFileSink {
    file: Arc<Mutex<File>>,
}

impl LogFileSink {
    /// Create a new log file sink.
    ///
    /// Creates parent directories if they don't exist.
    /// Opens file in append mode for crash recovery.
    pub fn new(path: &Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            file: Arc::new(Mutex::new(file)),
        })
    }

    /// Log a message with timestamp.
    pub fn log(&self, msg: &str) {
        if let Ok(mut f) = self.file.lock() {
            let timestamp = chrono::Utc::now().format("%H:%M:%S");
            let _ = writeln!(f, "[{timestamp}] {msg}");
        }
    }

    /// Log iteration progress.
    pub fn log_iteration(&self, iteration: i32, succeeded: i32, failed: i32) {
        self.log(&format!(
            "=== Iteration {iteration} complete: {succeeded} succeeded, {failed} failed ==="
        ));
    }
}

impl Clone for LogFileSink {
    fn clone(&self) -> Self {
        Self {
            file: Arc::clone(&self.file),
        }
    }
}
```

### Log File Content Example

```
[10:30:00] Starting task: feature-auth
[10:30:01] EventMsg::ModelResponse { content: "I'll implement..." }
[10:30:02] EventMsg::ToolCall { name: "shell", args: "mkdir src/auth" }
[10:30:05] EventMsg::ToolResult { success: true }
[10:30:05] === Iteration 0 complete: 1 succeeded, 0 failed ===
[10:30:06] EventMsg::ModelResponse { content: "Now adding tests..." }
[10:30:10] EventMsg::ToolCall { name: "write", path: "src/auth/mod.rs" }
[10:30:15] === Iteration 1 complete: 2 succeeded, 0 failed ===
[10:30:16] Task completed successfully
```

---

## 6. SpawnAgent Implementation

**File:** `codex-rs/core/src/spawn_task/agent/agent.rs`

```rust
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::codex::{Codex, CodexSpawnOk};
use crate::config::Config;
use crate::loop_driver::{LoopCondition, LoopDriver, LoopStopReason};
use crate::spawn_task::{
    SpawnTask, SpawnTaskMetadata, SpawnTaskResult, SpawnTaskStatus, SpawnTaskType,
    metadata::save_metadata,
};
use crate::subagent::definition::ApprovalMode;

/// Parameters for creating a SpawnAgent.
#[derive(Debug, Clone)]
pub struct SpawnAgentParams {
    /// Unique task ID.
    pub task_id: String,
    /// Loop condition.
    pub loop_condition: LoopCondition,
    /// User query.
    pub query: String,
    /// Working directory.
    pub cwd: PathBuf,
    /// Custom loop prompt (optional).
    pub custom_loop_prompt: Option<String>,
    /// Approval mode (default: DontAsk for autonomous execution).
    pub approval_mode: ApprovalMode,
}

/// SpawnAgent - Full Codex agent with loop driver.
///
/// Implements `SpawnTask` trait for unified lifecycle management.
pub struct SpawnAgent {
    params: SpawnAgentParams,
    cancellation_token: CancellationToken,
    codex_home: PathBuf,
    cwd: PathBuf,
    config: Config,
}

impl SpawnAgent {
    pub fn new(
        params: SpawnAgentParams,
        codex_home: PathBuf,
        config: Config,
    ) -> Self {
        let cwd = params.cwd.clone();
        Self {
            params,
            cancellation_token: CancellationToken::new(),
            codex_home,
            cwd,
            config,
        }
    }
}

impl SpawnTask for SpawnAgent {
    fn task_id(&self) -> &str {
        &self.params.task_id
    }

    fn task_type(&self) -> SpawnTaskType {
        SpawnTaskType::Agent
    }

    fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
    }

    fn cancellation_token(&self) -> &CancellationToken {
        &self.cancellation_token
    }

    fn metadata(&self) -> SpawnTaskMetadata {
        SpawnTaskMetadata {
            task_id: self.params.task_id.clone(),
            task_type: SpawnTaskType::Agent,
            status: SpawnTaskStatus::Running,
            created_at: chrono::Utc::now(),
            completed_at: None,
            cwd: self.cwd.clone(),
            error_message: None,
            loop_condition: Some(self.params.loop_condition.clone()),
            user_query: Some(self.params.query.clone()),
            iterations_completed: 0,
            iterations_failed: 0,
            workflow_path: None,
            worktree_path: None,
            branch_name: None,
            base_branch: None,
            execution_result: None,
        }
    }

    fn spawn(self: Box<Self>) -> AbortOnDropHandle<SpawnTaskResult> {
        let params = self.params;
        let token = self.cancellation_token;
        let codex_home = self.codex_home;
        let cwd = self.cwd;
        let config = self.config;

        // Wrap tokio::spawn with AbortOnDropHandle for auto-cleanup
        AbortOnDropHandle::new(tokio::spawn(async move {
            // ⭐ Create log file sink for this task
            let log_path = codex_home
                .join("spawn-tasks")
                .join("logs")
                .join(format!("{}.log", params.task_id));
            let sink = LogFileSink::new(&log_path).ok();

            if let Some(ref s) = sink {
                s.log(&format!("Starting SpawnAgent: {}", params.task_id));
                s.log(&format!("Condition: {}", params.loop_condition.display()));
            }

            info!(
                task_id = %params.task_id,
                condition = %params.loop_condition.display(),
                "Starting SpawnAgent"
            );

            // Build config for spawned agent
            let mut spawn_config = config.clone();
            spawn_config.cwd = cwd;

            // Spawn Codex session
            let CodexSpawnOk { codex, .. } = match Codex::spawn(
                spawn_config,
                // ... auth_manager, models_manager, skills_manager
                InitialHistory::New,
                SessionSource::SpawnAgent(/* ... */),
            ).await {
                Ok(spawn_ok) => spawn_ok,
                Err(e) => {
                    let error_msg = format!("Failed to spawn Codex: {e}");
                    if let Some(ref s) = sink {
                        s.log(&format!("ERROR: {error_msg}"));
                    }
                    error!(task_id = %params.task_id, error = %error_msg);
                    return SpawnTaskResult {
                        task_id: params.task_id,
                        status: SpawnTaskStatus::Failed,
                        iterations_completed: 0,
                        iterations_failed: 0,
                        error_message: Some(error_msg),
                    };
                }
            };

            // Create loop driver with log sink
            let mut driver = LoopDriver::new(params.loop_condition.clone(), token.clone());

            if let Some(prompt) = params.custom_loop_prompt {
                driver = driver.with_custom_prompt(prompt);
            }

            // ⭐ Set up progress callback with log sink
            if let Some(ref s) = sink {
                let sink_clone = s.clone();
                driver = driver.with_progress_callback(move |progress| {
                    sink_clone.log_iteration(
                        progress.iteration,
                        progress.succeeded,
                        progress.failed,
                    );
                });
            }

            // Run with loop (continue-on-error enabled)
            // Pass sink for event logging
            let result = driver.run_with_loop(&codex, &params.query, sink.as_ref()).await;

            // Determine final status
            let status = match result.stop_reason {
                LoopStopReason::Completed | LoopStopReason::DurationElapsed => {
                    SpawnTaskStatus::Completed
                }
                LoopStopReason::Cancelled => SpawnTaskStatus::Cancelled,
                LoopStopReason::TaskAborted => SpawnTaskStatus::Failed,
            };

            if let Some(ref s) = sink {
                s.log(&format!("Task completed with status: {:?}", status));
            }

            info!(
                task_id = %params.task_id,
                status = ?status,
                iterations_completed = result.iterations_succeeded,
                iterations_failed = result.iterations_failed,
                "SpawnAgent finished"
            );

            SpawnTaskResult {
                task_id: params.task_id,
                status,
                iterations_completed: result.iterations_succeeded,
                iterations_failed: result.iterations_failed,
                error_message: None,
            }
        }))  // Close both tokio::spawn and AbortOnDropHandle::new
    }
}
```

---

## 7. Module Exports

**File:** `codex-rs/core/src/spawn_task/mod.rs`

```rust
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

mod manager;
mod metadata;
mod worktree;
pub mod agent;
pub mod workflow;  // Future placeholder

pub use manager::SpawnTaskManager;
pub use metadata::{
    SpawnTaskMetadata, delete_metadata, list_metadata,
    load_metadata, save_metadata, metadata_path, tasks_dir,
};
pub use worktree::{WorktreeManager, WorktreeInfo};

// Re-export trait and types from mod.rs
// (trait defined above)
```

---

## 8. System Reminder Integration

The system reminder integration follows the same pattern as before, but uses unified types.

### SpawnTaskStore (Global Pattern)

```rust
/// Store for tracking spawn task status for system reminder injection.
///
/// ⭐ GENERIC: Tracks ALL spawn task types (Agent, Workflow, etc.)
#[derive(Debug, Default)]
pub struct SpawnTaskStore {
    tasks: DashMap<String, SpawnTaskEntry>,
}

impl SpawnTaskStore {
    /// Get tasks for system reminder injection.
    pub fn list_for_reminder(
        &self,
        conversation_id: Option<&ConversationId>,
    ) -> Vec<BackgroundTaskInfo> {
        self.tasks
            .iter()
            .filter(|entry| /* filter logic */)
            .map(|entry| BackgroundTaskInfo {
                task_id: entry.task_id.clone(),
                task_type: BackgroundTaskType::SpawnTask,  // Generic type
                description: entry.description.clone(),
                // ...
            })
            .collect()
    }
}
```

---

## 9. LLM-Driven Merge Command

Merge command is **SpawnAgent-specific** (requires worktree branches).

**File:** `codex-rs/core/src/spawn_task/agent/merge.rs`

```rust
/// Build merge prompt for main agent to execute.
///
/// This is SpawnAgent-specific functionality.
pub fn build_merge_prompt(
    request: &MergeRequest,
    tasks_metadata: &[SpawnTaskMetadata],
    conflict_info: Option<&ConflictInfo>,
) -> String {
    // ... same as before, but uses SpawnTaskMetadata
}
```

---

## 10. Protocol Events

```rust
// Unified events for all spawn task types
pub struct SpawnTaskStartedEvent {
    pub task_id: String,
    pub task_type: String,  // "agent", "workflow"
    // ...
}

pub struct SpawnTaskProgressEvent {
    pub task_id: String,
    pub task_type: String,
    pub iterations_completed: i32,
    pub iterations_failed: i32,  // Continue-on-error tracking
    // ...
}

pub struct SpawnTaskCompleteEvent {
    pub task_id: String,
    pub task_type: String,
    pub status: String,
    pub iterations_completed: i32,
    pub iterations_failed: i32,
    // ...
}
```

---

## 11. TUI Integration

**File:** `codex-rs/tui/src/spawn_command_ext.rs`

The TUI handler supports both current (SpawnAgent) and future (SpawnWorkflow) task types:

```rust
/// Handle /spawn command.
pub fn handle_spawn_command(widget: &mut ChatWidget) {
    let args: Vec<&str> = /* parse args */;

    match args[1] {
        "--list" => handle_list(widget),        // Lists ALL task types
        "--status" => handle_status(widget, id), // Works for ALL types
        "--kill" => handle_kill(widget, id),    // Works for ALL types
        "--drop" => handle_drop(widget, id),    // Works for ALL types + cleanup worktree
        "--merge" => handle_merge(widget, args), // SpawnAgent-specific
        "--workflow" => handle_workflow_start(widget, args), // Future
        "--name" => handle_agent_start(widget, args), // SpawnAgent (default)
        _ => show_help(widget),
    }
}
```

### Command Examples

```bash
# SpawnAgent (default) - with worktree
/spawn --name task1 --iter 5 implement feature

# SpawnAgent - without worktree
/spawn --name task1 --iter 5 --noworktree quick fix

# SpawnAgent - with auto PR creation
/spawn --name task1 --iter 5 --pr implement user auth

# SpawnAgent - with custom PR title
/spawn --name task1 --iter 5 --pr "Add authentication" implement user auth

# Future: SpawnWorkflow - with worktree (default)
/spawn --workflow pipeline.yaml --name task2

# Future: SpawnWorkflow - without worktree
/spawn --workflow pipeline.yaml --name task2 --noworktree

# Management (unified for ALL types)
/spawn --list                    # Shows Agent AND Workflow tasks
/spawn --status task1            # Works for any type
/spawn --kill task1              # Works for any type
/spawn --drop task1              # Works for any type + cleans worktree

# Merge (SpawnAgent-specific)
/spawn --merge task1,task2 select the best implementation
```

---

## 12. Auto PR Creation

SpawnAgent supports automatic pull request creation after task completion via the `--pr` flag.

### Command Syntax

```bash
# Create PR with task query as title
/spawn --name task1 --iter 5 --pr implement user auth

# Create PR with custom title
/spawn --name task1 --iter 5 --pr "Add authentication" implement user auth
```

**Requirements:**
- Requires `gh` CLI to be installed and authenticated
- Requires worktree mode (fails if `--noworktree` is used)
- Branch is automatically pushed to origin before PR creation

### Updated SpawnAgentParams

**File:** `codex-rs/core/src/spawn_task/agent/agent.rs`

```rust
/// Parameters for creating a SpawnAgent.
#[derive(Debug, Clone)]
pub struct SpawnAgentParams {
    /// Unique task ID.
    pub task_id: String,
    /// Loop condition.
    pub loop_condition: LoopCondition,
    /// User query.
    pub query: String,
    /// Working directory.
    pub cwd: PathBuf,
    /// Custom loop prompt (optional).
    pub custom_loop_prompt: Option<String>,
    /// Approval mode (default: DontAsk for autonomous execution).
    pub approval_mode: ApprovalMode,
    /// If true, create PR after task completion (requires worktree).
    pub create_pr: bool,
    /// Custom PR title (optional, defaults to task query).
    pub pr_title: Option<String>,
}
```

### Updated SpawnTaskResult

```rust
/// Result of spawn task execution.
#[derive(Debug, Clone)]
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
    /// PR URL if --pr was used and PR creation succeeded.
    pub pr_url: Option<String>,
}
```

### PR Creation Logic

Add to end of `SpawnAgent::spawn()` async block, after loop completion:

```rust
// After loop completion, create PR if requested
if params.create_pr {
    if metadata.worktree_path.is_some() {
        if let Some(ref s) = sink {
            s.log("Creating pull request...");
        }

        match create_pull_request(&params, &cwd).await {
            Ok(pr_url) => {
                if let Some(ref s) = sink {
                    s.log(&format!("PR created: {pr_url}"));
                }
                // Store PR URL in result
            }
            Err(e) => {
                if let Some(ref s) = sink {
                    s.log(&format!("Failed to create PR: {e}"));
                }
            }
        }
    } else {
        if let Some(ref s) = sink {
            s.log("Warning: --pr requires worktree (--noworktree was used)");
        }
    }
}
```

### Helper Function

```rust
/// Create a pull request for the completed spawn task.
async fn create_pull_request(params: &SpawnAgentParams, cwd: &Path) -> anyhow::Result<String> {
    use tokio::process::Command;

    // Push branch to origin
    let push_output = Command::new("git")
        .current_dir(cwd)
        .args(["push", "-u", "origin", &params.task_id])
        .output()
        .await?;

    if !push_output.status.success() {
        anyhow::bail!(
            "Failed to push branch: {}",
            String::from_utf8_lossy(&push_output.stderr)
        );
    }

    // Create PR via gh CLI
    let title = params.pr_title.as_deref().unwrap_or(&params.query);
    let body = format!(
        "Automated PR created by SpawnAgent task: {}\n\n## Query\n{}\n\n---\n_Created by codex spawn-task_",
        params.task_id, params.query
    );

    let pr_output = Command::new("gh")
        .current_dir(cwd)
        .args([
            "pr", "create",
            "--title", title,
            "--body", &body,
        ])
        .output()
        .await?;

    if !pr_output.status.success() {
        anyhow::bail!(
            "Failed to create PR: {}",
            String::from_utf8_lossy(&pr_output.stderr)
        );
    }

    // Extract PR URL from output
    let pr_url = String::from_utf8_lossy(&pr_output.stdout).trim().to_string();
    Ok(pr_url)
}
```

### Log Output Example

```
[10:30:00] Starting SpawnAgent: feature-auth
[10:30:01] Condition: 5 iterations
[10:30:05] === Iteration 0 complete: 1 succeeded, 0 failed ===
[10:30:10] === Iteration 1 complete: 2 succeeded, 0 failed ===
...
[10:31:00] === Iteration 4 complete: 5 succeeded, 0 failed ===
[10:31:01] Creating pull request...
[10:31:03] PR created: https://github.com/user/repo/pull/42
[10:31:03] Task completed with status: Completed
```
