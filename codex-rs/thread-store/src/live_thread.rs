use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_rollout::RolloutItem;
use codex_rollout::RolloutPersistenceTelemetry;
use codex_rollout::measure_and_filter_rollout_items;
use codex_rollout::persisted_rollout_items;
use tokio::sync::Mutex;
use tracing::warn;

use crate::AppendThreadItemsParams;
use crate::CreateThreadParams;
use crate::LoadThreadHistoryParams;
use crate::LocalThreadStore;
use crate::PersistContext;
use crate::ReadThreadParams;
use crate::ResumeThreadParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadMetadataPatch;
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::ThreadStoreFuture;
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
    history_mode: ThreadHistoryMode,
    thread_store: Arc<dyn ThreadStore>,
    metadata_sync: Arc<Mutex<ThreadMetadataSync>>,
    persistence_telemetry: RolloutPersistenceTelemetry,
}

/// Owns persistence acquisition and its live thread while initialization is still fallible.
///
/// If initialization returns early after persistence has been opened, dropping this guard discards
/// the live writer without forcing lazy in-memory state to become durable. Call [`commit`] once the
/// session owns the live thread for normal operation. Cancellation leaves an in-flight acquisition
/// owned by the guard: cleanup waits for it to finish before discarding any acquired writer.
#[derive(Default)]
pub struct LiveThreadInitGuard {
    live_thread: Option<LiveThread>,
    acquiring: Option<ThreadStoreFuture<'static, LiveThread>>,
}

impl LiveThreadInitGuard {
    pub fn new(live_thread: Option<LiveThread>) -> Self {
        Self {
            live_thread,
            acquiring: None,
        }
    }

    /// Retains the operation even if the caller stops waiting. Store operations may install a
    /// writer before returning or delegate to another runtime, so they cannot simply be dropped.
    pub async fn acquire(
        &mut self,
        acquisition: impl Future<Output = ThreadStoreResult<LiveThread>> + Send + 'static,
    ) -> ThreadStoreResult<LiveThread> {
        assert!(self.live_thread.is_none() && self.acquiring.is_none());
        self.acquiring = Some(Box::pin(acquisition));
        self.finish_acquisition().await?;
        self.live_thread
            .clone()
            .ok_or_else(|| ThreadStoreError::Internal {
                message: "persistence acquisition returned no live thread".to_owned(),
            })
    }

    async fn finish_acquisition(&mut self) -> ThreadStoreResult<()> {
        if let Some(acquiring) = self.acquiring.as_mut() {
            let result = acquiring.await;
            self.acquiring = None;
            self.live_thread = Some(result?);
        }
        Ok(())
    }

    pub fn as_ref(&self) -> Option<&LiveThread> {
        self.live_thread.as_ref()
    }

    pub fn commit(&mut self) {
        self.live_thread = None;
    }

