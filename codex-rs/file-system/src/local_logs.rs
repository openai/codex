//! Creates the configured diagnostic directory and retains a shared directory lock on Unix.
//! The caller selects the path. This module does not search for storage.

use std::io;
use std::path::Path;
use std::path::PathBuf;

/// A diagnostic directory with a Unix cleanup lock retained by all clones.
#[derive(Clone, Debug)]
#[must_use = "keep the guard until all log connections close"]
pub struct LocalLogsDir {
    path: PathBuf,
    #[cfg(unix)]
    _cleanup_lock: std::sync::Arc<std::fs::File>,
}

impl LocalLogsDir {
    /// Create the selected directory. Keep the guard until all log connections close.
    /// On Unix, newly created directories start with mode 0700. Existing permissions
    /// are preserved, including on user-created directories or after the user changes
    /// the mode. We do not enforce or restore private permissions on later calls.
    pub fn new(path: &Path) -> io::Result<Self> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;

            // Set private permissions only at creation; preserve user-chosen modes afterward.
            builder.mode(0o700);
        }
        builder.create(path)?;
        // the main reason to use the diagnostic is to redirect the logs to the tmp/ or /dev/shm (tmpfs)
        // so on unix system we have to ensure that the directory is not going to be cleaned via shared lock
        #[cfg(unix)]
        let directory = {
            use std::os::unix::fs::MetadataExt;

            let directory = std::fs::File::open(path)?;
            directory.try_lock_shared()?;
            let locked = directory.metadata()?;
            let current = std::fs::metadata(path)?;
            if !locked.is_dir() || (current.dev(), current.ino()) != (locked.dev(), locked.ino()) {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "log directory changed during initialization",
                ));
            }
            std::sync::Arc::new(directory)
        };

        Ok(Self {
            path: path.to_path_buf(),
            #[cfg(unix)]
            _cleanup_lock: directory,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
