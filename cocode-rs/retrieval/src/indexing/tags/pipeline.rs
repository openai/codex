//! Tag extraction pipeline for RepoMap functionality.
//!
//! Encapsulates the tag extraction workflow including:
//! - Tree-sitter based tag extraction (definitions, references)
//! - Tag cache management (SQLite L1, in-memory L2)
//! - Parallel processing with worker pool
//!
//! ## Architecture
//!
//! ```text
//!    TriggerSource (SessionStart/Timer/Watcher)
//!          │
//!          ▼
//!    TagEventQueue (dedup by path)
//!          │
//!          ▼
//!    TagWorkerPool
//!          │
//!          ├─► TagEventProcessor
//!          │     ├─ Read file content
//!          │     ├─ Parse with tree-sitter
//!          │     ├─ Extract tags (defs, refs)
//!          │     └─ Update cache
//!          │
//!          ├─► BatchTracker (SessionStart completion)
//!          └─► LagTracker (watermark-based lag)
//!                    │
//!                    ▼
//!              Readiness check
//! ```

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::repomap::RepoMapCache;
use crate::tags::extractor::TagExtractor;

// Import from parent indexing module (use crate:: since super:: refers to tags/)
use crate::indexing::BatchId;
use crate::indexing::BatchResult;
use crate::indexing::BatchTracker;
use crate::indexing::EventProcessor;
use crate::indexing::FileIndexLocks;
use crate::indexing::LagInfo;
use crate::indexing::LagTracker;
use crate::indexing::PipelineReadiness;
use crate::indexing::PipelineState;
use crate::indexing::TagEventKind;
use crate::indexing::TagEventQueue;
use crate::indexing::TrackedEvent;
use crate::indexing::WorkerPool;
use crate::indexing::WorkerPoolConfig;
use crate::indexing::compute_readiness;
use crate::indexing::new_tag_event_queue;
use crate::indexing::now_timestamp;

// Re-export StrictModeConfig with alias for backward compatibility
pub use crate::indexing::StrictModeConfig as TagStrictModeConfig;

/// Tag extraction statistics.
#[derive(Debug, Clone, PartialEq)]
pub struct TagStats {
    /// Number of files with extracted tags.
    pub file_count: i64,
    /// Total number of tags extracted.
    pub tag_count: i64,
    /// Unix timestamp of last extraction.
    pub last_extracted: Option<i64>,
}

impl Default for TagStats {
    fn default() -> Self {
        Self {
            file_count: 0,
            tag_count: 0,
            last_extracted: None,
        }
    }
}

/// Type alias for tag pipeline state using common generic type.
pub type TagPipelineState = PipelineState<TagStats>;

/// Type alias for tag pipeline readiness using common generic type.
pub type TagReadiness = PipelineReadiness<TagStats>;

/// Tag event processor that handles tag extraction.
#[allow(dead_code)] // Fields will be used when extraction logic is fully implemented
pub struct TagEventProcessor {
    /// Tag cache for storing extracted tags.
    cache: Arc<RepoMapCache>,
    /// Workspace root directory.
    workdir: PathBuf,
}

impl std::fmt::Debug for TagEventProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TagEventProcessor")
            .field("workdir", &self.workdir)
            .finish()
    }
}

impl TagEventProcessor {
    /// Create a new tag event processor.
    pub fn new(cache: Arc<RepoMapCache>, workdir: PathBuf) -> Self {
        Self { cache, workdir }
    }
}

#[async_trait]
impl EventProcessor for TagEventProcessor {
    type EventData = TagEventKind;

