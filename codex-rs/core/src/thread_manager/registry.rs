//! Keeps thread membership and Guardian publication atomic under one registry lock.
//! Refresh pauses registration during policy loading. Policy loads and session
//! updates happen outside the lock.

use crate::CodexThread;
use crate::ConfigRefreshOutcome;
use crate::config::Config;
use codex_config::CloudConfigBundleBinding;
use codex_config::CloudConfigBundleBindingStatus;
use codex_config::GuardianPolicySnapshot;
use codex_protocol::ThreadId;
use futures::future::BoxFuture;
use futures::future::join_all;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::RwLockMappedWriteGuard;
use tokio::sync::RwLockReadGuard;
use tokio::sync::RwLockWriteGuard;
use tokio::sync::watch;
use tracing::warn;

type LiveThreads = HashMap<ThreadId, Arc<CodexThread>>;

/// Reads the current managed Guardian requirements using the host's config authority.
pub trait GuardianPolicyLoader: Send + Sync {
    fn load_guardian_policy_snapshot(
        &self,
        revision: u64,
    ) -> BoxFuture<'_, std::io::Result<GuardianPolicySnapshot>>;
}

#[derive(Default)]
pub(crate) struct ThreadRegistry {
    pub(crate) state: RwLock<ThreadRegistryState>,
}

#[derive(Default)]
pub(crate) struct ThreadRegistryState {
    pub(crate) threads: LiveThreads,
    last_guardian_revision: u64,
    pub(super) published_guardian_revision: u64,
    guardian_refresh: Option<watch::Receiver<()>>,
}

impl ThreadRegistryState {
    fn reserve_guardian_policy_revision(&mut self) -> std::io::Result<u64> {
        self.last_guardian_revision = self
            .last_guardian_revision
            .checked_add(1)
            .ok_or_else(|| std::io::Error::other("Guardian observation revisions exhausted"))?;
        Ok(self.last_guardian_revision)
    }

    pub(super) fn pending_guardian_refresh(&self) -> Option<watch::Receiver<()>> {
        self.guardian_refresh
            .as_ref()
            .filter(|confirmation| confirmation.has_changed().is_ok())
            .cloned()
    }
}

impl ThreadRegistry {
    pub(crate) async fn read(&self) -> RwLockReadGuard<'_, LiveThreads> {
        RwLockReadGuard::map(self.state.read().await, |state| &state.threads)
    }

    pub(crate) async fn write(&self) -> RwLockMappedWriteGuard<'_, LiveThreads> {
        RwLockWriteGuard::map(self.state.write().await, |state| &mut state.threads)
    }

    pub(super) async fn reserve_guardian_policy_revision(&self) -> std::io::Result<u64> {
        self.state.write().await.reserve_guardian_policy_revision()
    }

    pub(super) async fn prepare_startup_guardian_policy(
        &self,
        config: &mut Config,
        loader: Option<&dyn GuardianPolicyLoader>,
    ) -> std::io::Result<()> {
        let stale_binding = config.guardian_policy_revision != 0
            && config
                .config_layer_stack
                .guardian_policy_binding()
                .is_some_and(|binding| {
                    binding.read().status != CloudConfigBundleBindingStatus::Current
                });
        let refresh_crossed_load =
            self.state.read().await.published_guardian_revision > config.guardian_policy_revision;
        let untracked = config.guardian_policy_revision == 0 && loader.is_some();
        if untracked || stale_binding || refresh_crossed_load {
            let snapshot = self.load_startup_guardian_policy(loader).await?;
            config.apply_guardian_policy(&snapshot)?;
        }
        Ok(())
    }

    pub(super) async fn load_startup_guardian_policy(
        &self,
        loader: Option<&dyn GuardianPolicyLoader>,
    ) -> std::io::Result<GuardianPolicySnapshot> {
        let unavailable = || std::io::Error::other("Guardian policy source changed");
        let loader = loader.ok_or_else(unavailable)?;
        let revision = self.reserve_guardian_policy_revision().await?;
        let snapshot = loader.load_guardian_policy_snapshot(revision).await?;
        if snapshot
            .binding
            .as_ref()
            .is_some_and(|binding| binding.read().status != CloudConfigBundleBindingStatus::Current)
        {
            return Err(unavailable());
        }
        Ok(snapshot)
    }

    pub(super) async fn refresh_guardian_policy(
        &self,
        loader: &dyn GuardianPolicyLoader,
    ) -> std::io::Result<()> {
        // Pause registration before reading. Dropping the sender releases waiters.
        let (registration_pause, revision) = loop {
            let mut wait = {
                let mut state = self.state.write().await;
                if let Some(wait) = state.pending_guardian_refresh() {
                    wait
                } else {
                    let revision = state.reserve_guardian_policy_revision()?;
                    let (sender, receiver) = watch::channel(());
                    state.guardian_refresh = Some(receiver);
                    break (sender, revision);
                }
            };
            let _ = wait.changed().await;
        };
        let mut snapshot = loader.load_guardian_policy_snapshot(revision).await?;
        let pending = {
            let mut state = self.state.write().await;
            let binding = snapshot
                .binding
                .as_ref()
                .map(CloudConfigBundleBinding::read);
            if binding
                .as_ref()
                .is_some_and(|guard| guard.status != CloudConfigBundleBindingStatus::Current)
            {
                return Err(std::io::Error::other(
                    "Guardian policy source changed before publication",
                ));
            }
            // A startup may have reserved a revision while the refresh was reading.
            snapshot.revision = state.reserve_guardian_policy_revision()?;
            state.published_guardian_revision = snapshot.revision;
            state
                .threads
                .values()
                .filter(|thread| !thread.session_source.is_internal())
                .cloned()
                .collect::<Vec<_>>()
        };
        drop(registration_pause);
        // Complete updates after registry publication even if the refresh caller is cancelled.
        let update = tokio::spawn(async move {
            let snapshot = &snapshot;
            let rejections = join_all(pending.into_iter().map(|thread| async move {
                let rejected =
                    thread.refresh_guardian_policy(snapshot).await == ConfigRefreshOutcome::Rejected;
                if rejected {
                    warn!(thread_id = %thread.session.thread_id(), "Guardian policy refresh rejected for thread");
                }
                rejected
            }))
            .await;
            if rejections.into_iter().any(|rejected| rejected) {
                return Err(std::io::Error::other(
                    "Guardian policy refresh rejected by one or more live threads",
                ));
            }
            Ok(())
        });
        update.await?
    }
}
