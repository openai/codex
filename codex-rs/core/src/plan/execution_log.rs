//! Plan execution logging and rollback
//!
//! Provides execution log persistence and rollback capabilities.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// File change record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// File path
    pub path: String,

    /// Change type
    pub change_type: ChangeType,

    /// Content before change (for rollback)
    pub before_content: Option<String>,

    /// Content after change
    pub after_content: Option<String>,

    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Type of file change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Created,
    Modified,
    Deleted,
}

/// Execution log for rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLog {
    /// Execution ID
    pub execution_id: String,

    /// Plan ID
    pub plan_id: String,

    /// File changes
    pub file_changes: Vec<FileChange>,

    /// Git commit hash before execution
    pub git_commit_before: Option<String>,

    /// Git commit hash after execution
    pub git_commit_after: Option<String>,

    /// Start time
    pub started_at: DateTime<Utc>,

    /// End time
    pub completed_at: Option<DateTime<Utc>>,

    /// Success flag
    pub success: bool,

    /// Error message (if failed)
    pub error: Option<String>,

    /// Rollback status
    pub rolled_back: bool,

    /// Rollback timestamp
    pub rolled_back_at: Option<DateTime<Utc>>,
}

impl ExecutionLog {
    /// Create a new execution log
    pub fn new(execution_id: String, plan_id: String) -> Self {
        Self {
            execution_id,
            plan_id,
            file_changes: vec![],
            git_commit_before: None,
            git_commit_after: None,
            started_at: Utc::now(),
            completed_at: None,
            success: false,
            error: None,
            rolled_back: false,
            rolled_back_at: None,
        }
    }

    /// Add file change
    pub fn add_file_change(&mut self, change: FileChange) {
        self.file_changes.push(change);
    }

    /// Mark as completed
    pub fn complete(&mut self, success: bool, error: Option<String>) {
        self.completed_at = Some(Utc::now());
        self.success = success;
        self.error = error;
    }

    /// Mark as rolled back
    pub fn mark_rolled_back(&mut self) {
        self.rolled_back = true;
        self.rolled_back_at = Some(Utc::now());
    }
}

/// Execution log manager
pub struct ExecutionLogManager {
    /// Log directory
    log_dir: PathBuf,
}

impl ExecutionLogManager {
    /// Create a new execution log manager
    pub fn new(log_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&log_dir).context("Failed to create execution log directory")?;

        Ok(Self { log_dir })
    }

    /// Save execution log
    pub fn save(&self, log: &ExecutionLog) -> Result<()> {
        let log_file = self.log_dir.join(format!("{}.json", log.execution_id));

        let json =
            serde_json::to_string_pretty(log).context("Failed to serialize execution log")?;

        std::fs::write(&log_file, json).context("Failed to write execution log")?;

        info!("Saved execution log: {}", log_file.display());

        Ok(())
    }

    /// Load execution log
    pub fn load(&self, execution_id: &str) -> Result<ExecutionLog> {
        let log_file = self.log_dir.join(format!("{}.json", execution_id));

        if !log_file.exists() {
            anyhow::bail!("Execution log not found: {}", execution_id);
        }

        let json = std::fs::read_to_string(&log_file).context("Failed to read execution log")?;

        let log: ExecutionLog =
            serde_json::from_str(&json).context("Failed to deserialize execution log")?;

        Ok(log)
    }

    /// List all execution logs
    pub fn list(&self) -> Result<Vec<ExecutionLog>> {
        let mut logs = Vec::new();

        if !self.log_dir.exists() {
            return Ok(logs);
        }

        for entry in std::fs::read_dir(&self.log_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(json) = std::fs::read_to_string(&path) {
                    if let Ok(log) = serde_json::from_str::<ExecutionLog>(&json) {
                        logs.push(log);
                    }
                }
            }
        }

        // Sort by start time (newest first)
        logs.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        Ok(logs)
    }

    /// Rollback an execution
    pub fn rollback(&self, execution_id: &str) -> Result<()> {
        let mut log = self.load(execution_id)?;

        if log.rolled_back {
            anyhow::bail!("Execution {} has already been rolled back", execution_id);
        }

        info!("Rolling back execution: {}", execution_id);

        // Rollback file changes in reverse order
        for change in log.file_changes.iter().rev() {
            match self.rollback_file_change(change) {
                Ok(_) => debug!("Rolled back: {}", change.path),
                Err(e) => warn!("Failed to rollback {}: {}", change.path, e),
            }
        }

        // Mark as rolled back
        log.mark_rolled_back();
        self.save(&log)?;

        info!("Rollback complete for execution: {}", execution_id);

        Ok(())
    }

    /// Rollback a single file change
    fn rollback_file_change(&self, change: &FileChange) -> Result<()> {
        match change.change_type {
            ChangeType::Created => {
                // Delete the file
                if std::path::Path::new(&change.path).exists() {
                    std::fs::remove_file(&change.path).context("Failed to delete created file")?;
                }
            }
            ChangeType::Modified => {
                // Restore original content
                if let Some(before_content) = &change.before_content {
                    std::fs::write(&change.path, before_content)
                        .context("Failed to restore modified file")?;
                }
            }
            ChangeType::Deleted => {
                // Restore deleted file
                if let Some(before_content) = &change.before_content {
                    std::fs::write(&change.path, before_content)
                        .context("Failed to restore deleted file")?;
                }
            }
        }

        Ok(())
    }

    /// Get execution statistics
    pub fn get_statistics(&self) -> Result<ExecutionStatistics> {
        let logs = self.list()?;

        let total_executions = logs.len();
        let successful_executions = logs.iter().filter(|l| l.success).count();
        let failed_executions = logs
            .iter()
            .filter(|l| !l.success && l.completed_at.is_some())
            .count();
        let rolled_back_executions = logs.iter().filter(|l| l.rolled_back).count();

        let avg_duration_secs = logs
            .iter()
            .filter_map(|l| {
                l.completed_at
                    .map(|completed| (completed - l.started_at).num_seconds() as f64)
            })
            .sum::<f64>()
            / total_executions.max(1) as f64;

        Ok(ExecutionStatistics {
            total_executions,
            successful_executions,
            failed_executions,
            rolled_back_executions,
            avg_duration_secs,
        })
    }
}

/// Execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStatistics {
    pub total_executions: usize,
    pub successful_executions: usize,
    pub failed_executions: usize,
    pub rolled_back_executions: usize,
    pub avg_duration_secs: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_log_creation() {
        let log = ExecutionLog::new("test-exec-1".to_string(), "bp-1".to_string());
        assert_eq!(log.execution_id, "test-exec-1");
        assert_eq!(log.plan_id, "bp-1");
        assert!(!log.rolled_back);
    }

    #[test]
    fn test_file_change_addition() {
        let mut log = ExecutionLog::new("test-exec-1".to_string(), "bp-1".to_string());

        log.add_file_change(FileChange {
            path: "test.rs".to_string(),
            change_type: ChangeType::Created,
            before_content: None,
            after_content: Some("fn main() {}".to_string()),
            timestamp: Utc::now(),
        });

        assert_eq!(log.file_changes.len(), 1);
    }
}
