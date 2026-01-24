//! Indexing module.
//!
//! Handles file traversal, change detection, and index management.
//!
//! ## Architecture
//!
//! The indexing system uses a unified event pipeline:
//!
//! ```text
//! TriggerSource (SessionStart/Timer/Watcher)
//!         │
//!         ▼
//!    EventQueue (with deduplication)
//!         │
//!         ▼
//!    WorkerPool (parallel processing)
//!         │
//!         ├─► BatchTracker (for SessionStart batches)
//!         └─► LagTracker (watermark-based lag detection)
//! ```

pub mod batch_tracker;
pub mod change_detector;
pub mod checkpoint;
pub mod coordinator;
pub mod event_queue;
pub mod file_filter;
pub mod file_locks;
pub mod index_pipeline;
pub mod lag_tracker;
pub mod lock;
pub mod manager;
pub mod pipeline_common;
pub mod progress;
pub mod tags;
pub mod unified_coordinator;
pub mod walker;
pub mod watcher;
pub mod worker_pool;

pub use change_detector::ChangeDetector;
pub use change_detector::ChangeStatus;
pub use change_detector::FileChange;
pub use checkpoint::Checkpoint;
pub use checkpoint::CheckpointState;
pub use checkpoint::IndexPhase;
pub use coordinator::CoordinatorFileChange;
pub use coordinator::FreshnessResult;
pub use coordinator::IndexCoordinator;
pub use coordinator::IndexState;
pub use coordinator::SharedCoordinator;
pub use coordinator::StaleReason;
pub use coordinator::TriggerSource;
pub use event_queue::EventQueue;
pub use event_queue::MergeFn;
pub use event_queue::SharedEventQueue;
pub use event_queue::SharedTagEventQueue;
pub use event_queue::TagEventKind;
pub use event_queue::TagEventQueue;
pub use event_queue::TrackedEvent;
pub use event_queue::WatchEventQueue;
pub use event_queue::new_tag_event_queue;
pub use event_queue::new_watch_event_queue;
pub use event_queue::tag_event_merge;
pub use event_queue::watch_event_merge;
pub use file_filter::FileFilter;
pub use file_filter::FilterSummary;
pub use file_locks::FileIndexGuard;
pub use file_locks::FileIndexLocks;
pub use file_locks::SharedFileLocks;
pub use lock::IndexLockGuard;
pub use manager::IndexManager;
pub use manager::IndexStats;
pub use manager::RebuildMode;
pub use progress::IndexProgress;
pub use progress::IndexStatus;
pub use walker::FileWalker;
pub use watcher::FileWatcher;
pub use watcher::WatchEvent;
pub use watcher::WatchEventKind;

// New modules for unified event pipeline
pub use batch_tracker::BatchId;
pub use batch_tracker::BatchResult;
pub use batch_tracker::BatchTracker;
pub use batch_tracker::SharedBatchTracker;
pub use index_pipeline::IndexEventProcessor;
pub use index_pipeline::IndexPipeline;
pub use index_pipeline::IndexWorkerPool;
pub use index_pipeline::Readiness;
pub use index_pipeline::SharedIndexPipeline;

// Common pipeline types (shared between IndexPipeline and TagPipeline)
pub use lag_tracker::LagInfo;
pub use lag_tracker::LagTimeoutError;
pub use lag_tracker::LagTracker;
pub use lag_tracker::SharedLagTracker;
pub use pipeline_common::PipelineReadiness;
pub use pipeline_common::PipelineState;
pub use pipeline_common::StrictModeConfig;
pub use pipeline_common::compute_readiness;
pub use pipeline_common::now_timestamp;
pub use unified_coordinator::FeatureFlags;
pub use unified_coordinator::SessionStartResult;
pub use unified_coordinator::SharedUnifiedCoordinator;
pub use unified_coordinator::UnifiedCoordinator;
pub use unified_coordinator::UnifiedState;
pub use worker_pool::EventProcessor;
pub use worker_pool::SharedWorkerPool;
pub use worker_pool::WorkerPool;
pub use worker_pool::WorkerPoolConfig;

// Tag pipeline exports (for RepoMap)
pub use tags::SharedTagPipeline;
pub use tags::TagEventProcessor;
pub use tags::TagPipeline;
pub use tags::TagPipelineState;
pub use tags::TagReadiness;
pub use tags::TagStats;
pub use tags::TagStrictModeConfig;
pub use tags::TagWorkerPool;