    pub async fn discard(&mut self) {
        let _ = self.finish_acquisition().await;
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
        if self.live_thread.is_none() && self.acquiring.is_none() {
            return;
        }
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            warn!("failed to discard thread persistence for failed session init: no Tokio runtime");
            return;
        };
        let acquiring = self.acquiring.take();
        let live_thread = self.live_thread.take();
        handle.spawn(async move {
            let live_thread = match acquiring {
                Some(acquiring) => acquiring.await.ok(),
                None => live_thread,
            };
            if let Some(live_thread) = live_thread
                && let Err(err) = live_thread.discard().await
            {
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
        let history_mode = params.history_mode;
        let metadata_sync = ThreadMetadataSync::for_create(&params).await;
        thread_store.create_thread(params).await?;
        Ok(Self {
            thread_id,
            history_mode,
            thread_store,
            metadata_sync: Arc::new(Mutex::new(metadata_sync)),
            persistence_telemetry: RolloutPersistenceTelemetry::new(thread_id),
        })
    }

    /// Create a child thread with inherited model context already durable.
    ///
    /// The boundary belongs in session metadata before the copied prefix is written so history
    /// projection can distinguish inherited context from the child's own records immediately.
    pub async fn create_with_inherited_model_context(
        thread_store: Arc<dyn ThreadStore>,
        mut params: CreateThreadParams,
        inherited_model_context: &[RolloutItem],
        guard: &mut LiveThreadInitGuard,
    ) -> ThreadStoreResult<Self> {
        let persisted_prefix_item_count =
            persisted_rollout_items(inherited_model_context, params.history_mode).len();
        params.subagent_history_start_ordinal = Some(
            u64::try_from(persisted_prefix_item_count)
                .map_err(|_| ThreadStoreError::Internal {
                    message: "inherited model context is too large".to_string(),
                })?
                .checked_add(1)
                .ok_or_else(|| ThreadStoreError::Internal {
                    message: "inherited model context is too large".to_string(),
                })?,
        );
        let live_thread = guard.acquire(Self::create(thread_store, params)).await?;
        live_thread
            .persist_appended_items(inherited_model_context)
            .await?;
        Ok(live_thread)
    }

    pub async fn resume(
        thread_store: Arc<dyn ThreadStore>,
        history_mode: ThreadHistoryMode,
        params: ResumeThreadParams,
    ) -> ThreadStoreResult<Self> {
        let thread_id = params.thread_id;
        let should_load_history = params.history.is_none();
        let include_archived = params.include_archived;
        let metadata = if history_mode == ThreadHistoryMode::Paginated
            && let Some(local_store) = thread_store.as_any().downcast_ref::<LocalThreadStore>()
            && let Some(state_db) = local_store.state_db().await
        {
            state_db
                .get_thread(thread_id)
                .await
                .map_err(|err| ThreadStoreError::Internal {
                    message: format!("failed to read thread metadata for {thread_id}: {err}"),
                })?
        } else {
            None
        };
        let mut metadata_sync = ThreadMetadataSync::for_resume(&params, metadata.as_ref());
        thread_store.resume_thread(params).await?;
        if should_load_history {
            match thread_store
                .load_history(LoadThreadHistoryParams {
                    thread_id,
                    include_archived,
                })
                .await
            {
                Ok(history) => metadata_sync.record_resume_history(&history.items),
                Err(err) => {
                    if let Err(discard_err) = thread_store.discard_thread(thread_id).await {
                        warn!(
                            "failed to discard thread persistence after resume history load failed: {discard_err}"
                        );
                    }
                    return Err(err);
                }
            }
        }
        Ok(Self {
            thread_id,
            history_mode,
            thread_store,
            metadata_sync: Arc::new(Mutex::new(metadata_sync)),
            persistence_telemetry: RolloutPersistenceTelemetry::new(thread_id),
        })
    }

    #[tracing::instrument(
        level = "trace",
        skip_all,
        fields(item_count = raw_items.len())
    )]
    pub async fn append_items(&self, raw_items: &[RolloutItem]) -> ThreadStoreResult<()> {
        let items = self.persist_appended_items(raw_items).await?;
        if items.is_empty() {
            return Ok(());
        }
        let update = self
            .metadata_sync
            .lock()
            .await
            .observe_appended_items(items.as_slice());
        if let Some(update) = update {
            self.thread_store
                .record_thread_metadata(UpdateThreadMetadataParams {
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

    async fn persist_appended_items(
        &self,
        raw_items: &[RolloutItem],
    ) -> ThreadStoreResult<Vec<RolloutItem>> {
        // Empty appends are intentionally ignored rather than represented as zero-sized batches.
        if raw_items.is_empty() {
            return Ok(Vec::new());
        }
        let (items, measurement) = if self.persistence_telemetry.is_enabled() {
            let (items, measurement) =
                measure_and_filter_rollout_items(raw_items, self.history_mode);
            (items, Some(measurement))
        } else {
            (persisted_rollout_items(raw_items, self.history_mode), None)
        };
        self.thread_store
            .append_items(AppendThreadItemsParams {
                thread_id: self.thread_id,
                items: raw_items.to_vec(),
            })
            .await?;
        if let Some(measurement) = measurement.as_ref() {
            self.persistence_telemetry
                .record_batch(raw_items, measurement);
        }
        Ok(items)
    }

    pub async fn persist(&self, context: PersistContext) -> ThreadStoreResult<()> {
        if context.allows_background_persistence() {
            let update = self
                .metadata_sync
                .lock()
                .await
                .take_pending_update_for_existing_history();
            self.apply_pending_metadata_update(update, context).await?;
        }
        self.thread_store
            .persist_thread(self.thread_id, context)
            .await?;
        let update = self.metadata_sync.lock().await.take_pending_update();
        self.apply_pending_metadata_update(update, context).await
    }

    pub async fn flush(&self) -> ThreadStoreResult<()> {
        self.thread_store.flush_thread(self.thread_id).await?;
        self.flush_pending_metadata_update_for_existing_history()
            .await
    }

    pub async fn shutdown(&self) -> ThreadStoreResult<()> {
        let metadata_result = self
            .flush_pending_metadata_update_for_existing_history()
            .await;
        let shutdown_result = self.thread_store.shutdown_thread(self.thread_id).await;
        match (metadata_result, shutdown_result) {
            (Err(metadata_error), Err(shutdown_error)) => Err(ThreadStoreError::Internal {
                message: format!(
                    "thread metadata update failed: {metadata_error}; thread shutdown failed: {shutdown_error}"
                ),
            }),
            (Err(metadata_error), Ok(())) => Err(metadata_error),
            (Ok(()), result) => result,
        }
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

    /// Updates metadata while preserving this API's materialized-thread contract.
    ///
    /// Stores may successfully return no thread for a no-op update, so this reads the thread as a
    /// fallback in that case.
    pub async fn update_metadata(
        &self,
        patch: ThreadMetadataPatch,
        include_archived: bool,
    ) -> ThreadStoreResult<StoredThread> {
        self.flush_pending_metadata_update().await?;
        let updated = self
            .thread_store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id: self.thread_id,
                patch,
                include_archived,
            })
            .await?;
        match updated {
            Some(thread) => Ok(thread),
            None => {
                self.read_thread(include_archived, /*include_history*/ false)
                    .await
            }
        }
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
        self.apply_pending_metadata_update(update, PersistContext::Standard)
            .await
    }

    async fn flush_pending_metadata_update_for_existing_history(&self) -> ThreadStoreResult<()> {
        let update = self
            .metadata_sync
            .lock()
            .await
            .take_pending_update_for_existing_history();
        self.apply_pending_metadata_update(update, PersistContext::Standard)
            .await
    }

    async fn apply_pending_metadata_update(
        &self,
        update: Option<crate::thread_metadata_sync::PendingThreadMetadataPatch>,
        context: PersistContext,
    ) -> ThreadStoreResult<()> {
        let Some(update) = update else {
            return Ok(());
        };
        let params = UpdateThreadMetadataParams {
            thread_id: self.thread_id,
            patch: update.patch.clone(),
            include_archived: true,
        };
        if context == PersistContext::Standard {
            self.thread_store.update_thread_metadata(params).await?;
        } else {
            self.thread_store.record_thread_metadata(params).await?;
        }
        self.metadata_sync
            .lock()
            .await
            .mark_pending_update_applied(&update);
        Ok(())
    }
}
