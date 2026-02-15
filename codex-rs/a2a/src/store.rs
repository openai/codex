//! Task storage trait and in-memory implementation.
//!
//! Mirrors `a2a-js/src/server/store.ts`.

use crate::types::Task;

/// Storage provider for tasks.
///
/// Implement this trait to use a custom backend (database, Redis, etc.).
pub trait TaskStore: Send + Sync + 'static {
    /// Save (upsert) a task.
    fn save(
        &self,
        task: Task,
    ) -> impl std::future::Future<Output = Result<(), crate::error::A2AError>> + Send;

    /// Load a task by ID. Returns `None` if not found.
    fn load(
        &self,
        task_id: &str,
    ) -> impl std::future::Future<Output = Result<Option<Task>, crate::error::A2AError>> + Send;
}

/// In-memory task store (default).
pub struct InMemoryTaskStore {
    store: tokio::sync::Mutex<std::collections::HashMap<String, Task>>,
}

impl InMemoryTaskStore {
    pub fn new() -> Self {
        Self {
            store: tokio::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for InMemoryTaskStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskStore for InMemoryTaskStore {
    async fn save(&self, task: Task) -> Result<(), crate::error::A2AError> {
        self.store.lock().await.insert(task.id.clone(), task);
        Ok(())
    }

    async fn load(&self, task_id: &str) -> Result<Option<Task>, crate::error::A2AError> {
        Ok(self.store.lock().await.get(task_id).cloned())
    }
}
