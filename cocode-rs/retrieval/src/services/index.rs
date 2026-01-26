//! Index management service.
//!
//! Manages the unified indexing pipeline for both search and repomap.
//! Provides operations for building, watching, and querying index status.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::context::RetrievalContext;
use crate::error::Result;
use crate::event_emitter;
use crate::events::RetrievalEvent;
use crate::indexing::FeatureFlags;
use crate::indexing::FileWatcher;
use crate::indexing::IndexManager;
use crate::indexing::IndexProgress;
use crate::indexing::IndexStats;
use crate::indexing::Readiness;
use crate::indexing::RebuildMode;
use crate::indexing::SessionStartResult;
use crate::indexing::SharedUnifiedCoordinator;
use crate::indexing::UnifiedCoordinator;
use crate::indexing::WatchEvent;
use crate::repomap::TagReadiness;

/// Service for index management.
///
/// Manages the unified pipeline for both search (chunking, embeddings)
/// and repomap (tag extraction). Uses `UnifiedCoordinator` internally.
#[derive(Debug)]
pub struct IndexService {
    /// Shared context with config, db, etc.
    ctx: Arc<RetrievalContext>,
    /// Unified coordinator for both search and repomap pipelines.
    coordinator: RwLock<Option<SharedUnifiedCoordinator>>,
}

impl IndexService {
    /// Create a new index service.
    pub fn new(ctx: Arc<RetrievalContext>) -> Self {
        Self {
            ctx,
            coordinator: RwLock::new(None),
        }
    }

    // ========== Coordinator Lifecycle ==========

    /// Get or create the unified coordinator.
    ///
    /// Lazily initializes the coordinator on first access.
    pub async fn coordinator(&self) -> Result<SharedUnifiedCoordinator> {
        // Fast path: check if already initialized
        {
            let guard = self.coordinator.read().await;
            if let Some(ref coord) = *guard {
                return Ok(Arc::clone(coord));
            }
        }

        // Slow path: initialize coordinator
        let mut guard = self.coordinator.write().await;

        // Double-check after acquiring write lock
        if let Some(ref coord) = *guard {
            return Ok(Arc::clone(coord));
        }

        // Determine feature flags from config
        let features = FeatureFlags {
            search_enabled: true,
            repomap_enabled: self.ctx.config().repo_map.is_some(),
        };

        let coord = Arc::new(UnifiedCoordinator::new(
            (*self.ctx.config()).clone(),
            features,
            self.ctx.workspace_root().to_path_buf(),
            self.ctx.db(),
        )?);

        *guard = Some(Arc::clone(&coord));
        Ok(coord)
    }

    /// Start the unified pipeline workers.
    ///
    /// Initializes and starts background workers for processing file events.
    pub async fn start_pipeline(&self) -> Result<()> {
        let coord = self.coordinator().await?;
        coord.start_workers().await;

        // Start timer for periodic freshness checks
        let check_interval = self.ctx.config().indexing.check_interval_secs;
        if check_interval > 0 {
            coord.start_timer(Duration::from_secs(check_interval as u64));
        }

        tracing::info!(
            search = coord.features().search_enabled,
            repomap = coord.features().repomap_enabled,
            "Index pipeline started"
        );
        Ok(())
    }

    /// Stop the unified pipeline.
    ///
    /// Stops all background workers and the periodic timer.
    pub async fn stop_pipeline(&self) {
        if let Ok(coord) = self.coordinator().await {
            coord.stop().await;
        }
    }

    // ========== Session Trigger ==========

    /// Trigger session start.
    ///
    /// Scans all files and dispatches events to both search and repomap pipelines.
    /// Returns receivers for batch completion if you need to wait for indexing.
    pub async fn trigger_session_start(&self) -> Result<SessionStartResult> {
        let coord = self.coordinator().await?;
        coord.trigger_session_start().await
    }

    // ========== Readiness ==========

    /// Get search readiness from the unified pipeline.
    ///
    /// Returns `None` if search is not enabled.
    pub async fn search_readiness(&self) -> Option<Readiness> {
        match self.coordinator().await {
            Ok(coord) => coord.search_readiness().await,
            Err(_) => None,
        }
    }

    /// Get repomap readiness from the unified pipeline.
    ///
    /// Returns `None` if repomap is not enabled.
    pub async fn repomap_readiness(&self) -> Option<TagReadiness> {
        match self.coordinator().await {
            Ok(coord) => coord.repomap_readiness().await,
            Err(_) => None,
        }
    }

    /// Check if the unified pipeline search is ready.
    pub async fn is_search_ready(&self) -> bool {
        matches!(self.search_readiness().await, Some(Readiness::Ready { .. }))
    }

    /// Check if the unified pipeline repomap is ready.
    pub async fn is_repomap_ready(&self) -> bool {
        matches!(
            self.repomap_readiness().await,
            Some(TagReadiness::Ready { .. })
        )
    }

    // ========== Operations ==========

