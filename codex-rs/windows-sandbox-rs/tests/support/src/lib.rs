//! Coordinates tests that share the runner's Windows sandbox accounts across processes.

#![cfg(windows)]

use std::fs::File;
use std::fs::OpenOptions;
use std::io;

/// Serializes tests that provision or use the runner's shared Windows sandbox accounts.
/// Hold this from before setup until all sandbox commands and cleanup have finished.
#[must_use = "the account lock is released when the guard is dropped"]
pub struct WindowsSandboxAccountTestGuard {
    _file: File,
}

impl WindowsSandboxAccountTestGuard {
    /// Acquires the same file lock across all test processes run by this Windows user.
    pub fn acquire() -> io::Result<Self> {
        let directory = dirs::data_local_dir().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Windows local application data directory is unavailable",
            )
        })?;
        // Bazel uses per-test temporary directories, so the lock lives in the runner user's
        // Windows known folder instead. It is separate from the production setup lock.
        let path = directory.join("codex-windows-sandbox-integration-test.lock");
        // Leave the file in place: deleting it would let processes lock different files at this path.
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("open Windows sandbox test lock: {error}"),
                )
            })?;
        file.lock().map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("lock Windows sandbox test accounts: {error}"),
            )
        })?;
        Ok(Self { _file: file })
    }
}
