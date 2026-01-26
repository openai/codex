//! Generic worker pool for parallel event processing.
//!
//! Provides a configurable worker pool that processes events from a queue
//! with support for:
//! - Parallel processing with configurable worker count
//! - File-level locking to prevent concurrent processing of the same file
//! - Batch tracking for SessionStart completion detection
//! - Lag tracking with watermark mechanism
//!
//! ## Architecture
//!
//! ```text
//!                     ┌─────────────────┐
//!                     │   EventQueue    │
//!                     │  (dedup/merge)  │
//!                     └────────┬────────┘
//!                              │
//!          ┌───────────────────┼───────────────────┐
//!          ▼                   ▼                   ▼
//!    ┌──────────┐       ┌──────────┐       ┌──────────┐
//!    │ Worker 1 │       │ Worker 2 │       │ Worker N │
//!    └────┬─────┘       └────┬─────┘       └────┬─────┘
//!         │                  │                  │
//!         ▼                  ▼                  ▼
//!    ┌──────────────────────────────────────────────┐
//!    │          EventProcessor (trait)              │
//!    │ - IndexEventProcessor (chunks, embeddings)   │
//!    │ - TagEventProcessor (tree-sitter tags)       │
//!    └──────────────────────────────────────────────┘
//!         │                  │                  │
//!         ▼                  ▼                  ▼
//!    ┌──────────┐       ┌──────────┐       ┌──────────┐
//!    │ Batch    │       │ Lag      │       │ File     │
//!    │ Tracker  │       │ Tracker  │       │ Locks    │
//!    └──────────┘       └──────────┘       └──────────┘
//! ```

use std::hash::Hash;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::BatchTracker;
use super::EventQueue;
use super::LagTracker;
use super::SharedFileLocks;
use super::TrackedEvent;
use crate::error::Result;

/// Trait for processing events from the queue.
///
/// Implement this trait to define how events are processed.
/// The processor must be `Send + Sync` for use in a worker pool.
#[async_trait]
pub trait EventProcessor: Send + Sync + std::fmt::Debug {
    /// The type of event data this processor handles.
    type EventData: Clone + Send + Sync;

    /// Process a single event.
    ///
    /// # Arguments
    /// * `path` - Path to the file being processed
    /// * `event` - The tracked event with metadata
    ///
    /// # Returns
    /// * `Ok(())` if processing succeeded
    /// * `Err(e)` if processing failed (event will be marked as failed)
    async fn process(&self, path: &Path, event: &TrackedEvent<Self::EventData>) -> Result<()>;

    /// Get the name of this processor for logging.
    fn name(&self) -> &str;
}

/// Configuration for the worker pool.
#[derive(Debug, Clone)]
pub struct WorkerPoolConfig {
    /// Number of worker threads.
    pub worker_count: i32,
    /// Delay before retrying a requeued event (lock conflict).
    pub requeue_delay_ms: i64,
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self {
            worker_count: 4,
            requeue_delay_ms: 10,
        }
    }
}

/// Generic worker pool for parallel event processing.
///
/// # Type Parameters
/// - `K`: Key type for the event queue (e.g., `PathBuf`)
/// - `V`: Value type for events (e.g., `WatchEventKind`)
/// - `P`: Event processor implementation
pub struct WorkerPool<K, V, P>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    P: EventProcessor<EventData = V> + 'static,
{
    /// Event queue to consume from.
    queue: Arc<EventQueue<K, V>>,
    /// Event processor implementation.
    processor: Arc<P>,
    /// File-level locks for concurrency control.
    file_locks: SharedFileLocks,
    /// Batch tracker for SessionStart completion.
    batch_tracker: Arc<BatchTracker>,
    /// Lag tracker for watermark-based lag detection.
    lag_tracker: Arc<LagTracker>,
    /// Cancellation token for graceful shutdown.
    cancel: CancellationToken,
    /// Configuration.
    config: WorkerPoolConfig,
    /// Number of active workers.
    active_workers: AtomicI64,
}

impl<K, V, P> WorkerPool<K, V, P>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    P: EventProcessor<EventData = V> + 'static,
{
    /// Create a new worker pool.
    ///
    /// # Arguments
    /// * `queue` - Event queue to consume from
    /// * `processor` - Event processor implementation
    /// * `file_locks` - File-level locks for concurrency control
    /// * `batch_tracker` - Batch tracker for completion detection
    /// * `lag_tracker` - Lag tracker for watermark mechanism
    /// * `cancel` - Cancellation token for shutdown
    /// * `config` - Worker pool configuration
    pub fn new(
        queue: Arc<EventQueue<K, V>>,
        processor: Arc<P>,
        file_locks: SharedFileLocks,
        batch_tracker: Arc<BatchTracker>,
        lag_tracker: Arc<LagTracker>,
        cancel: CancellationToken,
        config: WorkerPoolConfig,
    ) -> Self {
        Self {
            queue,
            processor,
            file_locks,
            batch_tracker,
            lag_tracker,
            cancel,
            config,
            active_workers: AtomicI64::new(0),
        }
    }

