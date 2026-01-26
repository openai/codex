//! Batch completion tracker for SessionStart events.
//!
//! Tracks the progress and completion of batched file processing operations,
//! supporting async notification when all events in a batch are complete.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use std::time::Instant;

use tokio::sync::RwLock;
use tokio::sync::oneshot;
use uuid::Uuid;

/// Unique identifier for a batch of events.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BatchId(String);

impl BatchId {
    /// Create a new unique batch ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create a batch ID from a string (for testing).
    pub fn from_str(s: &str) -> Self {
        Self(s.to_string())
    }

    /// Get the string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for BatchId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for BatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Result of a completed batch.
#[derive(Debug, Clone)]
pub struct BatchResult {
    /// The batch ID.
    pub batch_id: BatchId,
    /// Number of successfully completed events.
    pub completed: i64,
    /// Number of failed events.
    pub failed: i64,
    /// Total duration in milliseconds.
    pub duration_ms: i64,
}

/// Internal state for a tracked batch.
struct BatchState {
    /// Total number of events in this batch.
    total: i64,
    /// Number of completed events (success + failed).
    processed: AtomicI64,
    /// Number of failed events.
    failed: AtomicI64,
    /// When the batch was started.
    started_at: Instant,
    /// Channel to send completion notification.
    complete_tx: Option<oneshot::Sender<BatchResult>>,
}

/// Batch completion tracker.
///
/// Tracks multiple batches concurrently and notifies when each batch completes.
pub struct BatchTracker {
    /// Active batches being tracked.
    batches: RwLock<HashMap<BatchId, BatchState>>,
}

impl BatchTracker {
    /// Create a new batch tracker.
    pub fn new() -> Self {
        Self {
            batches: RwLock::new(HashMap::new()),
        }
    }

    /// Start tracking a new batch.
    ///
    /// Returns a receiver that will receive the batch result when all events
    /// in the batch are complete (either success or failure).
    ///
    /// # Arguments
    /// * `batch_id` - Unique identifier for this batch
    /// * `total` - Total number of events in the batch
    pub async fn start_batch(
        &self,
        batch_id: BatchId,
        total: i64,
    ) -> oneshot::Receiver<BatchResult> {
        let (tx, rx) = oneshot::channel();

        // Handle empty batch immediately
        if total <= 0 {
            tracing::info!(
                batch_id = %batch_id,
                "Batch started with 0 items, completing immediately"
            );
            let result = BatchResult {
                batch_id: batch_id.clone(),
                completed: 0,
                failed: 0,
                duration_ms: 0,
            };
            let _ = tx.send(result);
            return rx;
        }

        let state = BatchState {
            total,
            processed: AtomicI64::new(0),
            failed: AtomicI64::new(0),
            started_at: Instant::now(),
            complete_tx: Some(tx),
        };

        tracing::info!(
            batch_id = %batch_id,
            total = total,
            "Batch started"
        );

        self.batches.write().await.insert(batch_id, state);
        rx
    }

    /// Mark an event in a batch as complete.
    ///
    /// # Arguments
    /// * `batch_id` - The batch this event belongs to
    /// * `success` - Whether the event completed successfully
    ///
    /// When the last event in a batch is marked complete, the batch result
    /// is sent to the receiver returned by `start_batch`.
    pub async fn mark_complete(&self, batch_id: &BatchId, success: bool) {
        let mut batches = self.batches.write().await;

        if let Some(state) = batches.get_mut(batch_id) {
            // Increment counters
            let processed = state.processed.fetch_add(1, Ordering::SeqCst) + 1;
            if !success {
                state.failed.fetch_add(1, Ordering::SeqCst);
            }

            tracing::debug!(
                batch_id = %batch_id,
                processed = processed,
                total = state.total,
                success = success,
                "Event completed in batch"
            );

            // Check if batch is complete
            if processed >= state.total {
                let failed = state.failed.load(Ordering::Acquire);
                let completed = processed - failed;
                let duration_ms = state.started_at.elapsed().as_millis() as i64;

                let result = BatchResult {
                    batch_id: batch_id.clone(),
                    completed,
                    failed,
                    duration_ms,
                };

                tracing::info!(
                    batch_id = %batch_id,
                    completed = completed,
                    failed = failed,
                    duration_ms = duration_ms,
                    "Batch complete"
                );

                // Send completion notification
                if let Some(tx) = state.complete_tx.take() {
                    let _ = tx.send(result);
                }

                // Remove the batch
                batches.remove(batch_id);
            }
        } else {
            tracing::warn!(
                batch_id = %batch_id,
                "Attempted to mark complete for unknown batch"
            );
        }
    }

