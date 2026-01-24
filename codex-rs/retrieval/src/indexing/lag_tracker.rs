//! Lag tracking with watermark mechanism for out-of-order completion.
//!
//! Provides accurate lag detection when events are processed in parallel
//! and may complete out of order. Uses a watermark algorithm similar to
//! Kafka/Flink to track the highest contiguous completed sequence.
//!
//! # Watermark Algorithm
//!
//! - Each event is assigned a monotonically increasing sequence number
//! - Watermark = the highest seq where all events with seq <= watermark are complete
//! - Lag = next_seq - watermark - 1 = number of incomplete events
//!
//! # Example
//!
//! ```text
//! Assigned: [1, 2, 3, 4, 5]
//! Complete order: [3, 1, 5, 2, 4] (out of order)
//!
//! After 3 completes: watermark=0, pending={1,2,4,5}, lag=4
//! After 1 completes: watermark=1, pending={2,4,5}, lag=3
//! After 5 completes: watermark=1, pending={2,4}, lag=3
//! After 2 completes: watermark=3, pending={4}, lag=1  (jumps to 3!)
//! After 4 completes: watermark=5, pending={}, lag=0
//! ```

use std::collections::BTreeSet;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use tokio::sync::RwLock;
use tokio::sync::broadcast;

/// Detailed lag information for monitoring and debugging.
#[derive(Debug, Clone, Default)]
pub struct LagInfo {
    /// Total number of sequences assigned.
    pub total_assigned: i64,
    /// Current watermark (all seq <= watermark are complete).
    pub watermark: i64,
    /// Number of events currently being processed.
    pub pending_count: i64,
    /// Number of events that failed and were skipped.
    pub failed_count: i64,
    /// Current lag (events not yet complete).
    pub lag: i64,
}

/// Error when waiting for zero lag times out.
#[derive(Debug)]
pub struct LagTimeoutError {
    /// Current lag when timeout occurred.
    pub lag: i64,
    /// Detailed lag info at timeout.
    pub info: LagInfo,
}

impl std::fmt::Display for LagTimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Lag timeout: {} events pending (watermark={}, pending={}, failed={})",
            self.lag, self.info.watermark, self.info.pending_count, self.info.failed_count
        )
    }
}

impl std::error::Error for LagTimeoutError {}

/// Lag tracker with watermark mechanism.
///
/// Tracks event processing with correct handling of out-of-order completion.
/// Failed events are skipped (don't block watermark) but are counted separately.
pub struct LagTracker {
    /// Next sequence number to assign.
    next_seq: AtomicI64,

    /// Pending event sequence numbers (in processing).
    /// Uses BTreeSet for efficient min() lookup.
    pending: RwLock<BTreeSet<i64>>,

    /// Failed event sequence numbers (skipped).
    failed: RwLock<HashSet<i64>>,

    /// Watermark: all events with seq <= watermark are complete or failed.
    watermark: AtomicI64,

    /// Broadcast channel to notify when lag changes.
    lag_changed: broadcast::Sender<i64>,
}

