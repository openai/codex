//! Background task management for async subagent execution.

use super::result::SubagentResult;
use crate::system_reminder::generator::BackgroundTaskInfo;
use crate::system_reminder::generator::BackgroundTaskStatus as ReminderTaskStatus;
use crate::system_reminder::generator::BackgroundTaskType;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::task::JoinHandle;

/// Status of a background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundTaskStatus {
    /// Task registered but handle not yet set (phase 1 of two-phase registration).
    /// Used to prevent race conditions between spawn and TaskOutput queries.
    Pending,
    Running,
    /// Handle has been taken and result is being awaited.
    /// Another caller should wait for Completed/Failed.
    AwaitingResult,
    Completed,
    Failed,
}

/// A background subagent task.
#[derive(Debug)]
pub struct BackgroundTask {
    /// Unique agent ID.
    pub agent_id: String,
    /// Short description of the task.
    pub description: String,
    /// The prompt/task being executed.
    pub prompt: String,
    /// Current status.
    pub status: BackgroundTaskStatus,
    /// Result when completed.
    pub result: Option<SubagentResult>,
    /// Handle to the running task.
    pub handle: Option<JoinHandle<SubagentResult>>,
    /// When the task was created.
    pub created_at: Instant,
    /// Whether completion has been notified via system reminder.
    pub notified: bool,
}

/// Store for managing background tasks.
#[derive(Debug, Default)]
pub struct BackgroundTaskStore {
    tasks: DashMap<String, BackgroundTask>,
}

