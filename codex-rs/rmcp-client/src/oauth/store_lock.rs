//! Cross-process serialization for MCP OAuth stores shared by multiple credentials.
//!
//! File and Secrets each keep credentials for multiple MCP servers in one aggregate document.
//! Their lock therefore protects the complete read-modify-write operation. Direct keyring entries
//! are already stored independently per credential and do not use this lock.

use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use codex_utils_home_dir::find_codex_home;

const OAUTH_LOCK_DIR: &str = "mcp-oauth-locks";
const STORE_LOCK_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(60);
const STORE_LOCK_RETRY_SLEEP: Duration = Duration::from_millis(50);
// Tests listen for this event so they prove a contender reached the real WouldBlock branch.
const LOCK_CONTENTION_EVENT_TARGET: &str = "codex_rmcp_client::oauth::store_lock::contention";

/// Marks aggregate-store coordination failures in an [`anyhow::Error`] chain.
///
/// Auto may fall back when the configured keyring backend is unavailable, but it must surface a
/// lock failure. Falling back while another process owns the aggregate-store lock could leave the
/// newer credential in File while a stale Secrets entry remains preferred.
#[derive(Debug, thiserror::Error)]
#[error("failed to acquire MCP OAuth {store} aggregate-store lock")]
pub(super) struct OAuthStoreLockFailure {
    store: &'static str,
}

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

/// Serializes one complete operation on an aggregate OAuth credential store.
pub(super) struct OAuthStoreLock {
    _file: File,
}

impl OAuthStoreLock {
    pub(super) fn acquire(store: OAuthStore) -> Result<Self> {
        let codex_home = find_codex_home().context(OAuthStoreLockFailure {
            store: store.description(),
        })?;
        Self::acquire_in(&codex_home, store, STORE_LOCK_ACQUIRE_TIMEOUT).context(
            OAuthStoreLockFailure {
                store: store.description(),
            },
        )
    }

    pub(super) fn acquire_in(
        codex_home: &Path,
        store: OAuthStore,
        acquire_timeout: Duration,
    ) -> Result<Self> {
        let path = oauth_store_lock_path(codex_home, store);
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
        let mut reported_contention = false;

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
                    if !reported_contention {
                        tracing::debug!(
                            target: LOCK_CONTENTION_EVENT_TARGET,
                            store = store.description(),
                            lock_path = %path.display(),
                            "waiting for another process to finish updating MCP OAuth store state"
                        );
                        reported_contention = true;
                    }
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

fn oauth_store_lock_path(codex_home: &Path, store: OAuthStore) -> PathBuf {
    codex_home.join(OAUTH_LOCK_DIR).join(store.lock_filename())
}
