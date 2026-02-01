//! Async hook tracking.
//!
//! Tracks background hook tasks and their completion status. When hooks
//! return `{ "async": true }`, they are registered here and their results
//! are collected for delivery via system reminders.
//!
//! ## Usage
//!
//! 1. When a hook returns `HookResult::Async`, register it with [`AsyncHookTracker::register`]
//! 2. When the background task completes, call [`AsyncHookTracker::complete`]
//! 3. Periodically call [`AsyncHookTracker::take_completed`] to get finished hooks

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Instant;

use serde::Deserialize;
use serde::Serialize;

use crate::result::HookResult;

/// Tracks pending and completed async hooks.
#[derive(Default)]
pub struct AsyncHookTracker {
    /// Pending async hooks indexed by task_id.
    pending: RwLock<HashMap<String, PendingAsyncHook>>,
    /// Completed async hooks ready for delivery.
    completed: RwLock<Vec<CompletedAsyncHook>>,
}

/// A pending async hook task.
#[derive(Debug, Clone)]
pub struct PendingAsyncHook {
    /// Unique task identifier.
    pub task_id: String,
    /// Name of the hook.
    pub hook_name: String,
    /// When the async task started.
    pub started_at: Instant,
}

/// A completed async hook with its result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedAsyncHook {
    /// Unique task identifier.
    pub task_id: String,
    /// Name of the hook.
    pub hook_name: String,
    /// Execution duration in milliseconds.
    pub duration_ms: i64,
    /// The result of the hook.
    pub result: HookResult,
    /// Additional context from the hook.
    pub additional_context: Option<String>,
    /// Whether the hook blocked execution (only possible for pre-hooks).
    pub was_blocking: bool,
    /// Reason for blocking (if was_blocking is true).
    pub blocking_reason: Option<String>,
}

impl AsyncHookTracker {
    /// Creates a new empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new async hook task.
    pub fn register(&self, task_id: String, hook_name: String) {
        if let Ok(mut pending) = self.pending.write() {
            pending.insert(
                task_id.clone(),
                PendingAsyncHook {
                    task_id,
                    hook_name,
                    started_at: Instant::now(),
                },
            );
        }
    }

    /// Marks an async hook as completed with its result.
    pub fn complete(&self, task_id: &str, result: HookResult) {
        // Get and remove the pending hook
        let pending_hook = if let Ok(mut pending) = self.pending.write() {
            pending.remove(task_id)
        } else {
            return;
        };

        let Some(pending) = pending_hook else {
            tracing::warn!(task_id, "Completed unknown async hook task");
            return;
        };

        let duration_ms = pending.started_at.elapsed().as_millis() as i64;

        // Extract blocking info and additional context from result
        let (was_blocking, blocking_reason, additional_context) = match &result {
            HookResult::Reject { reason } => (true, Some(reason.clone()), None),
            HookResult::ContinueWithContext {
                additional_context: ctx,
            } => (false, None, ctx.clone()),
            _ => (false, None, None),
        };

        let completed = CompletedAsyncHook {
            task_id: pending.task_id,
            hook_name: pending.hook_name,
            duration_ms,
            result,
            additional_context,
            was_blocking,
            blocking_reason,
        };

        if let Ok(mut completed_list) = self.completed.write() {
            completed_list.push(completed);
        }
    }

    /// Takes all completed hooks, clearing the completed list.
    ///
    /// Returns the completed hooks for processing (e.g., generating system reminders).
    pub fn take_completed(&self) -> Vec<CompletedAsyncHook> {
        if let Ok(mut completed) = self.completed.write() {
            std::mem::take(&mut *completed)
        } else {
            Vec::new()
        }
    }

    /// Returns the number of pending async hooks.
    pub fn pending_count(&self) -> i32 {
        self.pending.read().map(|p| p.len() as i32).unwrap_or(0)
    }

    /// Returns the number of completed but not yet processed hooks.
    pub fn completed_count(&self) -> i32 {
        self.completed.read().map(|c| c.len() as i32).unwrap_or(0)
    }

    /// Checks if there are any pending or completed hooks.
    pub fn is_empty(&self) -> bool {
        self.pending_count() == 0 && self.completed_count() == 0
    }