impl BackgroundTaskStore {
    /// Create a new background task store.
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
        }
    }

    /// Register a new background task (single-phase, for backwards compatibility).
    pub fn register(
        &self,
        agent_id: String,
        description: String,
        prompt: String,
        handle: JoinHandle<SubagentResult>,
    ) {
        let task = BackgroundTask {
            agent_id: agent_id.clone(),
            description,
            prompt,
            status: BackgroundTaskStatus::Running,
            result: None,
            handle: Some(handle),
            created_at: Instant::now(),
            notified: false,
        };
        self.tasks.insert(agent_id, task);
    }

    /// Phase 1: Pre-register task with Pending status (before tokio::spawn).
    ///
    /// This prevents race conditions where TaskOutput is called before the
    /// handle is registered, ensuring the task is visible immediately.
    ///
    /// # Example
    /// ```ignore
    /// store.register_pending(agent_id.clone(), description, prompt);
    /// let handle = tokio::spawn(async move { ... });
    /// store.set_handle(&agent_id, handle);
    /// ```
    pub fn register_pending(&self, agent_id: String, description: String, prompt: String) {
        let task = BackgroundTask {
            agent_id: agent_id.clone(),
            description,
            prompt,
            status: BackgroundTaskStatus::Pending,
            result: None,
            handle: None,
            created_at: Instant::now(),
            notified: false,
        };
        self.tasks.insert(agent_id, task);
    }

    /// Phase 2: Set the handle and transition to Running status.
    ///
    /// Must be called after `register_pending()` once the task has been spawned.
    pub fn set_handle(&self, agent_id: &str, handle: JoinHandle<SubagentResult>) {
        if let Some(mut task) = self.tasks.get_mut(agent_id) {
            task.handle = Some(handle);
            task.status = BackgroundTaskStatus::Running;
        }
    }

    /// Get the status of a task.
    pub fn get_status(&self, agent_id: &str) -> Option<BackgroundTaskStatus> {
        self.tasks.get(agent_id).map(|t| t.status)
    }

    /// Get task result, optionally blocking until complete.
    pub async fn get_result(
        &self,
        agent_id: &str,
        block: bool,
        timeout: Duration,
    ) -> Option<SubagentResult> {
        // First check: see current status
        let handle_to_await = {
            let Some(mut task) = self.tasks.get_mut(agent_id) else {
                return None;
            };

            match task.status {
                BackgroundTaskStatus::Completed | BackgroundTaskStatus::Failed => {
                    // Already done, return cached result
                    return task.result.clone();
                }
                BackgroundTaskStatus::Pending | BackgroundTaskStatus::AwaitingResult => {
                    // Task is pending (handle not set yet) or another caller is awaiting
                    if block {
                        // We need to wait for them to finish
                        drop(task);
                        return self.wait_for_completion(agent_id, timeout).await;
                    }
                    return None;
                }
                BackgroundTaskStatus::Running => {
                    if !block {
                        return None;
                    }
                    // Mark as awaiting so other callers know we're processing
                    task.status = BackgroundTaskStatus::AwaitingResult;
                    task.handle.take()
                }
            }
        };

        // Now await the handle outside the lock
        let Some(handle) = handle_to_await else {
            // No handle but status was Running - shouldn't happen
            return None;
        };

        let result = tokio::select! {
            res = handle => {
                match res {
                    Ok(result) => Some(result),
                    Err(_) => None,
                }
            }
            _ = tokio::time::sleep(timeout) => {
                // Timeout - mark as failed
                if let Some(mut task) = self.tasks.get_mut(agent_id) {
                    task.status = BackgroundTaskStatus::Failed;
                }
                None
            }
        };

        // Update the task with the result
        if let Some(mut task) = self.tasks.get_mut(agent_id) {
            if let Some(ref r) = result {
                task.status = BackgroundTaskStatus::Completed;
                task.result = Some(r.clone());
            } else {
                task.status = BackgroundTaskStatus::Failed;
            }
        }

        result
    }

    /// Wait for a task that another caller is already awaiting.
    async fn wait_for_completion(
        &self,
        agent_id: &str,
        timeout: Duration,
    ) -> Option<SubagentResult> {
        let start = Instant::now();
        let poll_interval = Duration::from_millis(50);

        while start.elapsed() < timeout {
            if let Some(task) = self.tasks.get(agent_id) {
                match task.status {
                    BackgroundTaskStatus::Completed | BackgroundTaskStatus::Failed => {
                        return task.result.clone();
                    }
                    BackgroundTaskStatus::Pending
                    | BackgroundTaskStatus::AwaitingResult
                    | BackgroundTaskStatus::Running => {
                        // Still waiting, poll again
                    }
                }
            } else {
                return None;
            }
            tokio::time::sleep(poll_interval).await;
        }
        None
    }

    /// List all task IDs.
    pub fn list_task_ids(&self) -> Vec<String> {
        self.tasks.iter().map(|r| r.key().clone()).collect()
    }

    /// Remove completed tasks older than specified duration.
    pub fn cleanup_old_tasks(&self, older_than: Duration) {
        let now = Instant::now();
        self.tasks.retain(|_, task| {
            match task.status {
                // Keep pending and running tasks
                BackgroundTaskStatus::Pending | BackgroundTaskStatus::Running => true,
                // Remove old completed/failed/awaiting tasks
                _ => now.duration_since(task.created_at) < older_than,
            }
        });
    }

    /// List background agents that need notification.
    ///
    /// Returns agents that have finished (completed/failed) but not yet notified.
    /// Used by system reminder injection.
    pub fn list_for_reminder(&self) -> Vec<BackgroundTaskInfo> {
        self.tasks
            .iter()
            .filter(|r| {
                let task = r.value();
                // Only finished tasks that haven't been notified
                matches!(
                    task.status,
                    BackgroundTaskStatus::Completed | BackgroundTaskStatus::Failed
                ) && !task.notified
            })
            .map(|r| {
                let task = r.value();
                BackgroundTaskInfo {
                    task_id: r.key().clone(),
                    task_type: BackgroundTaskType::AsyncAgent,
                    command: None, // Agents don't have commands
                    description: task.description.clone(),
                    status: match task.status {
                        BackgroundTaskStatus::Completed => ReminderTaskStatus::Completed,
                        BackgroundTaskStatus::Failed => ReminderTaskStatus::Failed,
                        _ => ReminderTaskStatus::Running,
                    },
                    exit_code: None,
                    has_new_output: false, // Agents: no streaming output
                    notified: task.notified,
                }
            })
            .collect()
    }

    /// Mark an agent as notified (after reminder was injected).
    pub fn mark_notified(&self, agent_id: &str) {
        if let Some(mut task) = self.tasks.get_mut(agent_id) {
            task.notified = true;
        }
    }

    /// Batch mark multiple agents as notified.
    ///
    /// More efficient than calling mark_notified() in a loop when marking
    /// multiple agents, as it reduces lock contention.
    pub fn mark_all_notified(&self, agent_ids: &[String]) {
        for id in agent_ids {
            if let Some(mut task) = self.tasks.get_mut(id) {
                task.notified = true;
            }
        }
    }
}

// NOTE: BackgroundTaskStore intentionally does NOT implement Clone.
// JoinHandle cannot be cloned, and silently creating an empty store
// would cause data loss. Use Arc<BackgroundTaskStore> for sharing.

/// Wrap BackgroundTaskStore in Arc for sharing.
#[allow(dead_code)] // Type alias for shared ownership pattern
pub type SharedBackgroundTaskStore = Arc<BackgroundTaskStore>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subagent::result::SubagentStatus;

    #[tokio::test]
    async fn test_register_and_status() {
        let store = BackgroundTaskStore::new();

        let handle = tokio::spawn(async {
            SubagentResult {
                status: SubagentStatus::Goal,
                result: "done".to_string(),
                turns_used: 1,
                duration: Duration::from_secs(1),
                agent_id: "test-1".to_string(),
                total_tool_use_count: 0,
                total_duration_ms: 1000,
                total_tokens: 100,
                usage: None,
            }
        });

        store.register(
            "test-1".to_string(),
            "Test task".to_string(),
            "Do something".to_string(),
            handle,
        );

        let status = store.get_status("test-1");
        assert_eq!(status, Some(BackgroundTaskStatus::Running));
    }

    #[test]
    fn test_list_task_ids() {
        let store = BackgroundTaskStore::new();
        assert!(store.list_task_ids().is_empty());
    }
}
