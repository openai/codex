//! File-level index locks.
//!
//! Prevents the same file from being processed concurrently by multiple workers.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::sync::OwnedMutexGuard;
use tokio::sync::RwLock;

/// File-level index locks.
///
/// Prevents the same file from being processed concurrently by multiple workers.
/// Each file has its own lock that must be acquired before processing.
pub struct FileIndexLocks {
    /// filepath -> Mutex
    locks: RwLock<HashMap<PathBuf, Arc<Mutex<()>>>>,
}

impl FileIndexLocks {
    /// Create a new file index locks manager.
    pub fn new() -> Self {
        Self {
            locks: RwLock::new(HashMap::new()),
        }
    }

    /// Try to acquire a lock for a file (non-blocking).
    ///
    /// Returns `Some(FileIndexGuard)` if the lock was acquired,
    /// `None` if the file is already locked by another worker.
    pub async fn try_lock(&self, path: &Path) -> Option<FileIndexGuard> {
        // Get or create the lock for this path
        let lock = {
            let mut locks = self.locks.write().await;
            locks
                .entry(path.to_path_buf())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        // Try to acquire the lock (non-blocking) - use try_lock_owned for 'static lifetime
        match lock.try_lock_owned() {
            Ok(guard) => Some(FileIndexGuard {
                _guard: guard,
                path: path.to_path_buf(),
            }),
            Err(_) => None, // Already locked
        }
    }

    /// Acquire a lock for a file (blocking).
    ///
    /// Waits until the lock is available.
    pub async fn lock(&self, path: &Path) -> FileIndexGuard {
        // Get or create the lock for this path
        let lock = {
            let mut locks = self.locks.write().await;
            locks
                .entry(path.to_path_buf())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        // Acquire the lock (blocking) - use lock_owned for 'static lifetime
        let guard = lock.lock_owned().await;
        FileIndexGuard {
            _guard: guard,
            path: path.to_path_buf(),
        }
    }

    /// Clean up a lock that is no longer needed.
    ///
    /// Only removes the lock if it's not currently held by any guard.
    pub async fn cleanup(&self, path: &Path) {
        let mut locks = self.locks.write().await;
        if let Some(lock) = locks.get(path) {
            // Only clean up if no one else is holding a reference
            // strong_count == 1 means only the HashMap holds a reference
            if Arc::strong_count(lock) == 1 {
                locks.remove(path);
            }
        }
    }

    /// Get the number of currently tracked locks.
    pub async fn len(&self) -> usize {
        self.locks.read().await.len()
    }

    /// Check if there are no tracked locks.
    pub async fn is_empty(&self) -> bool {
        self.locks.read().await.is_empty()
    }

    /// Clean up all locks that are not currently held.
    pub async fn cleanup_all(&self) {
        let mut locks = self.locks.write().await;
        locks.retain(|_, lock| Arc::strong_count(lock) > 1);
    }
}

impl Default for FileIndexLocks {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard for a file index lock.
///
/// The lock is automatically released when the guard is dropped.
pub struct FileIndexGuard {
    /// The underlying owned mutex guard (owns the Arc, has 'static lifetime)
    _guard: OwnedMutexGuard<()>,
    /// Path of the locked file (for debugging)
    path: PathBuf,
}

impl FileIndexGuard {
    /// Get the path of the locked file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl std::fmt::Debug for FileIndexGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileIndexGuard")
            .field("path", &self.path)
            .finish()
    }
}

/// Shared file locks wrapped in Arc for use across threads.
pub type SharedFileLocks = Arc<FileIndexLocks>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_try_lock_success() {
        let locks = FileIndexLocks::new();
        let path = PathBuf::from("test.rs");

        let guard = locks.try_lock(&path).await;
        assert!(guard.is_some());
    }

    #[tokio::test]
    async fn test_try_lock_conflict() {
        let locks = FileIndexLocks::new();
        let path = PathBuf::from("test.rs");

        // First lock succeeds
        let _guard1 = locks.try_lock(&path).await.unwrap();

        // Second lock fails (same file)
        let guard2 = locks.try_lock(&path).await;
        assert!(guard2.is_none());
    }

    #[tokio::test]
    async fn test_different_files() {
        let locks = FileIndexLocks::new();
        let path1 = PathBuf::from("file1.rs");
        let path2 = PathBuf::from("file2.rs");

        // Both locks should succeed (different files)
        let guard1 = locks.try_lock(&path1).await;
        let guard2 = locks.try_lock(&path2).await;

        assert!(guard1.is_some());
        assert!(guard2.is_some());
    }

    #[tokio::test]
    async fn test_lock_release() {
        let locks = FileIndexLocks::new();
        let path = PathBuf::from("test.rs");

        // Acquire and release lock
        {
            let _guard = locks.try_lock(&path).await.unwrap();
            // guard is dropped here
        }

        // Should be able to acquire again
        let guard = locks.try_lock(&path).await;
        assert!(guard.is_some());
    }

    #[tokio::test]
    async fn test_cleanup() {
        let locks = FileIndexLocks::new();
        let path = PathBuf::from("test.rs");

        // Acquire and release lock
        {
            let _guard = locks.try_lock(&path).await.unwrap();
        }

        // Clean up the lock
        locks.cleanup(&path).await;

        // Lock should be removed
        assert!(locks.is_empty().await);
    }

    #[tokio::test]
    async fn test_cleanup_while_locked() {
        let locks = FileIndexLocks::new();
        let path = PathBuf::from("test.rs");

        // Acquire lock
        let _guard = locks.try_lock(&path).await.unwrap();

        // Try to clean up (should not remove because lock is held)
        locks.cleanup(&path).await;

        // Lock should still be tracked
        assert_eq!(locks.len().await, 1);
    }

    #[tokio::test]
    async fn test_blocking_lock() {
        let locks = Arc::new(FileIndexLocks::new());
        let path = PathBuf::from("test.rs");

        // Acquire lock in background
        let locks_clone = locks.clone();
        let path_clone = path.clone();
        let handle = tokio::spawn(async move {
            let _guard = locks_clone.lock(&path_clone).await;
            // Hold the lock briefly
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        });

        // Give the background task time to acquire the lock
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Try to acquire (should fail)
        let guard = locks.try_lock(&path).await;
        assert!(guard.is_none());

        // Wait for background task to complete
        handle.await.unwrap();

        // Now should be able to acquire
        let guard = locks.try_lock(&path).await;
        assert!(guard.is_some());
    }
}
