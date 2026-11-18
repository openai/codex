//! Hosted realtime test store with a durable clock floor and optional seal failures.
use codex_history::ResponseItemEnvelope;
use codex_history::RolloutItem;
use codex_protocol::ThreadId;
use codex_protocol::realtime::RealtimeItem;
use codex_protocol::realtime::RealtimeItemContent;
use codex_thread_store::AppendThreadItemsParams;
use codex_thread_store::ArchiveThreadParams;
use codex_thread_store::CreateThreadParams;
use codex_thread_store::DeleteThreadParams;
use codex_thread_store::InMemoryThreadStore;
use codex_thread_store::ListThreadsParams;
use codex_thread_store::LoadThreadHistoryParams;
use codex_thread_store::PersistContext;
use codex_thread_store::ReadThreadByRolloutPathParams;
use codex_thread_store::ReadThreadParams;
use codex_thread_store::ResumeThreadParams;
use codex_thread_store::StoredModelContext;
use codex_thread_store::StoredThread;
use codex_thread_store::StoredThreadHistory;
use codex_thread_store::ThreadPage;
use codex_thread_store::ThreadStore;
use codex_thread_store::ThreadStoreError;
use codex_thread_store::ThreadStoreFuture;
use codex_thread_store::UpdateThreadMetadataParams;
use std::any::Any;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::sync::Notify;

pub(super) const FRAME_FLOOR_MS: i64 = 4_102_444_800_000;

#[derive(Default)]
pub(in crate::suite) struct HostedClockStore {
    inner: InMemoryThreadStore,
    enrollment_reads: AtomicUsize,
    floor_reads: AtomicUsize,
    fail_close: bool,
    close_attempts: AtomicUsize,
    fail_after_input_commit: AtomicBool,
    input_attempts: AtomicUsize,
    sealed_inputs: Mutex<HashSet<String>>,
    block_first_frame: AtomicBool,
    frame_started: Notify,
    frame_release: Notify,
}

impl HostedClockStore {
    pub(in crate::suite) fn blocking_frame() -> Self {
        Self {
            block_first_frame: AtomicBool::new(true),
            ..Self::default()
        }
    }

    pub(in crate::suite) async fn wait_for_blocked_frame(&self) {
        self.frame_started.notified().await;
    }

    pub(in crate::suite) fn release_frame(&self) {
        self.frame_release.notify_one();
    }

    pub(in crate::suite) fn failing_close() -> Self {
        Self {
            fail_close: true,
            ..Self::default()
        }
    }

    pub(in crate::suite) fn close_attempts(&self) -> usize {
        self.close_attempts.load(Ordering::Relaxed)
    }

    pub(in crate::suite) fn failing_after_input_commit() -> Self {
        Self {
            fail_after_input_commit: AtomicBool::new(true),
            ..Self::default()
        }
    }

    pub(in crate::suite) fn input_attempts(&self) -> usize {
        self.input_attempts.load(Ordering::Relaxed)
    }

    pub(super) fn reads(&self) -> (usize, usize) {
        (
            self.enrollment_reads.load(Ordering::Relaxed),
            self.floor_reads.load(Ordering::Relaxed),
        )
    }
}

macro_rules! delegate_store_methods {
    ($(fn $name:ident($param:ident: $params:ty) -> $result:ty;)*) => {
        $(fn $name(&self, $param: $params) -> ThreadStoreFuture<'_, $result> {
            self.inner.$name($param)
        })*
    };
}

