//! Cross-process exclusion between memory imports and consolidation.
//! Lock files live beside memory roots and must not be removed or replaced.

use std::fs::File;
use std::fs::OpenOptions;
use std::fs::TryLockError;
use std::io;
use std::path::Path;

/// Keeps a memory workspace exclusively owned until all writers have stopped.
pub struct MemoryWorkspaceLock {
    _file: File,
}

impl MemoryWorkspaceLock {
    /// Returns `WouldBlock` when another import or consolidation owns this root.
    pub fn try_acquire(memory_root: &Path) -> io::Result<Self> {
        // Keep this outside the workspace: baseline resets and memory resets must
        // not replace the inode while another process still owns its lock.
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(memory_root.with_extension("lock"))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "memory workspace is busy; retry after consolidation or another import finishes",
            )),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }
}
