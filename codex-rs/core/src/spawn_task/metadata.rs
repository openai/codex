use std::path::Path;
use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use tokio::fs;

use super::SpawnTaskStatus;
use super::SpawnTaskType;
use crate::loop_driver::LoopCondition;

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
    /// Model override used for this task (e.g., "deepseek" or "deepseek/v3").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_override: Option<String>,

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
    pub fn set_worktree_info(
        &mut self,
        worktree_path: PathBuf,
        branch_name: String,
        base_branch: String,
    ) {
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

/// Get log file path for a task.
pub fn log_file_path(codex_home: &Path, task_id: &str) -> PathBuf {
    tasks_dir(codex_home)
        .join("logs")
        .join(format!("{task_id}.log"))
}

/// Save spawn task metadata to disk.
pub async fn save_metadata(codex_home: &Path, metadata: &SpawnTaskMetadata) -> anyhow::Result<()> {
    let dir = tasks_dir(codex_home);
    fs::create_dir_all(&dir).await?;

    let path = metadata_path(codex_home, &metadata.task_id);
    let content = serde_json::to_string_pretty(metadata)?;
    fs::write(&path, content).await?;

    Ok(())
}

/// Load spawn task metadata from disk.
pub async fn load_metadata(codex_home: &Path, task_id: &str) -> anyhow::Result<SpawnTaskMetadata> {
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

    let mut result: Vec<SpawnTaskMetadata> = Vec::new();
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
pub async fn delete_metadata(codex_home: &Path, task_id: &str) -> anyhow::Result<()> {
    let path = metadata_path(codex_home, task_id);
    fs::remove_file(&path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_metadata(task_id: &str) -> SpawnTaskMetadata {
        SpawnTaskMetadata {
            task_id: task_id.to_string(),
            task_type: SpawnTaskType::Agent,
            status: SpawnTaskStatus::Running,
            created_at: Utc::now(),
            completed_at: None,
            cwd: PathBuf::from("/test"),
            error_message: None,
            loop_condition: Some(LoopCondition::Iters { count: 5 }),
            user_query: Some("test query".to_string()),
            iterations_completed: 0,
            iterations_failed: 0,
            model_override: None,
            workflow_path: None,
            worktree_path: None,
            branch_name: None,
            base_branch: None,
            log_file: None,
            execution_result: None,
        }
    }

    #[tokio::test]
    async fn test_save_and_load_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let codex_home = temp_dir.path();

        let metadata = create_test_metadata("test-task-1");
        save_metadata(codex_home, &metadata).await.unwrap();

        let loaded = load_metadata(codex_home, "test-task-1").await.unwrap();
        assert_eq!(loaded.task_id, "test-task-1");
        assert_eq!(loaded.task_type, SpawnTaskType::Agent);
        assert_eq!(loaded.status, SpawnTaskStatus::Running);
    }

    #[tokio::test]
    async fn test_list_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let codex_home = temp_dir.path();

        save_metadata(codex_home, &create_test_metadata("task-1"))
            .await
            .unwrap();
        save_metadata(codex_home, &create_test_metadata("task-2"))
            .await
            .unwrap();

        let list = list_metadata(codex_home).await.unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let codex_home = temp_dir.path();

        save_metadata(codex_home, &create_test_metadata("task-to-delete"))
            .await
            .unwrap();
        assert!(load_metadata(codex_home, "task-to-delete").await.is_ok());

        delete_metadata(codex_home, "task-to-delete").await.unwrap();
        assert!(load_metadata(codex_home, "task-to-delete").await.is_err());
    }

    #[test]
    fn test_mark_completed() {
        let mut metadata = create_test_metadata("test");
        metadata.mark_completed(5, 1);

        assert_eq!(metadata.status, SpawnTaskStatus::Completed);
        assert!(metadata.completed_at.is_some());
        assert_eq!(metadata.iterations_completed, 5);
        assert_eq!(metadata.iterations_failed, 1);
    }

    #[test]
    fn test_mark_failed() {
        let mut metadata = create_test_metadata("test");
        metadata.mark_failed(3, 2, "test error".to_string());

        assert_eq!(metadata.status, SpawnTaskStatus::Failed);
        assert!(metadata.completed_at.is_some());
        assert_eq!(metadata.error_message, Some("test error".to_string()));
    }
}
