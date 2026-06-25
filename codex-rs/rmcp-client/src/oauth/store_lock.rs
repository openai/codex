//! Cross-process serialization for MCP OAuth stores shared by multiple credentials.
//!
//! The File and Secrets backends each store a map containing entries for multiple MCP servers.
//! Their lock therefore protects the complete read-modify-write operation, independently of the
//! per-credential refresh transaction lock in `refresh_lock`.

use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use codex_utils_home_dir::find_codex_home;

const OAUTH_STORE_LOCK_DIR: &str = "mcp-oauth-refresh-locks";
const STORE_LOCK_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(60);
const STORE_LOCK_RETRY_SLEEP: Duration = Duration::from_millis(50);

#[derive(Clone, Copy)]
pub(super) enum OAuthStore {
    File,
    Secrets,
}

impl OAuthStore {
    fn lock_filename(self) -> &'static str {
        match self {
            Self::File => "file-store.lock",
            Self::Secrets => "secrets-store.lock",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::File => "fallback file",
            Self::Secrets => "encrypted secrets",
        }
    }
}

/// Serializes access to stores that aggregate credentials for multiple MCP servers.
///
/// A per-credential transaction lock may be acquired before this lock. Store operations must not
/// acquire a credential lock, and cross-store cleanup must happen after releasing the first store
/// lock. This ordering prevents deadlocks while keeping each aggregate read-modify-write atomic.
pub(super) struct OAuthStoreLock {
    _file: File,
}

impl OAuthStoreLock {
    pub(super) fn acquire(store: OAuthStore) -> Result<Self> {
        Self::acquire_with_timeout(store, STORE_LOCK_ACQUIRE_TIMEOUT)
    }

    pub(super) fn acquire_with_timeout(
        store: OAuthStore,
        acquire_timeout: Duration,
    ) -> Result<Self> {
        let path = oauth_store_lock_path(store)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| {
                format!(
                    "failed to open MCP OAuth {} store lock {}",
                    store.description(),
                    path.display()
                )
            })?;
        let started = Instant::now();

        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(std::fs::TryLockError::WouldBlock) if started.elapsed() >= acquire_timeout => {
                    anyhow::bail!(
                        "timed out after {acquire_timeout:?} waiting for MCP OAuth {} store lock {}",
                        store.description(),
                        path.display()
                    );
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    std::thread::sleep(STORE_LOCK_RETRY_SLEEP.min(acquire_timeout));
                }
                Err(error) => {
                    return Err(std::io::Error::from(error)).with_context(|| {
                        format!(
                            "failed to lock MCP OAuth {} store lock {}",
                            store.description(),
                            path.display()
                        )
                    });
                }
            }
        }
    }
}

fn oauth_store_lock_path(store: OAuthStore) -> Result<PathBuf> {
    Ok(find_codex_home()?
        .join(OAUTH_STORE_LOCK_DIR)
        .join(store.lock_filename())
        .to_path_buf())
}
