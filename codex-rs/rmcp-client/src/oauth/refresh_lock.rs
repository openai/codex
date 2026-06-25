//! Cross-process serialization for one MCP OAuth credential's refresh transaction.
//!
//! The guard is intentionally acquired before the authoritative credential reread and retained
//! through provider refresh and persistence. This prevents two processes from replaying the same
//! rotating refresh token or observing a partially persisted transaction.

use anyhow::Context;
use anyhow::Result;
use codex_utils_home_dir::find_codex_home;
use sha2::Digest;
use sha2::Sha256;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use tokio::time::timeout;

const REFRESH_LOCK_DIR: &str = "mcp-oauth-refresh-locks";
const REFRESH_LOCK_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 60);
const REFRESH_LOCK_RETRY_SLEEP: Duration = Duration::from_millis(/*millis*/ 50);
// Keep this internal target stable so diagnostics and cross-process tests can distinguish actual
// WouldBlock contention from a contender that merely started late and observed persisted tokens.
const LOCK_CONTENTION_EVENT_TARGET: &str = "codex_rmcp_client::oauth::refresh_lock::contention";

pub(super) struct RefreshCredentialLock {
    _file: File,
}

impl RefreshCredentialLock {
    pub(super) async fn acquire_for_server(server_name: &str, url: &str) -> Result<Self> {
        let key = super::compute_store_key(server_name, url)?;
        Self::acquire(&key)
            .await
            .with_context(|| format!("failed to acquire OAuth credential lock for {server_name}"))
    }

    pub(super) async fn acquire(store_key: &str) -> Result<Self> {
        Self::acquire_with_timeout(store_key, REFRESH_LOCK_ACQUIRE_TIMEOUT).await
    }

    pub(super) async fn acquire_with_timeout(
        store_key: &str,
        acquire_timeout: Duration,
    ) -> Result<Self> {
        let codex_home = find_codex_home()?;
        Self::acquire_in(&codex_home, store_key, acquire_timeout).await
    }

    async fn acquire_in(
        codex_home: &Path,
        store_key: &str,
        acquire_timeout: Duration,
    ) -> Result<Self> {
        let path = refresh_lock_path(codex_home, store_key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("failed to open OAuth refresh lock {}", path.display()))?;

        // Bound every contender, but keep the acquired lock for the full provider request and
        // persistence transaction. Releasing it while awaiting the provider would allow concurrent
        // use of a rotating refresh token.
        let mut reported_contention = false;
        match timeout(acquire_timeout, async {
            loop {
                match file.try_lock() {
                    Ok(()) => return Ok(()),
                    Err(std::fs::TryLockError::WouldBlock) => {
                        if !reported_contention {
                            tracing::debug!(
                                target: LOCK_CONTENTION_EVENT_TARGET,
                                lock_path = %path.display(),
                                "waiting for another process to finish refreshing MCP OAuth credentials"
                            );
                            reported_contention = true;
                        }
                        sleep(REFRESH_LOCK_RETRY_SLEEP).await;
                    }
                    Err(error) => return Err(std::io::Error::from(error)),
                }
            }
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                return Err(error).with_context(|| {
                    format!("failed to lock OAuth refresh lock {}", path.display())
                });
            }
            Err(_) => anyhow::bail!(
                "timed out after {acquire_timeout:?} waiting for OAuth refresh lock {}",
                path.display()
            ),
        }

        Ok(Self { _file: file })
    }
}

fn refresh_lock_path(codex_home: &Path, store_key: &str) -> PathBuf {
    // Credential coordination is deliberately scoped to the active CODEX_HOME, alongside File
    // and Secrets state. Coordinating the process-global Direct keyring across distinct homes
    // would require a separately defined global lock namespace and is outside this transaction.
    // TODO(stevenlee): define a safe per-user, cross-platform rendezvous before extending Direct
    // keyring coordination across distinct CODEX_HOME values.
    let mut hasher = Sha256::new();
    hasher.update(store_key.as_bytes());
    let digest = hasher.finalize();
    codex_home
        .join(REFRESH_LOCK_DIR)
        .join(format!("{digest:x}.lock"))
}

#[cfg(test)]
#[path = "refresh_lock_tests.rs"]
mod tests;