impl ThreadStore for HostedClockStore {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn realtime_frames_enabled(&self, _: ThreadId) -> ThreadStoreFuture<'_, bool> {
        self.enrollment_reads.fetch_add(1, Ordering::Relaxed);
        Box::pin(async { Ok(true) })
    }

    fn realtime_frame_clock_floor(&self, _: ThreadId) -> ThreadStoreFuture<'_, Option<i64>> {
        self.floor_reads.fetch_add(1, Ordering::Relaxed);
        Box::pin(async { Ok(Some(FRAME_FLOOR_MS)) })
    }

    fn seal_realtime_items(
        &self,
        thread_id: ThreadId,
        items: Vec<RealtimeItem>,
    ) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            self.append_items(AppendThreadItemsParams {
                thread_id,
                items: items.into_iter().map(RolloutItem::RealtimeItem).collect(),
            })
            .await
        })
    }

    fn append_items(&self, params: AppendThreadItemsParams) -> ThreadStoreFuture<'_, ()> {
        if params.items.iter().any(|item| {
            matches!(item, RolloutItem::RealtimeItem(item) if matches!(
                item.content, RealtimeItemContent::RealtimeSessionClosed { .. }
            ))
        }) {
            self.close_attempts.fetch_add(1, Ordering::Relaxed);
            if self.fail_close {
                return Box::pin(async {
                    Err(ThreadStoreError::OutcomeUnknown {
                        message: "injected close failure".to_owned(),
                    })
                });
            }
        }
        Box::pin(async move {
            if params.items.iter().any(|item| {
                matches!(item, RolloutItem::RealtimeItem(item) if matches!(
                    item.content, RealtimeItemContent::Frame { .. }
                ))
            }) && self.block_first_frame.swap(false, Ordering::Relaxed)
            {
                self.frame_started.notify_one();
                self.frame_release.notified().await;
            }
            self.inner.append_items(params).await
        })
    }

    fn load_realtime_session(
        &self,
        thread_id: ThreadId,
        session_id: String,
    ) -> ThreadStoreFuture<'_, Option<Vec<RealtimeItem>>> {
        Box::pin(async move {
            let context = self
                .inner
                .load_latest_model_context(LoadThreadHistoryParams {
                    thread_id,
                    include_archived: true,
                })
                .await?;
            let items: Vec<_> = context
                .items
                .iter()
                .filter_map(|item| item.realtime_item().map(std::borrow::Cow::into_owned))
                .filter(|item| item.realtime_session_id == session_id)
                .collect();
            Ok(items
                .iter()
                .any(|item| matches!(item.content, RealtimeItemContent::RealtimeSessionStarted))
                .then_some(items))
        })
    }

    fn load_realtime_session_head(
        &self,
        thread_id: ThreadId,
    ) -> ThreadStoreFuture<'_, Option<String>> {
        Box::pin(async move {
            let context = self
                .inner
                .load_latest_model_context(LoadThreadHistoryParams {
                    thread_id,
                    include_archived: true,
                })
                .await?;
            Ok(context
                .items
                .iter()
                .rev()
                .filter_map(RolloutItem::realtime_item)
                .find_map(|item| {
                    matches!(item.content, RealtimeItemContent::RealtimeSessionStarted)
                        .then(|| item.realtime_session_id.clone())
                }))
        })
    }

    fn persist_thread(
        &self,
        thread_id: ThreadId,
        context: PersistContext,
    ) -> ThreadStoreFuture<'_, ()> {
        self.inner.persist_thread(thread_id, context)
    }

    fn seal_realtime_input(
        &self,
        thread_id: ThreadId,
        frames: Vec<RealtimeItem>,
        input: ResponseItemEnvelope,
    ) -> ThreadStoreFuture<'_, ()> {
        self.input_attempts.fetch_add(1, Ordering::Relaxed);
        Box::pin(async move {
            let id = input.item.id().expect("accepted input ID").to_string();
            // This fixture retries seals serially; do not hold its bookkeeping lock across storage I/O.
            if self.sealed_inputs.lock().await.contains(&id) {
                return Ok(());
            }
            let mut items = frames
                .into_iter()
                .map(RolloutItem::RealtimeItem)
                .collect::<Vec<_>>();
            items.push(RolloutItem::ResponseItem(input));
            self.inner
                .append_items(AppendThreadItemsParams { thread_id, items })
                .await?;
            self.sealed_inputs.lock().await.insert(id);
            if self.fail_after_input_commit.swap(false, Ordering::Relaxed) {
                return Err(ThreadStoreError::OutcomeUnknown {
                    message: "injected error after input commit".to_owned(),
                });
            }
            Ok(())
        })
    }

    delegate_store_methods! {
        fn read_thread(params: ReadThreadParams) -> StoredThread;
        fn resume_thread(params: ResumeThreadParams) -> Arc<Vec<RolloutItem>>;
        fn create_thread(params: CreateThreadParams) -> ();
        fn flush_thread(thread_id: ThreadId) -> ();
        fn shutdown_thread(thread_id: ThreadId) -> ();
        fn discard_thread(thread_id: ThreadId) -> ();
        fn load_history(params: LoadThreadHistoryParams) -> StoredThreadHistory;
        fn load_latest_model_context(params: LoadThreadHistoryParams) -> StoredModelContext;
        fn read_thread_by_rollout_path(params: ReadThreadByRolloutPathParams) -> StoredThread;
        fn list_threads(params: ListThreadsParams) -> ThreadPage;
        fn update_thread_metadata(params: UpdateThreadMetadataParams) -> Option<StoredThread>;
        fn archive_thread(params: ArchiveThreadParams) -> ();
        fn unarchive_thread(params: ArchiveThreadParams) -> StoredThread;
        fn delete_thread(params: DeleteThreadParams) -> ();
    }
}