    /// Build or rebuild the index.
    ///
    /// # Arguments
    /// * `mode` - `RebuildMode::Incremental` (default) or `RebuildMode::Clean`
    /// * `cancel` - Cancellation token to abort the operation
    ///
    /// # Returns
    /// A channel receiver that yields `IndexProgress` updates.
    pub async fn build_index(
        &self,
        mode: RebuildMode,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<IndexProgress>> {
        tracing::info!(mode = ?mode, "Starting index build");

        let workspace = self.ctx.workspace_name().to_string();
        let workdir = self.ctx.workspace_root().to_path_buf();
        let config = (*self.ctx.config()).clone();
        let db = self.ctx.db();

        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            let mut manager = IndexManager::new(config, db);

            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Index build cancelled by user");
                    let _ = tx.send(IndexProgress::failed("Cancelled by user")).await;
                }
                result = manager.rebuild(&workspace, &workdir, mode) => {
                    match result {
                        Ok(mut progress_rx) => {
                            while let Some(progress) = progress_rx.recv().await {
                                if tx.send(progress).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Index rebuild failed: {}", e);
                            let _ = tx.send(IndexProgress::failed(e.to_string())).await;
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Get index status and statistics.
    ///
    /// Returns information about the index including file count, chunk count,
    /// and last indexing time.
    pub async fn get_status(&self) -> Result<IndexStats> {
        let workspace = self.ctx.workspace_name().to_string();
        let config = (*self.ctx.config()).clone();
        let db = self.ctx.db();

        let manager = IndexManager::new(config, db);
        manager.get_stats(&workspace).await
    }

    /// Start file watcher for incremental index updates.
    ///
    /// # Arguments
    /// * `cancel` - Cancellation token to stop watching
    ///
    /// # Returns
    /// A channel receiver that yields `WatchEvent` updates.
    pub async fn start_watch(
        &self,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<WatchEvent>> {
        let workspace = self.ctx.workspace_name().to_string();
        let workdir = self.ctx.workspace_root().to_path_buf();
        let config = (*self.ctx.config()).clone();
        let db = self.ctx.db();

        let debounce_ms = config.indexing.watch_debounce_ms.max(0) as u64;
        let watcher = FileWatcher::new(&workdir, debounce_ms)?;

        // Emit watch started event
        event_emitter::emit(RetrievalEvent::WatchStarted {
            workspace: workspace.clone(),
            paths: vec![workdir.display().to_string()],
        });

        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            let mut manager = IndexManager::new(config.clone(), db);

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        tracing::info!("File watcher cancelled");
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        if let Some(events) = watcher.recv_timeout(Duration::from_millis(100)) {
                            for event in &events {
                                let watch_event = WatchEvent {
                                    path: event.path.clone(),
                                    kind: event.kind.clone(),
                                };
                                let _ = tx.send(watch_event).await;
                            }

                            if !events.is_empty() {
                                // Trigger incremental rebuild
                                if let Err(e) = manager
                                    .rebuild(&workspace, &workdir, RebuildMode::Incremental)
                                    .await
                                {
                                    tracing::error!("Incremental rebuild failed: {}", e);
                                }
                            }
                        }
                    }
                }
            }

            event_emitter::emit(RetrievalEvent::WatchStopped { workspace });
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RetrievalConfig;
    use crate::context::RetrievalFeatures;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_index_service_creation() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::with_code_search();
        let ctx = Arc::new(
            RetrievalContext::new(config, features, dir.path().to_path_buf())
                .await
                .unwrap(),
        );

        let service = IndexService::new(ctx);

        // Get coordinator (lazy init)
        let coord = service.coordinator().await.unwrap();

        // Check features
        assert!(coord.features().search_enabled);
        // repomap disabled since repo_map config is None
        assert!(!coord.features().repomap_enabled);

        // Stop
        coord.stop().await;
    }

    #[tokio::test]
    async fn test_session_start() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        // Create test files
        std::fs::write(dir.path().join("test1.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("test2.rs"), "fn foo() {}").unwrap();

        let features = RetrievalFeatures::with_code_search();
        let ctx = Arc::new(
            RetrievalContext::new(config, features, dir.path().to_path_buf())
                .await
                .unwrap(),
        );

        let service = IndexService::new(ctx);

        // Start pipeline
        service.start_pipeline().await.unwrap();

        // Trigger session start
        let result = service.trigger_session_start().await.unwrap();

        // Should have scanned files
        assert!(result.file_count >= 2);
        assert!(result.index_receiver.is_some());

        // Cleanup
        service.stop_pipeline().await;
    }

    #[tokio::test]
    async fn test_readiness() {
        let dir = TempDir::new().unwrap();
        let mut config = RetrievalConfig::default();
        config.data_dir = dir.path().to_path_buf();

        let features = RetrievalFeatures::with_code_search();
        let ctx = Arc::new(
            RetrievalContext::new(config, features, dir.path().to_path_buf())
                .await
                .unwrap(),
        );

        let service = IndexService::new(ctx);

        // Before initialization, readiness check should work
        let readiness = service.search_readiness().await;
        assert!(readiness.is_some());

        // Cleanup
        service.stop_pipeline().await;
    }
}
