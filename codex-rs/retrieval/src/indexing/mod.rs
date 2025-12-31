//! Indexing module.
//!
//! Handles file traversal, change detection, and index management.

pub mod change_detector;
pub mod checkpoint;
pub mod lock;
pub mod manager;
pub mod progress;
pub mod walker;
pub mod watcher;

pub use change_detector::ChangeDetector;
pub use change_detector::ChangeStatus;
pub use change_detector::FileChange;
pub use checkpoint::Checkpoint;
pub use checkpoint::CheckpointState;
pub use checkpoint::IndexPhase;
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
