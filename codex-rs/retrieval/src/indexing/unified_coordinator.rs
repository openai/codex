//! Unified coordinator for both Search and RepoMap pipelines.
//!
//! Manages shared file scanning and dispatches events to both IndexPipeline
//! (for search) and TagPipeline (for RepoMap) based on enabled features.
//!
//! ## Architecture
//!
//! ```text
//!    ┌─────────────────────────────────────────────────────────────────┐
//!    │                    UnifiedCoordinator                           │
//!    │                                                                 │
//!    │  ┌──────────────────────────────────────────────────────────┐  │
//!    │  │                   File Scanner (shared)                   │  │
//!    │  │  SessionStart: scan_all_files() → Vec<PathBuf>           │  │
//!    │  │  Timer: check_freshness() → Vec<FileChange>              │  │
//!    │  │  Watcher: file events → dispatch                         │  │
//!    │  └──────────────────────────────────────────────────────────┘  │
//!    │                         │                                       │
//!    │           ┌─────────────┴─────────────┐                        │
//!    │           ▼                           ▼                        │
//!    │  ┌─────────────────────┐    ┌─────────────────────┐           │
//!    │  │  IndexPipeline      │    │  TagPipeline        │           │
//!    │  │  (if search_enabled)│    │  (if repomap_enabled)│           │
//!    │  │                     │    │                     │           │
//!    │  │  ┌─IndexEventQueue─┐│    │  ┌─TagEventQueue───┐│           │
//!    │  │  │ dedup by path   ││    │  │ dedup by path   ││           │
//!    │  │  └────────┬────────┘│    │  └────────┬────────┘│           │
//!    │  │           ▼         │    │           ▼         │           │
//!    │  │  IndexWorkerPool    │    │  TagWorkerPool      │           │
//!    │  │  (chunks, embeds)   │    │  (tree-sitter tags) │           │
//!    │  │           │         │    │           │         │           │
//!    │  │  ┌────────▼────────┐│    │  ┌────────▼────────┐│           │
//!    │  │  │ LagTracker      ││    │  │ LagTracker      ││           │
//!    │  │  │ BatchTracker    ││    │  │ BatchTracker    ││           │
//!    │  │  └─────────────────┘│    │  └─────────────────┘│           │
//!    │  └─────────────────────┘    └─────────────────────┘           │
//!    └─────────────────────────────────────────────────────────────────┘
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use super::BatchId;
use super::CoordinatorFileChange;
use super::FileWalker;
use super::IndexStats;
use super::TrackedEvent;
use super::TriggerSource;
use super::WatchEventKind;
use super::index_pipeline::IndexPipeline;
use super::index_pipeline::Readiness as IndexReadiness;
use super::index_pipeline::StrictModeConfig as IndexStrictModeConfig;
use crate::config::RetrievalConfig;
use crate::error::Result;
use crate::repomap::RepoMapCache;
use crate::storage::SqliteStore;

// Import from local tags module (now in indexing/)
use super::TagEventKind;
use super::tags::TagPipeline;
use super::tags::TagReadiness;
use super::tags::TagStats;
use super::tags::TagStrictModeConfig;

/// Feature flags for the unified coordinator.
#[derive(Debug, Clone)]
pub struct FeatureFlags {
    /// Enable search (chunking, embeddings, BM25).
    pub search_enabled: bool,
    /// Enable repomap (tag extraction, PageRank).
    pub repomap_enabled: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            search_enabled: true,
            repomap_enabled: true,
        }
    }
}

/// Result of a SessionStart trigger.
#[derive(Debug)]
pub struct SessionStartResult {
    /// Batch ID for tracking.
    pub batch_id: BatchId,
    /// Number of files to process.
    pub file_count: i64,
    /// Receiver for index pipeline completion (if enabled).
    pub index_receiver: Option<tokio::sync::oneshot::Receiver<super::BatchResult>>,
    /// Receiver for tag pipeline completion (if enabled).
    pub tag_receiver: Option<tokio::sync::oneshot::Receiver<super::BatchResult>>,
}

