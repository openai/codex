//! Background task management for long-running shell commands.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, Notify};

/// A background process tracked by the registry.
#[derive(Debug, Clone)]
pub struct BackgroundProcess {
    /// Unique identifier for this background task.
    pub id: String,
    /// The command being executed.
    pub command: String,
    /// Accumulated output (stdout + stderr interleaved).
    pub output: Arc<Mutex<String>>,
    /// Notification sent when the process completes.
    pub completed: Arc<Notify>,
}

/// Registry for tracking background shell processes.
#[derive(Debug, Clone)]
pub struct BackgroundTaskRegistry {
    tasks: Arc<Mutex<HashMap<String, BackgroundProcess>>>,
}

impl BackgroundTaskRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Registers a background process with the given task ID.
    pub async fn register(&self, task_id: String, process: BackgroundProcess) {
        let mut tasks = self.tasks.lock().await;
        tasks.insert(task_id, process);
    }

    /// Returns the accumulated output for the given task, if it exists.
    pub async fn get_output(&self, task_id: &str) -> Option<String> {
        let tasks = self.tasks.lock().await;
        let process = tasks.get(task_id)?;
        let output = process.output.lock().await;
        Some(output.clone())
    }

    /// Signals the task to stop and removes it from the registry.
    ///
    /// Returns true if the task was found and removed, false otherwise.
    pub async fn stop(&self, task_id: &str) -> bool {
        let mut tasks = self.tasks.lock().await;
        if let Some(process) = tasks.remove(task_id) {
            // Notify any waiters that the process is complete
            process.completed.notify_waiters();
            true
        } else {
            false
        }
    }

    /// Returns true if the task is still registered (potentially running).
    pub async fn is_running(&self, task_id: &str) -> bool {
        let tasks = self.tasks.lock().await;
        tasks.contains_key(task_id)
    }
}

impl Default for BackgroundTaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_process(id: &str, command: &str) -> BackgroundProcess {
        BackgroundProcess {
            id: id.to_string(),
            command: command.to_string(),
            output: Arc::new(Mutex::new(String::new())),
            completed: Arc::new(Notify::new()),
        }
    }

    #[tokio::test]
    async fn test_register_and_is_running() {
        let registry = BackgroundTaskRegistry::new();
        let process = make_process("task-1", "sleep 10");

        assert!(!registry.is_running("task-1").await);
        registry.register("task-1".to_string(), process).await;
        assert!(registry.is_running("task-1").await);
    }

    #[tokio::test]
    async fn test_get_output_empty() {
        let registry = BackgroundTaskRegistry::new();
        let process = make_process("task-2", "echo hello");

        registry.register("task-2".to_string(), process).await;
        let output = registry.get_output("task-2").await;
        assert_eq!(output, Some(String::new()));
    }

    #[tokio::test]
    async fn test_get_output_with_data() {
        let registry = BackgroundTaskRegistry::new();
        let process = make_process("task-3", "echo hello");
        let output_ref = Arc::clone(&process.output);

        registry.register("task-3".to_string(), process).await;

        // Simulate writing output
        {
            let mut out = output_ref.lock().await;
            out.push_str("hello world\n");
        }

        let output = registry.get_output("task-3").await;
        assert_eq!(output, Some("hello world\n".to_string()));
    }

    #[tokio::test]
    async fn test_stop_existing_task() {
        let registry = BackgroundTaskRegistry::new();
        let process = make_process("task-4", "sleep 60");

        registry.register("task-4".to_string(), process).await;
        assert!(registry.is_running("task-4").await);

        let stopped = registry.stop("task-4").await;
        assert!(stopped);
        assert!(!registry.is_running("task-4").await);
    }

    #[tokio::test]
    async fn test_stop_nonexistent_task() {
        let registry = BackgroundTaskRegistry::new();
        let stopped = registry.stop("no-such-task").await;
        assert!(!stopped);
    }

    #[tokio::test]
    async fn test_get_output_nonexistent() {
        let registry = BackgroundTaskRegistry::new();
        assert!(registry.get_output("missing").await.is_none());
    }

    #[tokio::test]
    async fn test_default() {
        let registry = BackgroundTaskRegistry::default();
        assert!(!registry.is_running("anything").await);
    }
}
