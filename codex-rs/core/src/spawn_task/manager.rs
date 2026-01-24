use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use codex_lsp::LspServerManager;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::warn;

use super::SpawnTask;
use super::SpawnTaskMetadata;
use super::SpawnTaskStatus;
use super::metadata::delete_metadata;
use super::metadata::list_metadata;
use super::metadata::load_metadata;
use super::metadata::save_metadata;
use super::worktree::WorktreeManager;

/// Running task entry in the registry.
struct RunningTask {
    metadata: SpawnTaskMetadata,
    cancellation_token: CancellationToken,
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
    /// LSP server manager for cleanup when worktree is deleted.
    lsp_manager: Option<Arc<LspServerManager>>,
}

impl SpawnTaskManager {
    /// Default maximum concurrent tasks.
    pub const DEFAULT_MAX_CONCURRENT: i32 = 5;

    /// Create a new spawn task manager with default concurrency limit.
    pub fn new(codex_home: PathBuf, project_root: PathBuf) -> Self {
        Self::with_options(codex_home, project_root, Self::DEFAULT_MAX_CONCURRENT, None)
    }

    /// Create a new spawn task manager with custom concurrency limit.
    pub fn with_max_concurrent(
        codex_home: PathBuf,
        project_root: PathBuf,
        max_concurrent_tasks: i32,
    ) -> Self {
        Self::with_options(codex_home, project_root, max_concurrent_tasks, None)
    }

    /// Create a new spawn task manager with all options.
    pub fn with_options(
        codex_home: PathBuf,
        project_root: PathBuf,
        max_concurrent_tasks: i32,
        lsp_manager: Option<Arc<LspServerManager>>,
    ) -> Self {
        let worktree_manager = WorktreeManager::new(codex_home.clone(), project_root.clone());

        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            codex_home,
            project_root,
            worktree_manager,
            max_concurrent_tasks,
            lsp_manager,
        }
    }

    /// Get the codex home path.
    pub fn codex_home(&self) -> &PathBuf {
        &self.codex_home
    }

    /// Get the project root path.
    pub fn project_root(&self) -> &PathBuf {
        &self.project_root
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

        // Validate task_id format (no spaces, only a-z, 0-9, -, _)
        if !Self::is_valid_task_id(&task_id) {
            anyhow::bail!(
                "Invalid task name '{}'. Only lowercase letters, numbers, hyphens, and underscores are allowed.",
                task_id
            );
        }

        // Check if task already exists
        {
            let tasks = self.tasks.read().await;
            if tasks.contains_key(&task_id) {
                anyhow::bail!("Task '{}' already exists", task_id);
            }
        }

        // Check concurrency limit
        {
            let tasks = self.tasks.read().await;
            let running_count = tasks
                .values()
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
                    "Task '{}' has stale running status. Use /spawn --drop {} first.",
                    task_id,
                    task_id
                );
            }
        }

        // Get initial metadata
        let mut metadata = task.metadata();

        // WORKTREE: Framework-level handling for ALL task types
        if use_worktree {
            info!(task_id = %task_id, "Creating git worktree for task");
            match self
                .worktree_manager
                .create_worktree(&task_id, base_branch)
                .await
            {
                Ok(info) => {
                    // Update task's cwd to worktree path
                    task.set_cwd(info.worktree_path.clone());
                    // Update metadata with worktree info
                    metadata.set_worktree_info(
                        info.worktree_path,
                        info.branch_name,
                        info.base_branch,
                    );
                }
                Err(e) => {
                    anyhow::bail!("Failed to create worktree: {e}");
                }
            }
        }

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
            tasks.insert(
                task_id.clone(),
                RunningTask {
                    metadata,
                    cancellation_token: token,
                },
            );
        }

        // Spawn background monitoring task to clean up registry when task completes
        let tasks_ref = Arc::clone(&self.tasks);
        let task_id_clone = task_id.clone();
        tokio::spawn(async move {
            // Wait for task completion (handle keeps task alive via AbortOnDropHandle)
            let _result = handle.await;

            // Remove from running tasks registry
            let mut tasks = tasks_ref.write().await;
            tasks.remove(&task_id_clone);

            info!(task_id = %task_id_clone, "Task removed from registry after completion");
        });

        info!(
            task_id = %task_id,
            use_worktree = %use_worktree,
            "Spawn task started"
        );

        Ok(task_id)
    }

    /// Kill a running task by cancelling its token.
    ///
    /// This implementation cancels the task's `CancellationToken` but does not
    /// wait for task completion. The background monitoring task will automatically
    /// clean up the registry when the task finishes. This is a deliberate design
    /// choice to avoid blocking the caller.
    ///
    /// The task will gracefully stop at its next cancellation check point,
    /// typically between loop iterations in SpawnAgent.
    pub async fn kill(&self, task_id: &str) -> anyhow::Result<()> {
        let task = {
            let mut tasks = self.tasks.write().await;
            tasks.remove(task_id)
        };

        match task {
            Some(task) => {
                info!(task_id = %task_id, "Killing spawn task");

                // Cancel the token - task will gracefully stop at next cancellation check
                task.cancellation_token.cancel();

                // Update metadata to cancelled
                let mut metadata = task.metadata;
                metadata.mark_cancelled(metadata.iterations_completed, metadata.iterations_failed);
                if let Err(e) = save_metadata(&self.codex_home, &metadata).await {
                    warn!(task_id = %task_id, error = %e, "Failed to save cancelled metadata");
                }

                Ok(())
            }
            None => {
                if load_metadata(&self.codex_home, task_id).await.is_ok() {
                    anyhow::bail!(
                        "Task '{}' is not running (use /spawn --drop to remove metadata)",
                        task_id
                    )
                } else {
                    anyhow::bail!("Task '{}' not found", task_id)
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
                anyhow::bail!(
                    "Task '{}' is still running. Use /spawn --kill first.",
                    task_id
                );
            }
        }

        // Load metadata to check for worktree
        if let Ok(metadata) = load_metadata(&self.codex_home, task_id).await {
            // WORKTREE: Framework-level cleanup for ALL task types
            if let Some(ref worktree_path) = metadata.worktree_path {
                // Shutdown LSP servers for this worktree BEFORE removing directory
                if let Some(ref lsp_manager) = self.lsp_manager {
                    info!(task_id = %task_id, "Shutting down LSP servers for worktree");
                    lsp_manager.shutdown_for_root(worktree_path).await;
                }

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

    /// Check if a task is running.
    pub async fn is_running(&self, task_id: &str) -> bool {
        let tasks = self.tasks.read().await;
        tasks.contains_key(task_id)
    }

    /// Validate task ID format.
    fn is_valid_task_id(task_id: &str) -> bool {
        if task_id.is_empty() || task_id.len() > 64 {
            return false;
        }
        task_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_task_id() {
        assert!(SpawnTaskManager::is_valid_task_id("my-task"));
        assert!(SpawnTaskManager::is_valid_task_id("task_123"));
        assert!(SpawnTaskManager::is_valid_task_id("abc"));
        assert!(SpawnTaskManager::is_valid_task_id("a-b-c_1_2_3"));

        // Invalid
        assert!(!SpawnTaskManager::is_valid_task_id(""));
        assert!(!SpawnTaskManager::is_valid_task_id("My Task")); // Uppercase and space
        assert!(!SpawnTaskManager::is_valid_task_id("task.name")); // Dot not allowed
        assert!(!SpawnTaskManager::is_valid_task_id("task@name")); // @ not allowed
    }
}
