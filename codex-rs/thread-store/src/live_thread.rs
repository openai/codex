use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use codex_protocol::ThreadId;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadMemoryMode;
use tokio::sync::Mutex;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tracing::warn;

use crate::AppendThreadItemsParams;
use crate::ApplyThreadMetadataParams;
use crate::CreateThreadParams;
use crate::LoadThreadHistoryParams;
use crate::LocalThreadStore;
use crate::ReadThreadParams;
use crate::ResumeThreadParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadMetadataPatch;
use crate::ThreadMetadataUpdate;
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::thread_metadata_handler::PreparedThreadMetadata;
use crate::thread_metadata_handler::ThreadMetadataHandler;
use crate::thread_metadata_handler::protocol_git_info;

/// Handle for an active thread's persistence lifecycle.
///
/// `LiveThread` keeps lifecycle decisions with the caller while delegating storage details to
/// [`ThreadStore`]. Local stores may use a rollout file internally and remote stores may use a
/// service, but session code should only need this handle for the active thread.
#[derive(Clone)]
pub struct LiveThread {
    thread_id: ThreadId,
    thread_store: Arc<dyn ThreadStore>,
    metadata_handler: Arc<Mutex<ThreadMetadataHandler>>,
    operation_semaphore: Arc<Semaphore>,
    pending_metadata_updates: Arc<Mutex<VecDeque<ThreadMetadataUpdate>>>,
}