impl LagTracker {
    /// Create a new lag tracker.
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(64);
        Self {
            next_seq: AtomicI64::new(1), // Start from 1, watermark=0 means nothing complete
            pending: RwLock::new(BTreeSet::new()),
            failed: RwLock::new(HashSet::new()),
            watermark: AtomicI64::new(0),
            lag_changed: tx,
        }
    }

    /// Assign a new sequence number to an event.
    ///
    /// This should be called when an event is created/queued.
    pub fn assign_seq(&self) -> i64 {
        self.next_seq.fetch_add(1, Ordering::SeqCst)
    }

    /// Mark an event as started (add to pending set).
    ///
    /// Call this when a worker starts processing an event.
    pub async fn start_event(&self, seq: i64) {
        let mut pending = self.pending.write().await;
        pending.insert(seq);

        tracing::trace!(seq = seq, "Event started, added to pending");
    }

    /// Mark an event as successfully completed.
    ///
    /// Removes from pending and updates watermark if possible.
    pub async fn complete_event(&self, seq: i64) {
        let mut pending = self.pending.write().await;
        pending.remove(&seq);

        tracing::trace!(seq = seq, "Event completed");

        self.update_watermark(&pending);
    }

    /// Maximum failed events before auto-cleanup is triggered.
    const MAX_FAILED_BEFORE_CLEANUP: usize = 10000;

    /// Number of recent failed events to keep after cleanup.
    const KEEP_AFTER_CLEANUP: usize = 1000;

    /// Mark an event as failed (skip it).
    ///
    /// Failed events are removed from pending and added to the failed set.
    /// They don't block watermark advancement.
    ///
    /// Auto-cleanup is triggered when failed count exceeds threshold to prevent
    /// memory leaks in long-running sessions.
    pub async fn fail_event(&self, seq: i64, error: &str) {
        let mut pending = self.pending.write().await;
        pending.remove(&seq);

        {
            let mut failed = self.failed.write().await;
            failed.insert(seq);

            // Auto-cleanup to prevent memory leak in long-running sessions
            if failed.len() > Self::MAX_FAILED_BEFORE_CLEANUP {
                Self::cleanup_failed_internal(&mut failed, Self::KEEP_AFTER_CLEANUP);
            }
        }

        tracing::warn!(
            seq = seq,
            error = error,
            "Event failed, skipping (will not block watermark)"
        );

        self.update_watermark(&pending);
    }

    /// Internal helper to cleanup old failed events.
    ///
    /// Keeps only the most recent `keep_count` failed events based on sequence number.
    fn cleanup_failed_internal(failed: &mut HashSet<i64>, keep_count: usize) {
        let mut sorted: Vec<_> = failed.drain().collect();
        sorted.sort_unstable();
        *failed = sorted.into_iter().rev().take(keep_count).collect();
        tracing::debug!(
            kept = keep_count,
            "Auto-cleaned old failed events to prevent memory leak"
        );
    }

    /// Update watermark based on current pending set.
    ///
    /// Watermark is set to min(pending) - 1, or next_seq - 1 if pending is empty.
    fn update_watermark(&self, pending: &BTreeSet<i64>) {
        let new_watermark = if pending.is_empty() {
            // No pending events, watermark is at the last assigned seq
            self.next_seq.load(Ordering::Acquire) - 1
        } else {
            // Watermark is one less than the minimum pending seq
            *pending.first().expect("pending is not empty") - 1
        };

        let old_watermark = self.watermark.swap(new_watermark, Ordering::SeqCst);

        if new_watermark != old_watermark {
            let lag = self.current_lag_internal(new_watermark);

            tracing::debug!(
                old_watermark = old_watermark,
                new_watermark = new_watermark,
                lag = lag,
                "Watermark updated"
            );

            // Notify listeners of lag change
            let _ = self.lag_changed.send(lag);
        }
    }

    /// Get current lag (number of incomplete events).
    pub fn current_lag(&self) -> i64 {
        let watermark = self.watermark.load(Ordering::Acquire);
        self.current_lag_internal(watermark)
    }

    /// Internal lag calculation.
    fn current_lag_internal(&self, watermark: i64) -> i64 {
        let next = self.next_seq.load(Ordering::Acquire);
        // Lag = (next_seq - 1) - watermark = number of seqs not yet at or past watermark
        // Since next_seq is the next to assign, (next_seq - 1) is the last assigned
        (next - 1) - watermark
    }

    /// Get detailed lag information.
    pub async fn lag_info(&self) -> LagInfo {
        let pending_count = self.pending.read().await.len() as i64;
        let failed_count = self.failed.read().await.len() as i64;
        let watermark = self.watermark.load(Ordering::Acquire);
        let total_assigned = self.next_seq.load(Ordering::Acquire) - 1;
        let lag = self.current_lag_internal(watermark);

        LagInfo {
            total_assigned,
            watermark,
            pending_count,
            failed_count,
            lag,
        }
    }

    /// Wait until lag reaches zero.
    ///
    /// Used in strict mode to wait for all events to complete.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait
    ///
    /// # Returns
    /// * `Ok(())` if lag reached zero
    /// * `Err(LagTimeoutError)` if timeout occurred with lag > 0
    pub async fn wait_for_zero_lag(&self, timeout: Duration) -> Result<(), LagTimeoutError> {
        // Fast path: already at zero
        if self.current_lag() == 0 {
            return Ok(());
        }

        let mut rx = self.lag_changed.subscribe();
        let deadline = Instant::now() + timeout;

        loop {
            let lag = self.current_lag();
            if lag == 0 {
                return Ok(());
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(LagTimeoutError {
                    lag,
                    info: self.lag_info().await,
                });
            }

            tokio::select! {
                _ = tokio::time::sleep(remaining) => {
                    return Err(LagTimeoutError {
                        lag: self.current_lag(),
                        info: self.lag_info().await,
                    });
                }
                result = rx.recv() => {
                    match result {
                        Ok(0) => return Ok(()),
                        Ok(_) => continue, // Lag changed but not zero, keep waiting
                        Err(_) => {
                            // Channel closed, check one more time
                            if self.current_lag() == 0 {
                                return Ok(());
                            }
                            return Err(LagTimeoutError {
                                lag: self.current_lag(),
                                info: self.lag_info().await,
                            });
                        }
                    }
                }
            }
        }
    }

    /// Subscribe to lag change notifications.
    pub fn subscribe(&self) -> broadcast::Receiver<i64> {
        self.lag_changed.subscribe()
    }

    /// Get the current watermark value.
    pub fn watermark(&self) -> i64 {
        self.watermark.load(Ordering::Acquire)
    }

    /// Reset the tracker to initial state.
    ///
    /// Use with caution - only when restarting a new session.
    ///
    /// This method acquires locks before updating atomics to prevent race
    /// conditions with concurrent `complete_event` calls.
    pub async fn reset(&self) {
        // Acquire locks FIRST to prevent race with complete_event
        let mut pending = self.pending.write().await;
        let mut failed = self.failed.write().await;

        // Clear collections while holding locks
        pending.clear();
        failed.clear();

        // Now safe to reset atomics - no concurrent complete_event can
        // see inconsistent state because we hold both locks
        self.next_seq.store(1, Ordering::SeqCst);
        self.watermark.store(0, Ordering::SeqCst);

        tracing::info!("Lag tracker reset");
    }

    /// Get count of failed events.
    ///
    /// This is a quick check that doesn't require waiting for the lock.
    pub async fn failed_count(&self) -> usize {
        self.failed.read().await.len()
    }

    /// Clean up old failed events to prevent memory leak.
    ///
    /// Keeps the most recent `keep_count` failed events based on sequence number.
    /// Call this periodically for long-running systems with many failures.
    ///
    /// # Arguments
    /// * `keep_count` - Number of recent failed events to keep
    pub async fn cleanup_failed(&self, keep_count: usize) {
        let mut failed = self.failed.write().await;
        if failed.len() > keep_count {
            // Sort by seq and keep only the most recent ones
            let mut sorted: Vec<_> = failed.drain().collect();
            sorted.sort_unstable();
            *failed = sorted.into_iter().rev().take(keep_count).collect();

            tracing::debug!(kept = keep_count, "Cleaned up old failed events");
        }
    }
}