    /// Get the number of active workers.
    pub fn active_workers(&self) -> i64 {
        self.active_workers.load(Ordering::Acquire)
    }

    /// Check if the pool is stopped.
    pub fn is_stopped(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// Stop all workers.
    pub fn stop(&self) {
        self.cancel.cancel();
    }
}

/// Worker pool for PathBuf-keyed events (files).
impl<V, P> WorkerPool<PathBuf, V, P>
where
    V: Clone + Send + Sync + 'static,
    P: EventProcessor<EventData = V> + 'static,
{
    /// Start the worker pool with the configured number of workers.
    ///
    /// Workers will process events until the cancellation token is triggered.
    pub fn start(self: &Arc<Self>) {
        let worker_count = self.config.worker_count;

        tracing::info!(
            processor = self.processor.name(),
            workers = worker_count,
            "Starting worker pool"
        );

        for id in 0..worker_count {
            let pool = Arc::clone(self);
            tokio::spawn(async move {
                pool.worker_loop(id).await;
            });
        }
    }

    /// Main worker loop.
    async fn worker_loop(self: Arc<Self>, worker_id: i32) {
        self.active_workers.fetch_add(1, Ordering::AcqRel);

        tracing::debug!(
            worker_id = worker_id,
            processor = self.processor.name(),
            "Worker started"
        );

        let mut rx = self.queue.subscribe();

        // Process any events that were pushed before we subscribed
        self.process_available_events(worker_id).await;

        loop {
            tokio::select! {
                biased;

                _ = self.cancel.cancelled() => {
                    tracing::debug!(worker_id = worker_id, "Worker cancelled");
                    break;
                }

                result = rx.recv() => {
                    match result {
                        Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            // Process all available events
                            self.process_available_events(worker_id).await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            // Channel closed, exit
                            break;
                        }
                    }
                }

                // Periodic poll as a fallback (in case we miss notifications)
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    self.process_available_events(worker_id).await;
                }
            }
        }

        self.active_workers.fetch_sub(1, Ordering::AcqRel);

        tracing::debug!(
            worker_id = worker_id,
            processor = self.processor.name(),
            "Worker stopped"
        );
    }

    /// Process all available events in the queue.
    async fn process_available_events(&self, worker_id: i32) {
        while let Some((path, event)) = self.queue.pop().await {
            // Mark event as started in lag tracker
            self.lag_tracker.start_event(event.seq).await;

            let start_time = Instant::now();
            let trace_id = &event.trace_id;
            let batch_ids = event.batch_ids.clone();
            let seq = event.seq;

            tracing::debug!(
                worker_id = worker_id,
                trace_id = %trace_id,
                path = %path.display(),
                seq = seq,
                "Processing event"
            );

            // Try to acquire file lock
            // IMPORTANT: Keep the guard alive during processing to prevent
            // concurrent processing of the same file
            let lock_guard = self.file_locks.try_lock(&path).await;

            if lock_guard.is_some() {
                // Process the event (lock is held via lock_guard)
                let result = self.processor.process(&path, &event).await;
                let duration = start_time.elapsed();

                // Complete any merged sequence numbers first
                for merged_seq in &event.merged_seqs {
                    self.lag_tracker.complete_event(*merged_seq).await;
                }

                match result {
                    Ok(()) => {
                        tracing::debug!(
                            worker_id = worker_id,
                            trace_id = %trace_id,
                            path = %path.display(),
                            seq = seq,
                            duration_ms = duration.as_millis() as i64,
                            "Event processed successfully"
                        );

                        // Mark complete in lag tracker
                        self.lag_tracker.complete_event(seq).await;

                        // Mark complete in batch tracker for ALL batch_ids
                        for bid in &batch_ids {
                            self.batch_tracker.mark_complete(bid, true).await;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            worker_id = worker_id,
                            trace_id = %trace_id,
                            path = %path.display(),
                            seq = seq,
                            error = %e,
                            duration_ms = duration.as_millis() as i64,
                            "Event processing failed"
                        );

                        // Mark failed in lag tracker (doesn't block watermark)
                        self.lag_tracker.fail_event(seq, &e.to_string()).await;

                        // Mark failed in batch tracker for ALL batch_ids
                        for bid in &batch_ids {
                            self.batch_tracker.mark_complete(bid, false).await;
                        }
                    }
                }

                // Explicitly drop lock guard before cleanup
                drop(lock_guard);

                // Clean up file lock entry from tracking map
                self.file_locks.cleanup(&path).await;
            } else {
                // Lock contention, requeue the event
                tracing::trace!(
                    worker_id = worker_id,
                    trace_id = %trace_id,
                    path = %path.display(),
                    "Lock contention, requeueing event"
                );

                self.queue.requeue(path, event).await;

                // Brief sleep to avoid busy loop
                tokio::time::sleep(Duration::from_millis(self.config.requeue_delay_ms as u64))
                    .await;
            }
        }
    }
}

