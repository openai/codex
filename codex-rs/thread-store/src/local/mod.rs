mod archive_thread;
mod create_thread;
mod helpers;
mod list_threads;
mod live_writer;
mod read_thread;
mod unarchive_thread;
mod update_thread_metadata;

#[cfg(test)]
mod test_support;

use async_trait::async_trait;
use codex_protocol::ThreadId;
use codex_rollout::RolloutConfig;
use codex_rollout::RolloutRecorder;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::AppendThreadItemsParams;
use crate::ArchiveThreadParams;
use crate::CreateThreadParams;
use crate::ListThreadsParams;
use crate::LoadThreadHistoryParams;
use crate::ReadThreadParams;
use crate::ResumeThreadParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadPage;
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::UpdateThreadMetadataParams;

/// Local filesystem/SQLite-backed implementation of [`ThreadStore`].
#[derive(Clone)]
pub struct LocalThreadStore {
    pub(super) config: RolloutConfig,
    live_recorders: Arc<Mutex<HashMap<ThreadId, RolloutRecorder>>>,
}

impl std::fmt::Debug for LocalThreadStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalThreadStore")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl LocalThreadStore {
    /// Create a local store from the rollout configuration used by existing local persistence.
    pub fn new(config: RolloutConfig) -> Self {
        Self {
            config,
            live_recorders: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Read a local rollout-backed thread by path.
    pub async fn read_thread_by_rollout_path(
        &self,
        rollout_path: PathBuf,
        include_archived: bool,
        include_history: bool,
    ) -> ThreadStoreResult<StoredThread> {
        read_thread::read_thread_by_rollout_path(
            self,
            rollout_path,
            include_archived,
            include_history,
        )
        .await
    }

    /// Return the live local rollout path for legacy local-only code paths.
    pub async fn live_rollout_path(&self, thread_id: ThreadId) -> ThreadStoreResult<PathBuf> {
        live_writer::rollout_path(self, thread_id).await
    }

    pub(super) async fn live_recorder(
        &self,
        thread_id: ThreadId,
    ) -> ThreadStoreResult<RolloutRecorder> {
        self.live_recorders
            .lock()
            .await
            .get(&thread_id)
            .cloned()
            .ok_or(ThreadStoreError::ThreadNotFound { thread_id })
    }

    pub(super) async fn insert_live_recorder(
        &self,
        thread_id: ThreadId,
        recorder: RolloutRecorder,
    ) {
        self.live_recorders.lock().await.insert(thread_id, recorder);
    }
}

#[async_trait]
impl ThreadStore for LocalThreadStore {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn create_thread(&self, params: CreateThreadParams) -> ThreadStoreResult<()> {
        live_writer::create_thread(self, params).await
    }

    async fn resume_thread(&self, params: ResumeThreadParams) -> ThreadStoreResult<()> {
        live_writer::resume_thread(self, params).await
    }

    async fn append_items(&self, params: AppendThreadItemsParams) -> ThreadStoreResult<()> {
        live_writer::append_items(self, params).await
    }

    async fn persist_thread(&self, thread_id: ThreadId) -> ThreadStoreResult<()> {
        live_writer::persist_thread(self, thread_id).await
    }

    async fn flush_thread(&self, thread_id: ThreadId) -> ThreadStoreResult<()> {
        live_writer::flush_thread(self, thread_id).await
    }

    async fn shutdown_thread(&self, thread_id: ThreadId) -> ThreadStoreResult<()> {
        live_writer::shutdown_thread(self, thread_id).await
    }

    async fn discard_thread(&self, thread_id: ThreadId) -> ThreadStoreResult<()> {
        live_writer::discard_thread(self, thread_id).await
    }

    async fn load_history(
        &self,
        params: LoadThreadHistoryParams,
    ) -> ThreadStoreResult<StoredThreadHistory> {
        read_thread::read_thread(
            self,
            ReadThreadParams {
                thread_id: params.thread_id,
                include_archived: params.include_archived,
                include_history: true,
            },
        )
        .await?
        .history
        .ok_or_else(|| ThreadStoreError::Internal {
            message: format!("failed to load history for thread {}", params.thread_id),
        })
    }

    async fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreResult<StoredThread> {
        read_thread::read_thread(self, params).await
    }

    async fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreResult<ThreadPage> {
        list_threads::list_threads(self, params).await
    }

    async fn update_thread_metadata(
        &self,
        params: UpdateThreadMetadataParams,
    ) -> ThreadStoreResult<StoredThread> {
        update_thread_metadata::update_thread_metadata(self, params).await
    }

    async fn archive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreResult<()> {
        archive_thread::archive_thread(self, params).await
    }

    async fn unarchive_thread(
        &self,
        params: ArchiveThreadParams,
    ) -> ThreadStoreResult<StoredThread> {
        unarchive_thread::unarchive_thread(self, params).await
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::ThreadId;
    use codex_protocol::models::BaseInstructions;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::UserMessageEvent;
    use tempfile::TempDir;

    use super::*;
    use crate::ThreadEventPersistenceMode;
    use crate::local::test_support::test_config;

    #[tokio::test]
    async fn live_writer_lifecycle_writes_and_closes() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        let thread_id = ThreadId::default();

        store
            .create_thread(create_thread_params(thread_id))
            .await
            .expect("create live thread");
        let rollout_path = store
            .live_rollout_path(thread_id)
            .await
            .expect("load rollout path");

        store
            .append_items(AppendThreadItemsParams {
                thread_id,
                items: vec![user_message_item("first live write")],
            })
            .await
            .expect("append live item");
        store
            .persist_thread(thread_id)
            .await
            .expect("persist live thread");
        store
            .flush_thread(thread_id)
            .await
            .expect("flush live thread");

        assert_rollout_contains_message(rollout_path.as_path(), "first live write").await;

        store
            .shutdown_thread(thread_id)
            .await
            .expect("shutdown live thread");
        let err = store
            .append_items(AppendThreadItemsParams {
                thread_id,
                items: vec![user_message_item("write after shutdown")],
            })
            .await
            .expect_err("shutdown should remove the live thread writer");
        assert!(
            matches!(err, ThreadStoreError::ThreadNotFound { thread_id: missing } if missing == thread_id)
        );
    }

    #[tokio::test]
    async fn discard_thread_drops_unmaterialized_live_writer() {
        let home = TempDir::new().expect("temp dir");
        let store = LocalThreadStore::new(test_config(home.path()));
        let thread_id = ThreadId::default();

        store
            .create_thread(create_thread_params(thread_id))
            .await
            .expect("create live thread");
        let rollout_path = store
            .live_rollout_path(thread_id)
            .await
            .expect("load rollout path");
        store
            .discard_thread(thread_id)
            .await
            .expect("discard live thread");

        assert!(
            !tokio::fs::try_exists(rollout_path.as_path())
                .await
                .expect("check rollout path")
        );
        let err = store
            .append_items(AppendThreadItemsParams {
                thread_id,
                items: vec![user_message_item("write after discard")],
            })
            .await
            .expect_err("discard should remove the live thread writer");
        assert!(
            matches!(err, ThreadStoreError::ThreadNotFound { thread_id: missing } if missing == thread_id)
        );
    }

    #[tokio::test]
    async fn resume_thread_reopens_live_writer_and_appends() {
        let home = TempDir::new().expect("temp dir");
        let config = test_config(home.path());
        let thread_id = ThreadId::default();

        let first_store = LocalThreadStore::new(config.clone());
        first_store
            .create_thread(create_thread_params(thread_id))
            .await
            .expect("create initial thread");
        first_store
            .append_items(AppendThreadItemsParams {
                thread_id,
                items: vec![user_message_item("before resume")],
            })
            .await
            .expect("append initial item");
        first_store
            .persist_thread(thread_id)
            .await
            .expect("persist initial thread");
        first_store
            .flush_thread(thread_id)
            .await
            .expect("flush initial thread");
        let rollout_path = first_store
            .live_rollout_path(thread_id)
            .await
            .expect("load rollout path");
        first_store
            .shutdown_thread(thread_id)
            .await
            .expect("shutdown initial writer");

        let resumed_store = LocalThreadStore::new(config);
        resumed_store
            .resume_thread(ResumeThreadParams {
                thread_id,
                rollout_path: None,
                history: None,
                include_archived: true,
                event_persistence_mode: ThreadEventPersistenceMode::Limited,
            })
            .await
            .expect("resume live thread");
        resumed_store
            .append_items(AppendThreadItemsParams {
                thread_id,
                items: vec![user_message_item("after resume")],
            })
            .await
            .expect("append resumed item");
        resumed_store
            .flush_thread(thread_id)
            .await
            .expect("flush resumed thread");

        assert_rollout_contains_message(rollout_path.as_path(), "before resume").await;
        assert_rollout_contains_message(rollout_path.as_path(), "after resume").await;
    }

    fn create_thread_params(thread_id: ThreadId) -> CreateThreadParams {
        CreateThreadParams {
            thread_id,
            forked_from_id: None,
            source: SessionSource::Exec,
            base_instructions: BaseInstructions::default(),
            dynamic_tools: Vec::new(),
            event_persistence_mode: ThreadEventPersistenceMode::Limited,
        }
    }

    fn user_message_item(message: &str) -> RolloutItem {
        RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
            message: message.to_string(),
            images: None,
            local_images: Vec::new(),
            text_elements: Vec::new(),
        }))
    }

    async fn assert_rollout_contains_message(path: &std::path::Path, expected: &str) {
        let (items, _, _) = RolloutRecorder::load_rollout_items(path)
            .await
            .expect("load rollout items");
        assert!(items.iter().any(|item| {
            matches!(
                item,
                RolloutItem::EventMsg(EventMsg::UserMessage(event)) if event.message == expected
            )
        }));
    }
}