impl Default for LagTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared lag tracker wrapped in Arc for use across threads.
pub type SharedLagTracker = Arc<LagTracker>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sequential_completion() {
        let tracker = LagTracker::new();

        // Assign 3 sequences
        let seq1 = tracker.assign_seq();
        let seq2 = tracker.assign_seq();
        let seq3 = tracker.assign_seq();

        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);
        assert_eq!(seq3, 3);
        assert_eq!(tracker.current_lag(), 3);

        // Start all
        tracker.start_event(seq1).await;
        tracker.start_event(seq2).await;
        tracker.start_event(seq3).await;

        // Complete in order
        tracker.complete_event(seq1).await;
        assert_eq!(tracker.watermark(), 1);
        assert_eq!(tracker.current_lag(), 2);

        tracker.complete_event(seq2).await;
        assert_eq!(tracker.watermark(), 2);
        assert_eq!(tracker.current_lag(), 1);

        tracker.complete_event(seq3).await;
        assert_eq!(tracker.watermark(), 3);
        assert_eq!(tracker.current_lag(), 0);
    }

    #[tokio::test]
    async fn test_out_of_order_completion() {
        let tracker = LagTracker::new();

        // Assign 5 sequences
        let seq1 = tracker.assign_seq();
        let seq2 = tracker.assign_seq();
        let seq3 = tracker.assign_seq();
        let seq4 = tracker.assign_seq();
        let seq5 = tracker.assign_seq();

        // Start all
        for seq in [seq1, seq2, seq3, seq4, seq5] {
            tracker.start_event(seq).await;
        }

        assert_eq!(tracker.current_lag(), 5);

        // Complete out of order: 3, 1, 5, 2, 4
        tracker.complete_event(seq3).await;
        assert_eq!(tracker.watermark(), 0); // Still 0, seq 1 and 2 pending
        assert_eq!(tracker.current_lag(), 5);

        tracker.complete_event(seq1).await;
        assert_eq!(tracker.watermark(), 1); // Now 1, only seq 2 blocks
        assert_eq!(tracker.current_lag(), 4);

        tracker.complete_event(seq5).await;
        assert_eq!(tracker.watermark(), 1); // Still 1, seq 2 and 4 pending
        assert_eq!(tracker.current_lag(), 4);

        tracker.complete_event(seq2).await;
        assert_eq!(tracker.watermark(), 3); // Jumps to 3!
        assert_eq!(tracker.current_lag(), 2);

        tracker.complete_event(seq4).await;
        assert_eq!(tracker.watermark(), 5); // All complete
        assert_eq!(tracker.current_lag(), 0);
    }

    #[tokio::test]
    async fn test_failed_events() {
        let tracker = LagTracker::new();

        let seq1 = tracker.assign_seq();
        let seq2 = tracker.assign_seq();
        let seq3 = tracker.assign_seq();

        tracker.start_event(seq1).await;
        tracker.start_event(seq2).await;
        tracker.start_event(seq3).await;

        // seq1 succeeds
        tracker.complete_event(seq1).await;
        assert_eq!(tracker.watermark(), 1);

        // seq2 fails
        tracker.fail_event(seq2, "test error").await;
        assert_eq!(tracker.watermark(), 2); // Watermark advances past failed event

        // seq3 succeeds
        tracker.complete_event(seq3).await;
        assert_eq!(tracker.watermark(), 3);
        assert_eq!(tracker.current_lag(), 0);

        // Check failed count
        let info = tracker.lag_info().await;
        assert_eq!(info.failed_count, 1);
    }

    #[tokio::test]
    async fn test_lag_info() {
        let tracker = LagTracker::new();

        let seq1 = tracker.assign_seq();
        let seq2 = tracker.assign_seq();
        tracker.start_event(seq1).await;
        tracker.start_event(seq2).await;

        let info = tracker.lag_info().await;
        assert_eq!(info.total_assigned, 2);
        assert_eq!(info.pending_count, 2);
        assert_eq!(info.failed_count, 0);
        assert_eq!(info.lag, 2);
        assert_eq!(info.watermark, 0);
    }

    #[tokio::test]
    async fn test_wait_for_zero_lag() {
        let tracker = Arc::new(LagTracker::new());

        let seq1 = tracker.assign_seq();
        tracker.start_event(seq1).await;

        // Spawn a task to complete the event after a delay
        let tracker_clone = Arc::clone(&tracker);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            tracker_clone.complete_event(seq1).await;
        });

        // Wait for zero lag
        let result = tracker.wait_for_zero_lag(Duration::from_secs(1)).await;
        assert!(result.is_ok());
        assert_eq!(tracker.current_lag(), 0);
    }

    #[tokio::test]
    async fn test_wait_for_zero_lag_timeout() {
        let tracker = LagTracker::new();

        let seq1 = tracker.assign_seq();
        tracker.start_event(seq1).await;

        // Don't complete the event, expect timeout
        let result = tracker.wait_for_zero_lag(Duration::from_millis(50)).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.lag, 1);
    }

    #[tokio::test]
    async fn test_reset() {
        let tracker = LagTracker::new();

        let seq1 = tracker.assign_seq();
        tracker.start_event(seq1).await;
        tracker.fail_event(seq1, "test").await;

        assert_eq!(tracker.lag_info().await.failed_count, 1);

        tracker.reset().await;

        let info = tracker.lag_info().await;
        assert_eq!(info.total_assigned, 0);
        assert_eq!(info.pending_count, 0);
        assert_eq!(info.failed_count, 0);
        assert_eq!(info.watermark, 0);
    }

    #[tokio::test]
    async fn test_subscribe() {
        let tracker = LagTracker::new();
        let mut rx = tracker.subscribe();

        let seq1 = tracker.assign_seq();
        tracker.start_event(seq1).await;
        tracker.complete_event(seq1).await;

        // Should receive notification
        let lag = rx.recv().await.unwrap();
        assert_eq!(lag, 0);
    }

    #[tokio::test]
    async fn test_cleanup_failed() {
        let tracker = LagTracker::new();

        // Create 10 failed events
        for _ in 0..10 {
            let seq = tracker.assign_seq();
            tracker.start_event(seq).await;
            tracker.fail_event(seq, "test error").await;
        }

        assert_eq!(tracker.failed_count().await, 10);

        // Cleanup keeping only 3
        tracker.cleanup_failed(3).await;
        assert_eq!(tracker.failed_count().await, 3);

        // Cleanup when under threshold should be no-op
        tracker.cleanup_failed(5).await;
        assert_eq!(tracker.failed_count().await, 3);
    }

    #[tokio::test]
    async fn test_failed_count() {
        let tracker = LagTracker::new();
        assert_eq!(tracker.failed_count().await, 0);

        let seq = tracker.assign_seq();
        tracker.start_event(seq).await;
        tracker.fail_event(seq, "test").await;

        assert_eq!(tracker.failed_count().await, 1);
    }
}