impl<K, V, P> std::fmt::Debug for WorkerPool<K, V, P>
where
    K: Hash + Eq + Clone + Send + Sync,
    V: Clone + Send + Sync,
    P: EventProcessor<EventData = V>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerPool")
            .field("processor", &self.processor.name())
            .field("worker_count", &self.config.worker_count)
            .field(
                "active_workers",
                &self.active_workers.load(Ordering::Acquire),
            )
            .field("is_stopped", &self.is_stopped())
            .finish()
    }
}

/// Shared worker pool wrapped in Arc.
pub type SharedWorkerPool<K, V, P> = Arc<WorkerPool<K, V, P>>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexing::FileIndexLocks;
    use crate::indexing::WatchEventKind;
    use crate::indexing::new_watch_event_queue;
    use std::sync::atomic::AtomicI32;

    /// Test processor that counts processed events.
    #[derive(Debug)]
    struct CountingProcessor {
        count: AtomicI32,
    }

    impl CountingProcessor {
        fn new() -> Self {
            Self {
                count: AtomicI32::new(0),
            }
        }

        fn count(&self) -> i32 {
            self.count.load(Ordering::Acquire)
        }
    }

    #[async_trait]
    impl EventProcessor for CountingProcessor {
        type EventData = WatchEventKind;

        async fn process(
            &self,
            _path: &Path,
            _event: &TrackedEvent<Self::EventData>,
        ) -> Result<()> {
            self.count.fetch_add(1, Ordering::AcqRel);
            Ok(())
        }

        fn name(&self) -> &str {
            "counting-processor"
        }
    }

    /// Test processor that fails on specific paths.
    #[derive(Debug)]
    struct FailingProcessor {
        fail_pattern: String,
    }

    impl FailingProcessor {
        fn new(fail_pattern: &str) -> Self {
            Self {
                fail_pattern: fail_pattern.to_string(),
            }
        }
    }

    #[async_trait]
    impl EventProcessor for FailingProcessor {
        type EventData = WatchEventKind;

        async fn process(&self, path: &Path, _event: &TrackedEvent<Self::EventData>) -> Result<()> {
            if path.to_string_lossy().contains(&self.fail_pattern) {
                Err(crate::error::RetrievalErr::SearchFailed {
                    query: "test".to_string(),
                    cause: "Simulated failure".to_string(),
                })
            } else {
                Ok(())
            }
        }

        fn name(&self) -> &str {
            "failing-processor"
        }
    }

    fn create_test_pool<P: EventProcessor<EventData = WatchEventKind> + 'static>(
        processor: Arc<P>,
    ) -> (
        Arc<WorkerPool<PathBuf, WatchEventKind, P>>,
        Arc<crate::indexing::WatchEventQueue>,
        Arc<LagTracker>,
        Arc<BatchTracker>,
    ) {
        let queue = Arc::new(new_watch_event_queue(64));
        let file_locks = Arc::new(FileIndexLocks::new());
        let batch_tracker = Arc::new(BatchTracker::new());
        let lag_tracker = Arc::new(LagTracker::new());
        let cancel = CancellationToken::new();

        let config = WorkerPoolConfig {
            worker_count: 2,
            requeue_delay_ms: 1,
        };

        let pool = Arc::new(WorkerPool::new(
            Arc::clone(&queue),
            processor,
            file_locks,
            Arc::clone(&batch_tracker),
            Arc::clone(&lag_tracker),
            cancel,
            config,
        ));

        (pool, queue, lag_tracker, batch_tracker)
    }

    #[tokio::test]
    async fn test_worker_pool_basic() {
        let processor = Arc::new(CountingProcessor::new());
        let (pool, queue, lag_tracker, _) = create_test_pool(Arc::clone(&processor));

        // Start the pool
        pool.start();

        // Push some events with seq numbers
        for i in 0..5 {
            let seq = lag_tracker.assign_seq();
            let event = TrackedEvent::new(WatchEventKind::Changed, None, seq, format!("trace-{i}"));
            queue
                .push(PathBuf::from(format!("file{i}.rs")), event)
                .await;
        }

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert_eq!(processor.count(), 5);
        assert_eq!(lag_tracker.current_lag(), 0);

        pool.stop();
    }

    #[tokio::test]
    async fn test_worker_pool_with_batch() {
        let processor = Arc::new(CountingProcessor::new());
        let (pool, queue, lag_tracker, batch_tracker) = create_test_pool(Arc::clone(&processor));

        pool.start();

        // Create a batch
        let batch_id = crate::indexing::BatchId::new();
        let rx = batch_tracker.start_batch(batch_id.clone(), 3).await;

        // Push events with batch_id
        for i in 0..3 {
            let seq = lag_tracker.assign_seq();
            let event = TrackedEvent::new(
                WatchEventKind::Changed,
                Some(batch_id.clone()),
                seq,
                format!("batch-trace-{i}"),
            );
            queue
                .push(PathBuf::from(format!("batch{i}.rs")), event)
                .await;
        }

        // Wait for batch completion
        let result = tokio::time::timeout(Duration::from_secs(1), rx).await;
        assert!(result.is_ok());

        let batch_result = result.unwrap().unwrap();
        assert_eq!(batch_result.completed, 3);
        assert_eq!(batch_result.failed, 0);

        pool.stop();
    }

    #[tokio::test]
    async fn test_worker_pool_with_failures() {
        let processor = Arc::new(FailingProcessor::new("fail"));
        let (pool, queue, lag_tracker, batch_tracker) = create_test_pool(Arc::clone(&processor));

        pool.start();

        let batch_id = crate::indexing::BatchId::new();
        let rx = batch_tracker.start_batch(batch_id.clone(), 3).await;

        // One will fail, two will succeed
        let paths = ["ok1.rs", "fail.rs", "ok2.rs"];
        for (i, path) in paths.iter().enumerate() {
            let seq = lag_tracker.assign_seq();
            let event = TrackedEvent::new(
                WatchEventKind::Changed,
                Some(batch_id.clone()),
                seq,
                format!("trace-{i}"),
            );
            queue.push(PathBuf::from(path), event).await;
        }

        // Wait for batch completion
        let result = tokio::time::timeout(Duration::from_secs(1), rx).await;
        assert!(result.is_ok());

        let batch_result = result.unwrap().unwrap();
        assert_eq!(batch_result.completed, 2);
        assert_eq!(batch_result.failed, 1);

        // Lag should still be 0 (failed events don't block)
        assert_eq!(lag_tracker.current_lag(), 0);

        // Check failed count in lag tracker
        let info = lag_tracker.lag_info().await;
        assert_eq!(info.failed_count, 1);

        pool.stop();
    }

    #[tokio::test]
    async fn test_worker_pool_stop() {
        let processor = Arc::new(CountingProcessor::new());
        let (pool, _, _, _) = create_test_pool(Arc::clone(&processor));

        pool.start();

        // Wait for workers to start
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(pool.active_workers() > 0);

        // Stop the pool
        pool.stop();

        // Wait for workers to stop
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(pool.is_stopped());
    }

    #[tokio::test]
    async fn test_worker_pool_lag_tracking() {
        let processor = Arc::new(CountingProcessor::new());
        let (pool, queue, lag_tracker, _) = create_test_pool(Arc::clone(&processor));

        // Assign seq numbers before starting pool
        let seqs: Vec<i64> = (0..5).map(|_| lag_tracker.assign_seq()).collect();

        // Push events
        for (i, seq) in seqs.iter().enumerate() {
            let event =
                TrackedEvent::new(WatchEventKind::Changed, None, *seq, format!("trace-{i}"));
            queue
                .push(PathBuf::from(format!("file{i}.rs")), event)
                .await;
        }

        // Lag should be 5 before processing
        assert_eq!(lag_tracker.current_lag(), 5);

        pool.start();

        // Wait for processing
        let result = lag_tracker.wait_for_zero_lag(Duration::from_secs(1)).await;
        assert!(result.is_ok());

        assert_eq!(lag_tracker.current_lag(), 0);

        pool.stop();
    }
}
