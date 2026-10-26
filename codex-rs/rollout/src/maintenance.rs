//! Coordinates maintenance jobs that replace local rollout files.
//!
//! Rollout compression and legacy rollout migration both publish by renaming a replacement over an
//! existing rollout path. They must not do that at the same time for one Codex home, so they share
//! this process-scoped, nonblocking file lock.
//!
//! This is separate from per-thread writer locks, which protect live rollout appenders. It is also
//! separate from compression's durable run marker, which throttles how often compression scans.

use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::path::Path;

const ROLLOUT_MAINTENANCE_LOCK: &str = "rollout-maintenance.lock";

#[cfg(test)]
#[path = "maintenance_tests.rs"]
mod tests;

/// Holds exclusive ownership of operations that replace local rollout files.
pub struct RolloutMaintenanceGuard {
    file: File,
}

impl Drop for RolloutMaintenanceGuard {
    fn drop(&mut self) {
        // Release the lock even if a child inherited the file before exec.
        let _ = self.file.unlock();
    }
}

/// Try to exclude rollout compression and migration for one Codex home.
pub fn try_acquire_rollout_maintenance_lock(
    codex_home: &Path,
) -> io::Result<Option<RolloutMaintenanceGuard>> {
    let directory = codex_home.join(".tmp");
    fs::create_dir_all(&directory)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(ROLLOUT_MAINTENANCE_LOCK))?;

    match file.try_lock() {
        Ok(()) => Ok(Some(RolloutMaintenanceGuard { file })),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}
