use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::persisted_rollout_items;
use tokio::sync::Mutex;
use tracing::warn;

use crate::AppendThreadItemsParams;
use crate::CreateThreadParams;
use crate::LoadThreadHistoryParams;
use crate::LocalThreadStore;
use crate::ReadThreadParams;
use crate::ResumeThreadParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadHistoryProjectionObserver;
use crate::ThreadMetadataPatch;
use crate::ThreadStore;
use crate::ThreadStoreResult;
use crate::UpdateThreadMetadataParams;
use crate::thread_metadata_sync::ThreadMetadataSync;

/// Handle for an active thread's persistence lifecycle.
///
/// `LiveThread` keeps lifecycle decisions with the caller while delegating storage details to
/// [`ThreadStore`]. Local stores may use a rollout file internally and remote stores may use a
/// service, but session code should only need this handle for the active thread.
#[derive(Clone)]
pub struct LiveThread {
    thread_id: ThreadId,
    thread_store: Arc<dyn ThreadStore>,
    metadata_sync: Arc<Mutex<ThreadMetadataSync>>,
    thread_history_projection_observers: Arc<Mutex<Vec<Box<dyn ThreadHistoryProjectionObserver>>>>,
}

/// Owns a live thread while session initialization is still fallible.
///
/// If initialization returns early after persistence has been opened, dropping this guard discards
/// the live writer without forcing lazy in-memory state to become durable. Call [`commit`] once the
/// session owns the live thread for normal operation.
pub struct LiveThreadInitGuard {
    live_thread: Option<LiveThread>,
}

impl LiveThreadInitGuard {
    pub fn new(live_thread: Option<LiveThread>) -> Self {
        Self { live_thread }
    }

    pub fn as_ref(&self) -> Option<&LiveThread> {
        self.live_thread.as_ref()
    }

    pub fn commit(&mut self) {
        self.live_thread = None;
    }