    async fn process(&self, path: &Path, event: &TrackedEvent<Self::EventData>) -> Result<()> {
        let trace_id = &event.trace_id;
        let filepath = path.to_string_lossy().to_string();

        tracing::debug!(
            trace_id = %trace_id,
            path = %path.display(),
            kind = ?event.data,
            "TagEventProcessor: processing file"
        );

        // Check file existence to determine action
        // This is more robust than trusting event type (handles race conditions)
        if path.exists() {
            // Record mtime BEFORE extraction (for optimistic lock)
            let mtime_before = RepoMapCache::file_mtime(&filepath);

            // File exists - extract/update tags
            let mut extractor = TagExtractor::new();
            match extractor.extract_file(path) {
                Ok(tags) => {
                    let tag_count = tags.len();
                    // Cache the tags with optimistic lock validation
                    let written = self.cache.put_tags(&filepath, &tags, mtime_before).await?;

                    if written {
                        tracing::debug!(
                            trace_id = %trace_id,
                            path = %path.display(),
                            tags = tag_count,
                            "Tags extracted and cached"
                        );
                    } else {
                        tracing::debug!(
                            trace_id = %trace_id,
                            path = %path.display(),
                            tags = tag_count,
                            "Tags extracted but cache write skipped (newer version exists)"
                        );
                    }
                }
                Err(e) => {
                    tracing::debug!(
                        trace_id = %trace_id,
                        path = %path.display(),
                        error = %e,
                        "Failed to extract tags (may be unsupported language)"
                    );
                    // Continue without error - unsupported languages are expected
                }
            }
        } else {
            // File doesn't exist - remove tags from cache
            self.cache.invalidate_tags(&filepath).await?;

            tracing::debug!(
                trace_id = %trace_id,
                path = %path.display(),
                "Tags removed from cache"
            );
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "tag-processor"
    }
}

/// Type alias for the tag worker pool.
pub type TagWorkerPool = WorkerPool<PathBuf, TagEventKind, TagEventProcessor>;

/// Tag extraction pipeline for RepoMap functionality.
pub struct TagPipeline {
    /// Current state of the pipeline.
    state: RwLock<TagPipelineState>,
    /// Event queue for file changes.
    event_queue: Arc<TagEventQueue>,
    /// File-level locks.
    file_locks: Arc<FileIndexLocks>,
    /// Batch tracker for SessionStart completion.
    batch_tracker: Arc<BatchTracker>,
    /// Lag tracker for watermark mechanism.
    lag_tracker: Arc<LagTracker>,
    /// Cancellation token.
    cancel: CancellationToken,
    /// Event processor.
    processor: Arc<TagEventProcessor>,
    /// Worker pool (initialized lazily).
    worker_pool: RwLock<Option<Arc<TagWorkerPool>>>,
    /// Whether initial build has completed.
    init_complete: AtomicBool,
    /// Strict mode configuration.
    strict_config: TagStrictModeConfig,
    /// Worker pool configuration.
    worker_config: WorkerPoolConfig,
}

impl TagPipeline {
    /// Create a new tag pipeline.
    pub fn new(
        cache: Arc<RepoMapCache>,
        workdir: PathBuf,
        strict_config: TagStrictModeConfig,
        worker_count: i32,
    ) -> Self {
        let event_queue = Arc::new(new_tag_event_queue(256));
        let file_locks = Arc::new(FileIndexLocks::new());
        let batch_tracker = Arc::new(BatchTracker::new());
        let lag_tracker = Arc::new(LagTracker::new());
        let cancel = CancellationToken::new();

        let processor = Arc::new(TagEventProcessor::new(cache, workdir));

        let worker_config = WorkerPoolConfig {
            worker_count,
            requeue_delay_ms: 10,
        };

        Self {
            state: RwLock::new(TagPipelineState::Uninitialized),
            event_queue,
            file_locks,
            batch_tracker,
            lag_tracker,
            cancel,
            processor,
            worker_pool: RwLock::new(None),
            init_complete: AtomicBool::new(false),
            strict_config,
            worker_config,
        }
    }

    /// Start the worker pool.
    pub async fn start_workers(&self) {
        let mut pool_guard = self.worker_pool.write().await;
        if pool_guard.is_none() {
            let pool = Arc::new(WorkerPool::new(
                Arc::clone(&self.event_queue),
                Arc::clone(&self.processor),
                Arc::clone(&self.file_locks),
                Arc::clone(&self.batch_tracker),
                Arc::clone(&self.lag_tracker),
                self.cancel.clone(),
                self.worker_config.clone(),
            ));
            pool.start();
            *pool_guard = Some(pool);

            tracing::info!("Tag pipeline workers started");
        }
    }