    /// Cancels a pending async hook.
    ///
    /// This removes the hook from pending without adding it to completed.
    /// Useful when a hook times out or is cancelled.
    pub fn cancel(&self, task_id: &str) -> bool {
        if let Ok(mut pending) = self.pending.write() {
            pending.remove(task_id).is_some()
        } else {
            false
        }
    }
}

impl std::fmt::Debug for AsyncHookTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncHookTracker")
            .field("pending_count", &self.pending_count())
            .field("completed_count", &self.completed_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_complete() {
        let tracker = AsyncHookTracker::new();

        tracker.register("task-1".to_string(), "test-hook".to_string());
        assert_eq!(tracker.pending_count(), 1);
        assert_eq!(tracker.completed_count(), 0);

        tracker.complete("task-1", HookResult::Continue);
        assert_eq!(tracker.pending_count(), 0);
        assert_eq!(tracker.completed_count(), 1);

        let completed = tracker.take_completed();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].task_id, "task-1");
        assert_eq!(completed[0].hook_name, "test-hook");
        assert!(!completed[0].was_blocking);

        assert_eq!(tracker.completed_count(), 0);
    }

    #[test]
    fn test_complete_with_reject() {
        let tracker = AsyncHookTracker::new();

        tracker.register("task-1".to_string(), "security-hook".to_string());
        tracker.complete(
            "task-1",
            HookResult::Reject {
                reason: "Not allowed".to_string(),
            },
        );

        let completed = tracker.take_completed();
        assert_eq!(completed.len(), 1);
        assert!(completed[0].was_blocking);
        assert_eq!(
            completed[0].blocking_reason,
            Some("Not allowed".to_string())
        );
    }

    #[test]
    fn test_complete_with_context() {
        let tracker = AsyncHookTracker::new();

        tracker.register("task-1".to_string(), "context-hook".to_string());
        tracker.complete(
            "task-1",
            HookResult::ContinueWithContext {
                additional_context: Some("Extra info".to_string()),
            },
        );

        let completed = tracker.take_completed();
        assert_eq!(completed.len(), 1);
        assert!(!completed[0].was_blocking);
        assert_eq!(
            completed[0].additional_context,
            Some("Extra info".to_string())
        );
    }

    #[test]
    fn test_complete_unknown_task() {
        let tracker = AsyncHookTracker::new();
        // Should not panic or add to completed
        tracker.complete("unknown-task", HookResult::Continue);
        assert_eq!(tracker.completed_count(), 0);
    }

    #[test]
    fn test_cancel() {
        let tracker = AsyncHookTracker::new();

        tracker.register("task-1".to_string(), "hook".to_string());
        assert_eq!(tracker.pending_count(), 1);

        let cancelled = tracker.cancel("task-1");
        assert!(cancelled);
        assert_eq!(tracker.pending_count(), 0);
        assert_eq!(tracker.completed_count(), 0);
    }

    #[test]
    fn test_cancel_unknown() {
        let tracker = AsyncHookTracker::new();
        let cancelled = tracker.cancel("unknown");
        assert!(!cancelled);
    }

    #[test]
    fn test_is_empty() {
        let tracker = AsyncHookTracker::new();
        assert!(tracker.is_empty());

        tracker.register("task-1".to_string(), "hook".to_string());
        assert!(!tracker.is_empty());

        tracker.complete("task-1", HookResult::Continue);
        assert!(!tracker.is_empty()); // Has completed

        tracker.take_completed();
        assert!(tracker.is_empty());
    }

    #[test]
    fn test_multiple_hooks() {
        let tracker = AsyncHookTracker::new();

        tracker.register("task-1".to_string(), "hook-1".to_string());
        tracker.register("task-2".to_string(), "hook-2".to_string());
        tracker.register("task-3".to_string(), "hook-3".to_string());

        assert_eq!(tracker.pending_count(), 3);

        tracker.complete("task-2", HookResult::Continue);
        tracker.complete("task-1", HookResult::Continue);

        assert_eq!(tracker.pending_count(), 1);
        assert_eq!(tracker.completed_count(), 2);

        let completed = tracker.take_completed();
        assert_eq!(completed.len(), 2);
    }
}