    pub async fn discard(&mut self) {
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
    pub async fn create(
        thread_store: Arc<dyn ThreadStore>,
        params: CreateThreadParams,
    ) -> ThreadStoreResult<Self> {
        let thread_id = params.thread_id;
        let metadata_sync = ThreadMetadataSync::for_create(&params).await;
        thread_store.create_thread(params).await?;
        Ok(Self {
            thread_id,
            thread_store,
            metadata_sync: Arc::new(Mutex::new(metadata_sync)),
            thread_history_projection_observers: default_thread_history_projection_observers(),
        })
    }

    pub async fn resume(
        thread_store: Arc<dyn ThreadStore>,
        mut params: ResumeThreadParams,
    ) -> ThreadStoreResult<Self> {
        let thread_id = params.thread_id;
        let should_load_history = params.history.is_none();
        let include_archived = params.include_archived;
        thread_store.resume_thread(params.clone()).await?;
        if should_load_history {
            match thread_store
                .load_history(LoadThreadHistoryParams {
                    thread_id,
                    include_archived,
                })
                .await
            {
                Ok(history) => params.history = Some(history.items),
                Err(err) => {
                    let _ = thread_store.discard_thread(thread_id).await;
                    return Err(err);
                }
            }
        }
        let metadata_sync = ThreadMetadataSync::for_resume(&params);
        Ok(Self {
            thread_id,
            thread_store,
            metadata_sync: Arc::new(Mutex::new(metadata_sync)),
            thread_history_projection_observers: default_thread_history_projection_observers(),
        })
    }

    /// Installs append observers that derive typed thread-history mutations for this live thread.
    pub fn with_thread_history_projection_observers(
        mut self,
        thread_history_projection_observers: Vec<Box<dyn ThreadHistoryProjectionObserver>>,
    ) -> Self {
        self.thread_history_projection_observers =
            Arc::new(Mutex::new(thread_history_projection_observers));
        self
    }

    pub async fn append_items(&self, items: &[RolloutItem]) -> ThreadStoreResult<()> {
        let canonical_items = persisted_rollout_items(items);
        // TODO(wiltzius): Once stores persist thread-history mutations, advance observer state
        // only when the canonical append and derived mutations commit successfully together.
        let thread_history_mutations = self
            .thread_history_projection_observers
            .lock()
            .await
            .iter_mut()
            .flat_map(|observer| observer.observe_append(canonical_items.as_slice(), items))
            .collect::<Vec<_>>();
        if canonical_items.is_empty() && thread_history_mutations.is_empty() {
            return Ok(());
        }
        self.thread_store
            .append_items(AppendThreadItemsParams {
                thread_id: self.thread_id,
                items: canonical_items.clone(),
                thread_history_mutations,
            })
            .await?;
        let update = self
            .metadata_sync
            .lock()
            .await
            .observe_appended_items(canonical_items.as_slice());
        if let Some(update) = update {
            self.thread_store
                .update_thread_metadata(UpdateThreadMetadataParams {
                    thread_id: self.thread_id,
                    patch: update.patch.clone(),
                    include_archived: true,
                })
                .await?;
            self.metadata_sync
                .lock()
                .await
                .mark_pending_update_applied(&update);
        }
        Ok(())
    }

    pub async fn persist(&self) -> ThreadStoreResult<()> {
        self.thread_store.persist_thread(self.thread_id).await?;
        self.flush_pending_metadata_update().await
    }

    pub async fn flush(&self) -> ThreadStoreResult<()> {
        self.thread_store.flush_thread(self.thread_id).await?;
        self.flush_pending_metadata_update_for_existing_history()
            .await
    }

    pub async fn shutdown(&self) -> ThreadStoreResult<()> {
        self.flush_pending_metadata_update_for_existing_history()
            .await?;
        self.thread_store.shutdown_thread(self.thread_id).await
    }

    pub async fn discard(&self) -> ThreadStoreResult<()> {
        self.thread_store.discard_thread(self.thread_id).await
    }

    pub async fn load_history(
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

    pub async fn read_thread(
        &self,
        include_archived: bool,
        include_history: bool,
    ) -> ThreadStoreResult<StoredThread> {
        self.thread_store
            .read_thread(ReadThreadParams {
                thread_id: self.thread_id,
                include_archived,
                include_history,
            })
            .await
    }

    pub async fn update_memory_mode(
        &self,
        mode: ThreadMemoryMode,
        include_archived: bool,
    ) -> ThreadStoreResult<()> {
        self.flush_pending_metadata_update().await?;
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

    pub async fn update_metadata(
        &self,
        patch: ThreadMetadataPatch,
        include_archived: bool,
    ) -> ThreadStoreResult<StoredThread> {
        self.flush_pending_metadata_update().await?;
        self.thread_store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id: self.thread_id,
                patch,
                include_archived,
            })
            .await
    }

    /// Returns the live local rollout path for legacy local-only callers.
    ///
    /// Remote stores do not expose rollout files, so they return `Ok(None)`.
    pub async fn local_rollout_path(&self) -> ThreadStoreResult<Option<PathBuf>> {
        let Some(local_store) = self
            .thread_store
            .as_any()
            .downcast_ref::<LocalThreadStore>()
        else {
            return Ok(None);
        };
        local_store
            .live_rollout_path(self.thread_id)
            .await
            .map(Some)
    }

    async fn flush_pending_metadata_update(&self) -> ThreadStoreResult<()> {
        let update = self.metadata_sync.lock().await.take_pending_update();
        self.apply_pending_metadata_update(update).await
    }

    async fn flush_pending_metadata_update_for_existing_history(&self) -> ThreadStoreResult<()> {
        let update = self
            .metadata_sync
            .lock()
            .await
            .take_pending_update_for_existing_history();
        self.apply_pending_metadata_update(update).await
    }

    async fn apply_pending_metadata_update(
        &self,
        update: Option<crate::thread_metadata_sync::PendingThreadMetadataPatch>,
    ) -> ThreadStoreResult<()> {
        let Some(update) = update else {
            return Ok(());
        };
        self.thread_store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id: self.thread_id,
                patch: update.patch.clone(),
                include_archived: true,
            })
            .await?;
        self.metadata_sync
            .lock()
            .await
            .mark_pending_update_applied(&update);
        Ok(())
    }
}

fn default_thread_history_projection_observers()
-> Arc<Mutex<Vec<Box<dyn ThreadHistoryProjectionObserver>>>> {
    Arc::new(Mutex::new(Vec::new()))
}