    /// Stop the worker pool.
    pub async fn stop(&self) {
        self.cancel.cancel();
        tracing::info!("Tag pipeline stopped");
    }

    /// Check if the pipeline is stopped.
    pub fn is_stopped(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// Get the current state.
    pub async fn state(&self) -> TagPipelineState {
        self.state.read().await.clone()
    }

    /// Mark the pipeline as building.
    pub async fn mark_building(&self, batch_id: BatchId) {
        *self.state.write().await = TagPipelineState::Building {
            batch_id,
            progress: 0.0,
            started_at: now_timestamp(),
        };
    }

    /// Update building progress.
    pub async fn update_progress(&self, progress: f32) {
        let mut state = self.state.write().await;
        if let TagPipelineState::Building {
            batch_id,
            started_at,
            ..
        } = &*state
        {
            *state = TagPipelineState::Building {
                batch_id: batch_id.clone(),
                progress,
                started_at: *started_at,
            };
        }
    }

    /// Mark the pipeline as ready.
    ///
    /// Also triggers cleanup of file locks to prevent memory leaks from
    /// any locks that might have been missed during per-file cleanup.
    pub async fn mark_ready(&self, stats: TagStats) {
        *self.state.write().await = TagPipelineState::Ready {
            stats,
            completed_at: now_timestamp(),
        };
        self.init_complete.store(true, Ordering::Release);

        // Cleanup any remaining file locks to prevent memory leaks
        self.file_locks.cleanup_all().await;
        tracing::debug!("Cleaned up file locks after tag pipeline completion");
    }

    /// Mark the pipeline as failed.
    pub async fn mark_failed(&self, error: String) {
        *self.state.write().await = TagPipelineState::Failed {
            error,
            failed_at: now_timestamp(),
        };
    }

    /// Get the event queue for pushing events.
    pub fn event_queue(&self) -> Arc<TagEventQueue> {
        Arc::clone(&self.event_queue)
    }

    /// Get the batch tracker.
    pub fn batch_tracker(&self) -> Arc<BatchTracker> {
        Arc::clone(&self.batch_tracker)
    }

    /// Get the lag tracker.
    pub fn lag_tracker(&self) -> Arc<LagTracker> {
        Arc::clone(&self.lag_tracker)
    }

    /// Assign a sequence number for a new event.
    pub fn assign_seq(&self) -> i64 {
        self.lag_tracker.assign_seq()
    }

    /// Start a new batch for SessionStart.
    pub async fn start_batch(
        &self,
        batch_id: BatchId,
        total: i64,
    ) -> tokio::sync::oneshot::Receiver<BatchResult> {
        self.batch_tracker.start_batch(batch_id, total).await
    }

    /// Push an event to the queue.
    pub async fn push_event(&self, path: PathBuf, event: TrackedEvent<TagEventKind>) {
        self.event_queue.push(path, event).await;
    }

    /// Push a simple event without tracking.
    pub async fn push_simple(&self, path: PathBuf, kind: TagEventKind) {
        self.event_queue.push_simple(path, kind).await;
    }

    /// Get current lag.
    pub fn current_lag(&self) -> i64 {
        self.lag_tracker.current_lag()
    }

    /// Get detailed lag info.
    pub async fn lag_info(&self) -> LagInfo {
        self.lag_tracker.lag_info().await
    }

    /// Check if initial build is complete.
    pub fn is_init_complete(&self) -> bool {
        self.init_complete.load(Ordering::Acquire)
    }

    /// Get the readiness status.
    pub async fn readiness(&self) -> TagReadiness {
        let state = self.state.read().await.clone();
        let lag_info = self.lag_tracker.lag_info().await;
        compute_readiness(
            &state,
            lag_info,
            self.is_init_complete(),
            &self.strict_config,
        )
    }

    /// Check if ready for RepoMap generation (quick check).
    pub async fn is_ready(&self) -> bool {
        matches!(self.readiness().await, TagReadiness::Ready { .. })
    }
}

impl std::fmt::Debug for TagPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TagPipeline")
            .field("is_stopped", &self.is_stopped())
            .field("init_complete", &self.is_init_complete())
            .field("current_lag", &self.current_lag())
            .finish()
    }
}