/// Unified coordinator state.
#[derive(Debug, Clone, PartialEq)]
pub enum UnifiedState {
    /// Both pipelines uninitialized.
    Uninitialized,
    /// At least one pipeline is building.
    Building {
        /// Search pipeline state.
        search_building: bool,
        /// RepoMap pipeline state.
        repomap_building: bool,
    },
    /// Both enabled pipelines are ready.
    Ready,
    /// At least one pipeline failed.
    Failed {
        /// Error message.
        error: String,
    },
}

/// Unified coordinator for both Search and RepoMap pipelines.
pub struct UnifiedCoordinator {
    /// Configuration.
    config: RetrievalConfig,
    /// Feature flags.
    features: FeatureFlags,
    /// Workspace root directory.
    workdir: PathBuf,
    /// SQLite store.
    #[allow(dead_code)]
    db: Arc<SqliteStore>,
    /// Index pipeline (for search).
    index_pipeline: Option<Arc<IndexPipeline>>,
    /// Tag pipeline (for repomap).
    tag_pipeline: Option<Arc<TagPipeline>>,
    /// Epoch for optimistic locking.
    epoch: AtomicI64,
    /// Cancellation token.
    cancel: CancellationToken,
    /// Unified state.
    state: RwLock<UnifiedState>,
}

impl UnifiedCoordinator {
    /// Create a new unified coordinator.
    pub fn new(
        config: RetrievalConfig,
        features: FeatureFlags,
        workdir: PathBuf,
        db: Arc<SqliteStore>,
    ) -> Result<Self> {
        let index_pipeline = if features.search_enabled {
            // Use default strict mode config (init=true, incremental=false)
            let strict_config = IndexStrictModeConfig::default();
            Some(Arc::new(IndexPipeline::new(
                Arc::clone(&db),
                config.clone(),
                workdir.clone(),
                strict_config,
            )))
        } else {
            None
        };

        let tag_pipeline = if features.repomap_enabled {
            let cache = Arc::new(RepoMapCache::new(Arc::clone(&db)));
            // Use default strict mode config
            let strict_config = TagStrictModeConfig::default();
            Some(Arc::new(TagPipeline::new(
                cache,
                workdir.clone(),
                strict_config,
                config.indexing.worker_count,
            )))
        } else {
            None
        };

        Ok(Self {
            config,
            features,
            workdir,
            db,
            index_pipeline,
            tag_pipeline,
            epoch: AtomicI64::new(0),
            cancel: CancellationToken::new(),
            state: RwLock::new(UnifiedState::Uninitialized),
        })
    }

    /// Get the feature flags.
    pub fn features(&self) -> &FeatureFlags {
        &self.features
    }

    /// Get the current epoch.
    pub fn epoch(&self) -> i64 {
        self.epoch.load(Ordering::Acquire)
    }