#[derive(Clone, Copy)]
enum MetadataFailurePolicy {
    Propagate,
    WarnAndContinueForLocal,
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
        let metadata_handler = Arc::new(Mutex::new(ThreadMetadataHandler::for_create(&params)));
        thread_store.create_thread(params).await?;
        let live_thread = Self {
            thread_id,
            thread_store,
            metadata_handler,
            operation_semaphore: Arc::new(Semaphore::new(1)),
            pending_metadata_updates: Arc::new(Mutex::new(VecDeque::new())),
        };
        Ok(live_thread)
    }

    pub async fn resume(
        thread_store: Arc<dyn ThreadStore>,
        params: ResumeThreadParams,
    ) -> ThreadStoreResult<Self> {
        let thread_id = params.thread_id;
        thread_store.resume_thread(params.clone()).await?;
        let mut handler_params = params;
        if handler_params.history.is_none() {
            match thread_store
                .load_history(LoadThreadHistoryParams {
                    thread_id,
                    include_archived: handler_params.include_archived,
                })
                .await
            {
                Ok(history) => {
                    handler_params.history = Some(history.items);
                }
                Err(err) => {
                    let _ = thread_store.discard_thread(thread_id).await;
                    return Err(err);
                }
            }
        }
        let metadata_handler = Arc::new(Mutex::new(ThreadMetadataHandler::for_resume(
            &handler_params,
        )));
        Ok(Self {
            thread_id,
            thread_store,
            metadata_handler,
            operation_semaphore: Arc::new(Semaphore::new(1)),
            pending_metadata_updates: Arc::new(Mutex::new(VecDeque::new())),
        })
    }

    pub async fn append_items(&self, items: &[RolloutItem]) -> ThreadStoreResult<()> {
        let _operation_permit = self.acquire_operation_permit().await?;
        self.apply_pending_metadata_updates(MetadataFailurePolicy::WarnAndContinueForLocal)
            .await?;
        let prepared = {
            let handler = self.metadata_handler.lock().await;
            let Some(prepared) = handler.prepare_items(items) else {
                return Ok(());
            };
            prepared
        };
        let initial = self.initial_metadata().await;

        self.append_initial_metadata(initial).await?;
        self.apply_pending_metadata_updates(MetadataFailurePolicy::WarnAndContinueForLocal)
            .await?;

        let update = self.append_prepared_items(&prepared).await?;
        self.metadata_handler
            .lock()
            .await
            .commit_prepared(&prepared);
        self.push_pending_metadata_update(update).await;
        self.apply_pending_metadata_updates(MetadataFailurePolicy::WarnAndContinueForLocal)
            .await?;
        Ok(())
    }

    async fn emit_initial_metadata(&self) -> ThreadStoreResult<()> {
        self.apply_pending_metadata_updates(MetadataFailurePolicy::WarnAndContinueForLocal)
            .await?;
        let Some(prepared) = self.initial_metadata().await else {
            return Ok(());
        };
        self.append_initial_metadata(Some(prepared)).await?;
        self.apply_pending_metadata_updates(MetadataFailurePolicy::WarnAndContinueForLocal)
            .await
    }

    async fn initial_metadata(&self) -> Option<PreparedThreadMetadata> {
        let git_cwd = self.metadata_handler.lock().await.take_initial_git_cwd();
        let git_info = match git_cwd.as_ref() {
            Some(cwd) => protocol_git_info(cwd.as_path()).await,
            None => None,
        };
        let mut handler = self.metadata_handler.lock().await;
        if git_cwd.is_some() {
            handler.set_initial_git_info(git_info);
        }
        handler.initial_metadata()
    }

    async fn acquire_operation_permit(&self) -> ThreadStoreResult<OwnedSemaphorePermit> {
        self.operation_semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ThreadStoreError::Internal {
                message: format!("live thread {} operation semaphore closed", self.thread_id),
            })
    }

    async fn append_prepared_items(
        &self,
        prepared: &PreparedThreadMetadata,
    ) -> ThreadStoreResult<ThreadMetadataUpdate> {
        let update = prepared.update.clone();
        self.thread_store
            .append_items(AppendThreadItemsParams {
                thread_id: self.thread_id,
                items: prepared.items.clone(),
            })
            .await?;
        Ok(update)
    }

    async fn append_initial_metadata(
        &self,
        prepared: Option<PreparedThreadMetadata>,
    ) -> ThreadStoreResult<Option<ThreadMetadataUpdate>> {
        let Some(prepared) = prepared else {
            return Ok(None);
        };
        let update = self.append_prepared_items(&prepared).await?;
        self.mark_initial_metadata_emitted().await;
        self.push_pending_metadata_update(update.clone()).await;
        Ok(Some(update))
    }

    async fn mark_initial_metadata_emitted(&self) {
        self.metadata_handler
            .lock()
            .await
            .mark_initial_metadata_emitted();
    }

    async fn push_pending_metadata_update(&self, update: ThreadMetadataUpdate) {
        if !update.is_empty() {
            self.pending_metadata_updates.lock().await.push_back(update);
        }
    }

    async fn apply_pending_metadata_updates(
        &self,
        failure_policy: MetadataFailurePolicy,
    ) -> ThreadStoreResult<()> {
        loop {
            let Some(update) = self.pending_metadata_updates.lock().await.pop_front() else {
                return Ok(());
            };
            if let Err(err) = self
                .apply_metadata_update(update.clone(), failure_policy)
                .await
            {
                self.pending_metadata_updates
                    .lock()
                    .await
                    .push_front(update);
                return Err(err);
            }
        }
    }

    async fn apply_metadata_update(
        &self,
        update: ThreadMetadataUpdate,
        failure_policy: MetadataFailurePolicy,
    ) -> ThreadStoreResult<()> {
        if update.is_empty() {
            return Ok(());
        }
        let result = self
            .thread_store
            .apply_thread_metadata(ApplyThreadMetadataParams {
                thread_id: self.thread_id,
                update,
            })
            .await;
        if matches!(
            failure_policy,
            MetadataFailurePolicy::WarnAndContinueForLocal
        ) && self.thread_store.as_any().is::<LocalThreadStore>()
            && let Err(err) = result
        {
            warn!("failed to apply local thread metadata update: {err}");
            return Ok(());
        }
        result
    }

    async fn append_prepared_metadata(
        &self,
        prepared: PreparedThreadMetadata,
    ) -> ThreadStoreResult<()> {
        let update = self.append_prepared_items(&prepared).await?;
        self.metadata_handler
            .lock()
            .await
            .commit_prepared(&prepared);
        self.push_pending_metadata_update(update).await;
        self.apply_pending_metadata_updates(MetadataFailurePolicy::Propagate)
            .await
    }

    pub async fn persist(&self) -> ThreadStoreResult<()> {
        let _operation_permit = self.acquire_operation_permit().await?;
        self.emit_initial_metadata().await?;
        self.thread_store.persist_thread(self.thread_id).await
    }

    pub async fn flush(&self) -> ThreadStoreResult<()> {
        let _operation_permit = self.acquire_operation_permit().await?;
        self.emit_initial_metadata().await?;
        self.thread_store.flush_thread(self.thread_id).await
    }

    pub async fn shutdown(&self) -> ThreadStoreResult<()> {
        let _operation_permit = self.acquire_operation_permit().await?;
        self.apply_pending_metadata_updates(MetadataFailurePolicy::WarnAndContinueForLocal)
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
        self.update_metadata(
            ThreadMetadataPatch {
                memory_mode: Some(mode),
                ..Default::default()
            },
            include_archived,
        )
        .await?;
        Ok(())
    }

    pub async fn update_metadata(
        &self,
        patch: ThreadMetadataPatch,
        include_archived: bool,
    ) -> ThreadStoreResult<StoredThread> {
        if patch.name.is_some() || patch.memory_mode.is_some() || patch.git_info.is_some() {
            let _operation_permit = self.acquire_operation_permit().await?;
            self.apply_pending_metadata_updates(MetadataFailurePolicy::WarnAndContinueForLocal)
                .await?;
            self.reject_archived_metadata_update(include_archived)
                .await?;
            let initial = self.initial_metadata().await;
            let prepared = {
                let handler = self.metadata_handler.lock().await;
                handler.prepare_metadata_patch(&patch)
            };

            self.append_initial_metadata(initial).await?;
            self.apply_pending_metadata_updates(MetadataFailurePolicy::WarnAndContinueForLocal)
                .await?;
            if !prepared.items.is_empty() {
                self.append_prepared_metadata(prepared).await?;
            } else {
                self.apply_metadata_update(prepared.update, MetadataFailurePolicy::Propagate)
                    .await?;
            }
        }

        self.thread_store
            .read_thread(ReadThreadParams {
                thread_id: self.thread_id,
                include_archived,
                include_history: false,
            })
            .await
    }

    async fn reject_archived_metadata_update(
        &self,
        include_archived: bool,
    ) -> ThreadStoreResult<()> {
        if include_archived {
            return Ok(());
        }
        match self
            .thread_store
            .read_thread(ReadThreadParams {
                thread_id: self.thread_id,
                include_archived,
                include_history: false,
            })
            .await
        {
            Err(ThreadStoreError::InvalidRequest { message })
                if message.contains(" is archived") =>
            {
                Err(ThreadStoreError::InvalidRequest { message })
            }
            Ok(_) | Err(_) => Ok(()),
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
}