    /// Get the current progress of a batch.
    ///
    /// Returns `(completed, failed, total)` or `None` if the batch doesn't exist.
    pub async fn progress(&self, batch_id: &BatchId) -> Option<(i64, i64, i64)> {
        let batches = self.batches.read().await;
        batches.get(batch_id).map(|state| {
            let processed = state.processed.load(Ordering::Acquire);
            let failed = state.failed.load(Ordering::Acquire);
            let completed = processed - failed;
            (completed, failed, state.total)
        })
    }

    /// Check if a batch is complete.
    pub async fn is_complete(&self, batch_id: &BatchId) -> bool {
        let batches = self.batches.read().await;
        !batches.contains_key(batch_id)
    }

    /// Get the number of active batches.
    pub async fn active_batch_count(&self) -> usize {
        self.batches.read().await.len()
    }

    /// Cancel a batch (remove without sending completion).
    pub async fn cancel_batch(&self, batch_id: &BatchId) {
        let mut batches = self.batches.write().await;
        if batches.remove(batch_id).is_some() {
            tracing::info!(batch_id = %batch_id, "Batch cancelled");
        }
    }
}

impl Default for BatchTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared batch tracker wrapped in Arc for use across threads.
pub type SharedBatchTracker = Arc<BatchTracker>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_batch_completion() {
        let tracker = BatchTracker::new();
        let batch_id = BatchId::from_str("test-batch-1");

        let rx = tracker.start_batch(batch_id.clone(), 3).await;

        // Mark events as complete
        tracker.mark_complete(&batch_id, true).await;
        tracker.mark_complete(&batch_id, true).await;
        tracker.mark_complete(&batch_id, true).await;

        // Should receive result
        let result = rx.await.unwrap();
        assert_eq!(result.completed, 3);
        assert_eq!(result.failed, 0);
    }

    #[tokio::test]
    async fn test_batch_with_failures() {
        let tracker = BatchTracker::new();
        let batch_id = BatchId::from_str("test-batch-2");

        let rx = tracker.start_batch(batch_id.clone(), 3).await;

        // Mark events with mixed results
        tracker.mark_complete(&batch_id, true).await;
        tracker.mark_complete(&batch_id, false).await;
        tracker.mark_complete(&batch_id, true).await;

        let result = rx.await.unwrap();
        assert_eq!(result.completed, 2);
        assert_eq!(result.failed, 1);
    }

    #[tokio::test]
    async fn test_progress_tracking() {
        let tracker = BatchTracker::new();
        let batch_id = BatchId::from_str("test-batch-3");

        let _rx = tracker.start_batch(batch_id.clone(), 5).await;

        // Check initial progress
        let (completed, failed, total) = tracker.progress(&batch_id).await.unwrap();
        assert_eq!(completed, 0);
        assert_eq!(failed, 0);
        assert_eq!(total, 5);

        // Mark some as complete
        tracker.mark_complete(&batch_id, true).await;
        tracker.mark_complete(&batch_id, false).await;

        let (completed, failed, total) = tracker.progress(&batch_id).await.unwrap();
        assert_eq!(completed, 1);
        assert_eq!(failed, 1);
        assert_eq!(total, 5);
    }

    #[tokio::test]
    async fn test_multiple_batches() {
        let tracker = BatchTracker::new();
        let batch1 = BatchId::from_str("batch-1");
        let batch2 = BatchId::from_str("batch-2");

        let rx1 = tracker.start_batch(batch1.clone(), 2).await;
        let rx2 = tracker.start_batch(batch2.clone(), 2).await;

        assert_eq!(tracker.active_batch_count().await, 2);

        // Complete batch1
        tracker.mark_complete(&batch1, true).await;
        tracker.mark_complete(&batch1, true).await;

        let result1 = rx1.await.unwrap();
        assert_eq!(result1.completed, 2);
        assert_eq!(tracker.active_batch_count().await, 1);

        // Complete batch2
        tracker.mark_complete(&batch2, true).await;
        tracker.mark_complete(&batch2, true).await;

        let result2 = rx2.await.unwrap();
        assert_eq!(result2.completed, 2);
        assert_eq!(tracker.active_batch_count().await, 0);
    }

    #[tokio::test]
    async fn test_cancel_batch() {
        let tracker = BatchTracker::new();
        let batch_id = BatchId::from_str("test-batch-cancel");

        let rx = tracker.start_batch(batch_id.clone(), 3).await;

        tracker.mark_complete(&batch_id, true).await;
        tracker.cancel_batch(&batch_id).await;

        // Receiver should get an error (sender dropped)
        assert!(rx.await.is_err());
        assert!(tracker.is_complete(&batch_id).await);
    }
}