/// Shared tag pipeline.
pub type SharedTagPipeline = Arc<TagPipeline>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SqliteStore;
    use tempfile::TempDir;

    async fn create_test_pipeline() -> (TempDir, TagPipeline) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Arc::new(SqliteStore::open(&db_path).unwrap());
        let cache = Arc::new(RepoMapCache::new(db));

        let strict_config = TagStrictModeConfig::default();

        let pipeline = TagPipeline::new(
            cache,
            dir.path().to_path_buf(),
            strict_config,
            2, // worker count
        );

        (dir, pipeline)
    }

    #[tokio::test]
    async fn test_pipeline_initial_state() {
        let (_dir, pipeline) = create_test_pipeline().await;
        assert!(matches!(
            pipeline.state().await,
            TagPipelineState::Uninitialized
        ));
        assert!(!pipeline.is_init_complete());
    }

    #[tokio::test]
    async fn test_pipeline_building_state() {
        let (_dir, pipeline) = create_test_pipeline().await;
        let batch_id = BatchId::new();

        pipeline.mark_building(batch_id.clone()).await;

        let state = pipeline.state().await;
        assert!(matches!(state, TagPipelineState::Building { .. }));

        pipeline.update_progress(0.5).await;

        if let TagPipelineState::Building { progress, .. } = pipeline.state().await {
            assert_eq!(progress, 0.5);
        } else {
            panic!("Expected Building state");
        }
    }

    #[tokio::test]
    async fn test_pipeline_ready_state() {
        let (_dir, pipeline) = create_test_pipeline().await;

        let stats = TagStats {
            file_count: 10,
            tag_count: 100,
            last_extracted: Some(chrono::Utc::now().timestamp()),
        };

        pipeline.mark_ready(stats.clone()).await;

        assert!(pipeline.is_init_complete());

        if let TagPipelineState::Ready { stats: s, .. } = pipeline.state().await {
            assert_eq!(s.file_count, 10);
            assert_eq!(s.tag_count, 100);
        } else {
            panic!("Expected Ready state");
        }
    }

    #[tokio::test]
    async fn test_pipeline_readiness() {
        let (_dir, pipeline) = create_test_pipeline().await;

        // Initially uninitialized
        assert!(matches!(
            pipeline.readiness().await,
            TagReadiness::Uninitialized
        ));

        // Building
        let batch_id = BatchId::new();
        pipeline.mark_building(batch_id).await;
        assert!(matches!(
            pipeline.readiness().await,
            TagReadiness::Building { .. }
        ));

        // Ready
        let stats = TagStats {
            file_count: 5,
            tag_count: 50,
            last_extracted: Some(chrono::Utc::now().timestamp()),
        };
        pipeline.mark_ready(stats).await;

        // Should be ready (no lag)
        assert!(matches!(
            pipeline.readiness().await,
            TagReadiness::Ready { .. }
        ));
        assert!(pipeline.is_ready().await);
    }

    #[tokio::test]
    async fn test_pipeline_push_event() {
        let (_dir, pipeline) = create_test_pipeline().await;

        let seq = pipeline.assign_seq();
        let event = TrackedEvent::new(TagEventKind::Changed, None, seq, "test-trace".to_string());

        pipeline.push_event(PathBuf::from("test.rs"), event).await;

        assert_eq!(pipeline.event_queue().len().await, 1);
    }

    #[tokio::test]
    async fn test_pipeline_lag_tracking() {
        let (_dir, pipeline) = create_test_pipeline().await;

        // Assign some sequences
        let _seq1 = pipeline.assign_seq();
        let _seq2 = pipeline.assign_seq();

        // Initial lag should be 2
        assert_eq!(pipeline.current_lag(), 2);

        let info = pipeline.lag_info().await;
        assert_eq!(info.lag, 2);
    }

    #[tokio::test]
    async fn test_pipeline_stop() {
        let (_dir, pipeline) = create_test_pipeline().await;

        assert!(!pipeline.is_stopped());
        pipeline.stop().await;
        assert!(pipeline.is_stopped());
    }
}
