//! Cross-process serialization for MCP OAuth stores shared by multiple credentials.
//!
//! File and Secrets each keep credentials for multiple MCP servers in one aggregate document.
//! Their lock therefore protects the complete read-modify-write operation. Direct keyring entries
//! are already stored independently per credential and do not use this lock.

use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use codex_utils_home_dir::find_codex_home;

const OAUTH_LOCK_DIR: &str = "mcp-oauth-locks";
const STORE_LOCK_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(60);
const STORE_LOCK_RETRY_SLEEP: Duration = Duration::from_millis(50);
// Tests listen for this event so they prove a contender reached the real WouldBlock branch.
const LOCK_CONTENTION_EVENT_TARGET: &str = "codex_rmcp_client::oauth::store_lock::contention";

#[derive(Clone, Copy, Debug)]
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
}

impl std::fmt::Display for OAuthStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File => f.write_str("fallback file"),
            Self::Secrets => f.write_str("encrypted secrets"),
        }
    }
}

/// Serializes one complete operation on an aggregate OAuth credential store.
pub(super) struct OAuthStoreLock {
    _file: File,
}

impl OAuthStoreLock {
    pub(super) fn acquire(store: OAuthStore) -> Result<Self> {
        // This lock intentionally follows the existing local File/Secrets credential-store
        // authority. Those stores are CODEX_HOME-backed today: if CODEX_HOME is unset they use
        // the default home (`~/.codex`), and if an embedder has no local home/filesystem authority
        // those stores already cannot operate. A future provider-backed credential store should
        // provide its own matching lock authority instead of using this local path.
        let codex_home = find_codex_home()
            .map_err(|source| OAuthStoreLockFailure::CodexHome { store, source })?;
        Self::acquire_in(&codex_home, store, STORE_LOCK_ACQUIRE_TIMEOUT)
    }

    pub(super) fn acquire_in(
        codex_home: &Path,
        store: OAuthStore,
        acquire_timeout: Duration,
    ) -> Result<Self> {
        let path = oauth_store_lock_path(codex_home, store);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| OAuthStoreLockFailure::CreateDir {
                store,
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|source| OAuthStoreLockFailure::Open {
                store,
                path: path.clone(),
                source,
            })?;
        let started = Instant::now();
        let mut reported_contention = false;

        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(std::fs::TryLockError::WouldBlock) if started.elapsed() >= acquire_timeout => {
                    return Err(OAuthStoreLockFailure::Timeout {
                        store,
                        path,
                        acquire_timeout,
                    }
                    .into());
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    if !reported_contention {
                        tracing::debug!(
                            target: LOCK_CONTENTION_EVENT_TARGET,
                            store = %store,
                            lock_path = %path.display(),
                            "waiting for another process to finish updating MCP OAuth store state"
                        );
                        reported_contention = true;
                    }
                    std::thread::sleep(STORE_LOCK_RETRY_SLEEP.min(acquire_timeout));
                }
                Err(error) => {
                    return Err(OAuthStoreLockFailure::Lock {
                        store,
                        path,
                        source: io::Error::from(error),
                    }
                    .into());
                }
            }
        }
    }
}

/// Marks aggregate-store coordination failures in an [`anyhow::Error`] chain.
///
/// Auto may fall back when the configured keyring backend is unavailable, but it must surface a
/// lock failure. Falling back while another process owns the aggregate-store lock could leave the
/// newer credential in File while a stale Secrets entry remains preferred.
#[derive(Debug, thiserror::Error)]
pub(super) enum OAuthStoreLockFailure {
    #[error("failed to resolve CODEX_HOME for MCP OAuth {store} aggregate-store lock")]
    CodexHome {
        store: OAuthStore,
        #[source]
        source: io::Error,
    },
    #[error("failed to create MCP OAuth {store} aggregate-store lock directory {}", path.display())]
    CreateDir {
        store: OAuthStore,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to open MCP OAuth {store} aggregate-store lock {}", path.display())]
    Open {
        store: OAuthStore,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "timed out after {acquire_timeout:?} waiting for MCP OAuth {store} aggregate-store lock {}",
        path.display()
    )]
    Timeout {
        store: OAuthStore,
        path: PathBuf,
        acquire_timeout: Duration,
    },
    #[error("failed to lock MCP OAuth {store} aggregate-store lock {}", path.display())]
    Lock {
        store: OAuthStore,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

fn oauth_store_lock_path(codex_home: &Path, store: OAuthStore) -> PathBuf {
    codex_home.join(OAUTH_LOCK_DIR).join(store.lock_filename())
}

#[cfg(test)]
#[path = "tests/store_lock_tests.rs"]
mod tests;
