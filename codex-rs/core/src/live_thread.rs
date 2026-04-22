use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_thread_store::AppendThreadItemsParams;
use codex_thread_store::CreateThreadParams;
use codex_thread_store::LoadThreadHistoryParams;
use codex_thread_store::LocalThreadStore;
use codex_thread_store::ResumeThreadParams;
use codex_thread_store::StoredThreadHistory;
use codex_thread_store::ThreadMetadataPatch;
use codex_thread_store::ThreadStore;
use codex_thread_store::ThreadStoreResult;
use codex_thread_store::UpdateThreadMetadataParams;
use tracing::warn;

/// Session-owned handle for the active thread's persistence lifecycle.
///
/// `LiveThread` keeps lifecycle decisions in core while delegating storage details to
/// [`ThreadStore`]. Local stores may use a rollout file internally and remote stores may use a
/// service, but session code should only need this handle for the active thread.
#[derive(Clone)]
pub(crate) struct LiveThread {
    thread_id: ThreadId,
    thread_store: Arc<dyn ThreadStore>,
}

/// Owns a live thread while session initialization is still fallible.
///
/// If initialization returns early after persistence has been opened, dropping this guard discards
/// the live writer without forcing lazy in-memory state to become durable. Call [`commit`] once the
/// session owns the live thread for normal operation.
pub(crate) struct LiveThreadInitGuard {
    live_thread: Option<LiveThread>,
}

impl LiveThreadInitGuard {
    pub(crate) fn new(live_thread: Option<LiveThread>) -> Self {
        Self { live_thread }
    }

    pub(crate) fn as_ref(&self) -> Option<&LiveThread> {
        self.live_thread.as_ref()
    }

    pub(crate) fn commit(&mut self) {
        self.live_thread = None;
    }

    pub(crate) async fn discard(&mut self) {
        let Some(live_thread) = self.live_thread.take() else {
            return;
        };
        if let Err(err) = live_thread.discard().await {
            warn!("failed to discard thread persistence for failed session init: {err}");
        }
    }
}

impl Drop for LiveThreadInitGuard {
    fn drop(&mut self) {
        let Some(live_thread) = self.live_thread.take() else {
            return;
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            warn!("failed to discard thread persistence for failed session init: no Tokio runtime");
            return;
        };
        handle.spawn(async move {
            if let Err(err) = live_thread.discard().await {
                warn!("failed to discard thread persistence for failed session init: {err}");
            }
        });
    }
}

impl LiveThread {
    pub(crate) async fn create(
        thread_store: Arc<dyn ThreadStore>,
        params: CreateThreadParams,
    ) -> ThreadStoreResult<Self> {
        let thread_id = params.thread_id;
        thread_store.create_thread(params).await?;
        Ok(Self {
            thread_id,
            thread_store,
        })
    }

    pub(crate) async fn resume(
        thread_store: Arc<dyn ThreadStore>,
        params: ResumeThreadParams,
    ) -> ThreadStoreResult<Self> {
        let thread_id = params.thread_id;
        thread_store.resume_thread(params).await?;
        Ok(Self {
            thread_id,
            thread_store,
        })
    }

    pub(crate) async fn append_items(&self, items: &[RolloutItem]) -> ThreadStoreResult<()> {
        self.thread_store
            .append_items(AppendThreadItemsParams {
                thread_id: self.thread_id,
                items: items.to_vec(),
            })
            .await
    }

    pub(crate) async fn persist(&self) -> ThreadStoreResult<()> {
        self.thread_store.persist_thread(self.thread_id).await
    }

    pub(crate) async fn flush(&self) -> ThreadStoreResult<()> {
        self.thread_store.flush_thread(self.thread_id).await
    }

    pub(crate) async fn shutdown(&self) -> ThreadStoreResult<()> {
        self.thread_store.shutdown_thread(self.thread_id).await
    }

    pub(crate) async fn discard(&self) -> ThreadStoreResult<()> {
        self.thread_store.discard_thread(self.thread_id).await
    }

    pub(crate) async fn load_history(
        &self,
        include_archived: bool,
    ) -> ThreadStoreResult<StoredThreadHistory> {
        self.thread_store
            .load_history(LoadThreadHistoryParams {
                thread_id: self.thread_id,
                include_archived,
            })
            .await
    }

    pub(crate) async fn update_memory_mode(
        &self,
        mode: ThreadMemoryMode,
        include_archived: bool,
    ) -> ThreadStoreResult<()> {
        self.thread_store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id: self.thread_id,
                patch: ThreadMetadataPatch {
                    memory_mode: Some(mode),
                    ..Default::default()
                },
                include_archived,
            })
            .await?;
        Ok(())
    }

    pub(crate) async fn local_rollout_path(&self) -> anyhow::Result<Option<PathBuf>> {
        let Some(local_store) = self
            .thread_store
            .as_any()
            .downcast_ref::<LocalThreadStore>()
        else {
            anyhow::bail!(
                "rollout path requested for thread {} but the configured thread store is not local; this legacy path is unsupported for remote thread storage",
                self.thread_id
            );
        };
        local_store
            .live_rollout_path(self.thread_id)
            .await
            .map(Some)
            .map_err(anyhow::Error::from)
    }
}