    /// Increment and get new epoch.
    fn next_epoch(&self) -> i64 {
        self.epoch.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Get the unified state.
    pub async fn state(&self) -> UnifiedState {
        self.state.read().await.clone()
    }

    /// Check if stopped.
    pub fn is_stopped(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// Stop all pipelines.
    pub async fn stop(&self) {
        self.cancel.cancel();
        if let Some(ref pipeline) = self.index_pipeline {
            pipeline.stop().await;
        }
        if let Some(ref pipeline) = self.tag_pipeline {
            pipeline.stop().await;
        }
        tracing::info!("UnifiedCoordinator stopped");
    }

    /// Start worker pools for enabled pipelines.
    pub async fn start_workers(&self) {
        if let Some(ref pipeline) = self.index_pipeline {
            pipeline.start_workers().await;
        }
        if let Some(ref pipeline) = self.tag_pipeline {
            pipeline.start_workers().await;
        }
    }

    /// Get the index pipeline (if enabled).
    pub fn index_pipeline(&self) -> Option<&Arc<IndexPipeline>> {
        self.index_pipeline.as_ref()
    }

    /// Get the tag pipeline (if enabled).
    pub fn tag_pipeline(&self) -> Option<&Arc<TagPipeline>> {
        self.tag_pipeline.as_ref()
    }

    /// Trigger session start: scan files and dispatch to both pipelines.
    pub async fn trigger_session_start(&self) -> Result<SessionStartResult> {
        if !self.features.search_enabled && !self.features.repomap_enabled {
            return Err(crate::error::RetrievalErr::NotEnabled);
        }

        let epoch = self.next_epoch();
        let batch_id = BatchId::new();

        tracing::info!(
            batch_id = %batch_id,
            epoch = epoch,
            search = self.features.search_enabled,
            repomap = self.features.repomap_enabled,
            "SessionStart: beginning file scan"
        );

        // Scan all files
        let walker = FileWalker::new(&self.workdir, self.config.indexing.max_file_size_mb);
        let files: Vec<PathBuf> = walker.walk(&self.workdir)?;
        let file_count = files.len() as i64;

        tracing::info!(
            batch_id = %batch_id,
            files = file_count,
            "SessionStart: scanned files"
        );

        // Handle empty workspace - immediately mark as ready
        if file_count == 0 {
            tracing::info!(
                batch_id = %batch_id,
                "SessionStart: no files to index, marking as ready"
            );

            // Mark pipelines as ready with empty stats
            if let Some(ref pipeline) = self.index_pipeline {
                pipeline.mark_ready(IndexStats::default()).await;
            }
            if let Some(ref pipeline) = self.tag_pipeline {
                pipeline.mark_ready(TagStats::default()).await;
            }

            // Update unified state to ready
            *self.state.write().await = UnifiedState::Ready;

            // Return receivers that complete immediately (batch tracker handles this)
            let index_receiver = if let Some(ref pipeline) = self.index_pipeline {
                Some(pipeline.start_batch(batch_id.clone(), 0).await)
            } else {
                None
            };
            let tag_receiver = if let Some(ref pipeline) = self.tag_pipeline {
                Some(pipeline.start_batch(batch_id.clone(), 0).await)
            } else {
                None
            };

            return Ok(SessionStartResult {
                batch_id,
                file_count,
                index_receiver,
                tag_receiver,
            });
        }

        // Update state to building
        *self.state.write().await = UnifiedState::Building {
            search_building: self.features.search_enabled,
            repomap_building: self.features.repomap_enabled,
        };

        // Dispatch to index pipeline (all events are Changed, processor checks existence)
        let index_receiver = if let Some(ref pipeline) = self.index_pipeline {
            pipeline.mark_building(batch_id.clone()).await;
            let rx = pipeline.start_batch(batch_id.clone(), file_count).await;

            for file in &files {
                let seq = pipeline.assign_seq();
                let event = TrackedEvent::new(
                    WatchEventKind::Changed,
                    Some(batch_id.clone()),
                    seq,
                    generate_trace_id(TriggerSource::SessionStart, epoch),
                );
                pipeline.push_event(file.clone(), event).await;
            }

            Some(rx)
        } else {
            None
        };

        // Dispatch to tag pipeline (all events are Changed, processor checks existence)
        let tag_receiver = if let Some(ref pipeline) = self.tag_pipeline {
            pipeline.mark_building(batch_id.clone()).await;
            let rx = pipeline.start_batch(batch_id.clone(), file_count).await;

            for file in &files {
                let seq = pipeline.assign_seq();
                let event = TrackedEvent::new(
                    TagEventKind::Changed,
                    Some(batch_id.clone()),
                    seq,
                    generate_trace_id(TriggerSource::SessionStart, epoch),
                );
                pipeline.push_event(file.clone(), event).await;
            }

            Some(rx)
        } else {
            None
        };

        Ok(SessionStartResult {
            batch_id,
            file_count,
            index_receiver,
            tag_receiver,
        })
    }

    /// Dispatch file changes to enabled pipelines (for Timer/Watcher).
    pub async fn dispatch_changes(&self, changes: Vec<CoordinatorFileChange>) {
        let epoch = self.next_epoch();

        for change in changes {
            let path = change.path().to_path_buf();

            // Dispatch to index pipeline
            if let Some(ref pipeline) = self.index_pipeline {
                let seq = pipeline.assign_seq();
                let event = TrackedEvent::new(
                    change.to_event_kind(),
                    None,
                    seq,
                    generate_trace_id(TriggerSource::Watcher, epoch),
                );
                pipeline.push_event(path.clone(), event).await;
            }

            // Dispatch to tag pipeline (all events are Changed, processor checks existence)
            if let Some(ref pipeline) = self.tag_pipeline {
                let seq = pipeline.assign_seq();
                let event = TrackedEvent::new(
                    TagEventKind::Changed,
                    None,
                    seq,
                    generate_trace_id(TriggerSource::Watcher, epoch),
                );
                pipeline.push_event(path, event).await;
            }
        }
    }

    /// Mark index pipeline as ready with stats.
    pub async fn mark_index_ready(&self, stats: IndexStats) {
        if let Some(ref pipeline) = self.index_pipeline {
            pipeline.mark_ready(stats).await;
        }
        self.update_unified_state().await;
    }

    /// Mark tag pipeline as ready with stats.
    pub async fn mark_tag_ready(&self, stats: TagStats) {
        if let Some(ref pipeline) = self.tag_pipeline {
            pipeline.mark_ready(stats).await;
        }
        self.update_unified_state().await;
    }

    /// Update unified state based on pipeline states.
    async fn update_unified_state(&self) {
        let index_ready = self
            .index_pipeline
            .as_ref()
            .map(|p| p.is_init_complete())
            .unwrap_or(true);

        let tag_ready = self
            .tag_pipeline
            .as_ref()
            .map(|p| p.is_init_complete())
            .unwrap_or(true);

        let new_state = if index_ready && tag_ready {
            UnifiedState::Ready
        } else {
            UnifiedState::Building {
                search_building: !index_ready && self.features.search_enabled,
                repomap_building: !tag_ready && self.features.repomap_enabled,
            }
        };

        *self.state.write().await = new_state;
    }

    /// Get search readiness.
    pub async fn search_readiness(&self) -> Option<IndexReadiness> {
        if let Some(ref pipeline) = self.index_pipeline {
            Some(pipeline.readiness().await)
        } else {
            None
        }
    }

    /// Get repomap readiness.
    pub async fn repomap_readiness(&self) -> Option<TagReadiness> {
        if let Some(ref pipeline) = self.tag_pipeline {
            Some(pipeline.readiness().await)
        } else {
            None
        }
    }

    /// Check if search is ready.
    pub async fn is_search_ready(&self) -> bool {
        self.index_pipeline
            .as_ref()
            .map(|p| futures::executor::block_on(p.is_ready()))
            .unwrap_or(false)
    }

    /// Check if repomap is ready.
    pub async fn is_repomap_ready(&self) -> bool {
        self.tag_pipeline
            .as_ref()
            .map(|p| futures::executor::block_on(p.is_ready()))
            .unwrap_or(false)
    }

    /// Start periodic timer for freshness checks.
    pub fn start_timer(self: &Arc<Self>, interval: Duration) {
        let coord = Arc::clone(self);
        tokio::spawn(async move {
            // Wait for initial build to complete
            loop {
                if coord.is_stopped() {
                    return;
                }

                let state = coord.state().await;
                if matches!(state, UnifiedState::Ready) {
                    break;
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            tracing::info!(
                interval_secs = interval.as_secs(),
                "Timer started after initial build"
            );

            let mut interval_timer = tokio::time::interval(interval);

            loop {
                tokio::select! {
                    _ = coord.cancel.cancelled() => {
                        tracing::debug!("Timer cancelled");
                        break;
                    }
                    _ = interval_timer.tick() => {
                        // Timer tick - would trigger freshness check
                        // For now, just log
                        tracing::debug!("Timer tick - freshness check would run here");
                    }
                }
            }
        });
    }
}

impl std::fmt::Debug for UnifiedCoordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedCoordinator")
            .field("features", &self.features)
            .field("workdir", &self.workdir)
            .field("epoch", &self.epoch())
            .field("is_stopped", &self.is_stopped())
            .finish()
    }
}

/// Shared unified coordinator.
pub type SharedUnifiedCoordinator = Arc<UnifiedCoordinator>;

/// Generate a trace ID for an event.
fn generate_trace_id(source: TriggerSource, epoch: i64) -> String {
    let prefix = match source {
        TriggerSource::SessionStart => "session",
        TriggerSource::Timer => "timer",
        TriggerSource::Watcher => "watch",
        TriggerSource::Manual => "manual",
    };
    let timestamp = chrono::Utc::now().timestamp_millis();
    format!("{prefix}-{epoch}-{timestamp}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_coordinator(features: FeatureFlags) -> (TempDir, UnifiedCoordinator) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Arc::new(SqliteStore::open(&db_path).unwrap());

        let config = RetrievalConfig::default();

        let coordinator =
            UnifiedCoordinator::new(config, features, dir.path().to_path_buf(), db).unwrap();

        (dir, coordinator)
    }

    #[tokio::test]
    async fn test_coordinator_creation_both_enabled() {
        let features = FeatureFlags {
            search_enabled: true,
            repomap_enabled: true,
        };
        let (_dir, coord) = create_test_coordinator(features).await;

        assert!(coord.index_pipeline().is_some());
        assert!(coord.tag_pipeline().is_some());
        assert!(matches!(coord.state().await, UnifiedState::Uninitialized));
    }

    #[tokio::test]
    async fn test_coordinator_creation_search_only() {
        let features = FeatureFlags {
            search_enabled: true,
            repomap_enabled: false,
        };
        let (_dir, coord) = create_test_coordinator(features).await;

        assert!(coord.index_pipeline().is_some());
        assert!(coord.tag_pipeline().is_none());
    }

    #[tokio::test]
    async fn test_coordinator_creation_repomap_only() {
        let features = FeatureFlags {
            search_enabled: false,
            repomap_enabled: true,
        };
        let (_dir, coord) = create_test_coordinator(features).await;

        assert!(coord.index_pipeline().is_none());
        assert!(coord.tag_pipeline().is_some());
    }

    #[tokio::test]
    async fn test_coordinator_session_start() {
        let features = FeatureFlags::default();
        let (dir, coord) = create_test_coordinator(features).await;

        // Create some test files
        std::fs::write(dir.path().join("test1.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("test2.rs"), "fn foo() {}").unwrap();

        // Start workers
        coord.start_workers().await;

        // Trigger session start
        let result = coord.trigger_session_start().await.unwrap();

        assert!(result.index_receiver.is_some());
        assert!(result.tag_receiver.is_some());
        assert!(result.file_count >= 2);

        // State should be building
        assert!(matches!(coord.state().await, UnifiedState::Building { .. }));
    }

    #[tokio::test]
    async fn test_coordinator_stop() {
        let features = FeatureFlags::default();
        let (_dir, coord) = create_test_coordinator(features).await;

        assert!(!coord.is_stopped());
        coord.stop().await;
        assert!(coord.is_stopped());
    }

    #[tokio::test]
    async fn test_coordinator_epoch() {
        let features = FeatureFlags::default();
        let (_dir, coord) = create_test_coordinator(features).await;

        let e1 = coord.epoch();
        let e2 = coord.next_epoch();
        let e3 = coord.epoch();

        assert_eq!(e2, e1 + 1);
        assert_eq!(e3, e2);
    }

    #[tokio::test]
    async fn test_generate_trace_id() {
        let id1 = generate_trace_id(TriggerSource::SessionStart, 1);
        let id2 = generate_trace_id(TriggerSource::Timer, 2);
        let id3 = generate_trace_id(TriggerSource::Watcher, 3);

        assert!(id1.starts_with("session-1-"));
        assert!(id2.starts_with("timer-2-"));
        assert!(id3.starts_with("watch-3-"));
    }
}
