//! Defers diagnostic directory creation and the Unix cleanup lock until initialization.
//! The caller selects the path. This module does not search for storage.

use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::OnceCell;

/// A diagnostic directory whose clones share deferred setup and a Unix cleanup lock.
#[derive(Clone, Debug)]
#[must_use = "keep the guard until all log connections close"]
pub struct LocalLogsDir {
    path: PathBuf,
    initialized: Arc<OnceCell<LocalLogsDirLock>>,
}

#[derive(Debug)]
struct LocalLogsDirLock {
    #[cfg(unix)]
    _cleanup_lock: std::fs::File,
}

impl LocalLogsDir {
    /// Select a directory without accessing the filesystem.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            initialized: Arc::new(OnceCell::new()),
        }
    }

    /// Create the directory and acquire its cleanup lock once. Failed attempts remain retryable.
    /// Keep a clone alive until all log connections close.
    /// On Unix, newly created directories start with mode 0700. Existing permissions
    /// are preserved, including on user-created directories or after the user changes
    /// the mode. We do not enforce or restore private permissions on later calls.
    pub async fn initialize(&self) -> io::Result<()> {
        let _ = self
            .initialized
            .get_or_try_init(|| async {
                let path = self.path.clone();
                tokio::task::spawn_blocking(move || {
                    let mut builder = std::fs::DirBuilder::new();
                    builder.recursive(true);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::DirBuilderExt;

                        // Set private permissions only at creation; preserve existing permissions.
                        builder.mode(0o700);
                    }
                    builder.create(&path)?;
                    // Protect temporary log directories from cleanup while connections use them.
                    #[cfg(unix)]
                    let directory = {
                        use std::os::unix::fs::MetadataExt;

                        let directory = std::fs::File::open(&path)?;
                        directory.try_lock_shared()?;
                        let locked = directory.metadata()?;
                        let current = std::fs::metadata(&path)?;
                        if !locked.is_dir()
                            || (current.dev(), current.ino()) != (locked.dev(), locked.ino())
                        {
                            return Err(io::Error::new(
                                io::ErrorKind::NotFound,
                                "log directory changed during initialization",
                            ));
                        }
                        directory
                    };

                    Ok::<_, io::Error>(LocalLogsDirLock {
                        #[cfg(unix)]
                        _cleanup_lock: directory,
                    })
                })
                .await
                .map_err(io::Error::other)?
            })
            .await?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
