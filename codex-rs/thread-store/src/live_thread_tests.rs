use std::any::Any;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadMemoryMode;
use codex_protocol::protocol::UserMessageEvent;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use uuid::Uuid;

use super::LiveThread;
use crate::AppendThreadItemsParams;
use crate::ArchiveThreadParams;
use crate::CreateThreadParams;
use crate::DeleteThreadParams;
use crate::InMemoryThreadStore;
use crate::ItemPage;
use crate::ListItemsParams;
use crate::ListThreadsParams;
use crate::ListTurnsParams;
use crate::LoadThreadHistoryParams;
use crate::ReadThreadByRolloutPathParams;
use crate::ReadThreadParams;
use crate::ResumeThreadParams;
use crate::SearchThreadsParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadPage;
use crate::ThreadPersistenceMetadata;
use crate::ThreadSearchPage;
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::ThreadStoreFuture;
use crate::ThreadStoreResult;
use crate::TurnPage;
use crate::UpdateThreadMetadataParams;

const TEST_TIMEOUT: Duration = Duration::from_secs(1);

struct MetadataUpdateAttempt {
    params: UpdateThreadMetadataParams,
    completion: oneshot::Sender<ThreadStoreResult<()>>,
}

struct ControlledThreadStore {
    inner: InMemoryThreadStore,
    update_sender: mpsc::UnboundedSender<MetadataUpdateAttempt>,
}

impl ControlledThreadStore {
    fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<MetadataUpdateAttempt>) {
        let (update_sender, update_receiver) = mpsc::unbounded_channel();
        (
            Arc::new(Self {
                inner: InMemoryThreadStore::default(),
                update_sender,
            }),
            update_receiver,
        )
    }
}

impl ThreadStore for ControlledThreadStore {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn create_thread(&self, params: CreateThreadParams) -> ThreadStoreFuture<'_, ()> {
        ThreadStore::create_thread(&self.inner, params)
    }

    fn resume_thread(&self, params: ResumeThreadParams) -> ThreadStoreFuture<'_, ()> {
        ThreadStore::resume_thread(&self.inner, params)
    }

    fn append_items(&self, params: AppendThreadItemsParams) -> ThreadStoreFuture<'_, ()> {
        ThreadStore::append_items(&self.inner, params)
    }

    fn persist_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        ThreadStore::persist_thread(&self.inner, thread_id)
    }

    fn flush_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        ThreadStore::flush_thread(&self.inner, thread_id)
    }

    fn shutdown_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        ThreadStore::shutdown_thread(&self.inner, thread_id)
    }

    fn discard_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        ThreadStore::discard_thread(&self.inner, thread_id)
    }

    fn load_history(
        &self,
        params: LoadThreadHistoryParams,
    ) -> ThreadStoreFuture<'_, StoredThreadHistory> {
        ThreadStore::load_history(&self.inner, params)
    }

    fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreFuture<'_, StoredThread> {
        ThreadStore::read_thread(&self.inner, params)
    }

    fn read_thread_by_rollout_path(
        &self,
        params: ReadThreadByRolloutPathParams,
    ) -> ThreadStoreFuture<'_, StoredThread> {
        ThreadStore::read_thread_by_rollout_path(&self.inner, params)
    }

    fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreFuture<'_, ThreadPage> {
        ThreadStore::list_threads(&self.inner, params)
    }

    fn search_threads(
        &self,
        params: SearchThreadsParams,
    ) -> ThreadStoreFuture<'_, ThreadSearchPage> {
        ThreadStore::search_threads(&self.inner, params)
    }

    fn list_turns(&self, params: ListTurnsParams) -> ThreadStoreFuture<'_, TurnPage> {
        ThreadStore::list_turns(&self.inner, params)
    }

    fn list_items(&self, params: ListItemsParams) -> ThreadStoreFuture<'_, ItemPage> {
        ThreadStore::list_items(&self.inner, params)
    }

    fn update_thread_metadata(
        &self,
        params: UpdateThreadMetadataParams,
    ) -> ThreadStoreFuture<'_, StoredThread> {
        Box::pin(async move {
            let (completion, result) = oneshot::channel();
            self.update_sender
                .send(MetadataUpdateAttempt {
                    params: params.clone(),
                    completion,
                })
                .map_err(|_| ThreadStoreError::Internal {
                    message: "metadata update test receiver dropped".to_string(),
                })?;
            result.await.map_err(|err| ThreadStoreError::Internal {
                message: format!("metadata update test completion dropped: {err}"),
            })??;
            ThreadStore::update_thread_metadata(&self.inner, params).await
        })
    }

    fn archive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreFuture<'_, ()> {
        ThreadStore::archive_thread(&self.inner, params)
    }

    fn unarchive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreFuture<'_, StoredThread> {
        ThreadStore::unarchive_thread(&self.inner, params)
    }

    fn delete_thread(&self, params: DeleteThreadParams) -> ThreadStoreFuture<'_, ()> {
        ThreadStore::delete_thread(&self.inner, params)
    }
}

#[tokio::test]
async fn append_returns_after_history_acceptance_while_metadata_is_blocked() {
    let (store, mut updates) = ControlledThreadStore::new();
    let live_thread = create_live_thread(Arc::clone(&store)).await;

    timeout(
        TEST_TIMEOUT,
        live_thread.append_items(&[user_message("first message")]),
    )
    .await
    .expect("append should not wait for metadata persistence")
    .expect("append should succeed");
    let attempt = next_update(&mut updates).await;

    assert_eq!(store.inner.calls().await.append_items, 1);
    assert_eq!(store.inner.calls().await.update_thread_metadata, 0);
    assert_eq!(
        attempt.params.patch.preview.as_deref(),
        Some("first message")
    );

    attempt
        .completion
        .send(Ok(()))
        .expect("metadata worker should still be waiting");
    live_thread
        .flush()
        .await
        .expect("flush should drain metadata");
    assert_eq!(store.inner.calls().await.update_thread_metadata, 1);
}

#[tokio::test]
async fn appends_coalesce_while_metadata_update_is_in_flight() {
    let (store, mut updates) = ControlledThreadStore::new();
    let live_thread = create_live_thread(Arc::clone(&store)).await;

    live_thread
        .append_items(&[user_message("first message")])
        .await
        .expect("first append should succeed");
    let first_attempt = next_update(&mut updates).await;
    live_thread
        .append_items(&[user_message("second message")])
        .await
        .expect("second append should succeed");
    live_thread
        .append_items(&[user_message("third message")])
        .await
        .expect("third append should succeed");

    first_attempt
        .completion
        .send(Ok(()))
        .expect("metadata worker should still be waiting");
    let second_attempt = next_update(&mut updates).await;
    let mut flush = tokio::spawn({
        let live_thread = live_thread.clone();
        async move { live_thread.flush().await }
    });
    assert!(
        timeout(Duration::from_millis(20), &mut flush)
            .await
            .is_err(),
        "flush should wait for the in-flight coalesced update"
    );
    live_thread
        .append_items(&[user_message("accepted after flush barrier")])
        .await
        .expect("later append should succeed");
    second_attempt
        .completion
        .send(Ok(()))
        .expect("metadata worker should still be waiting");
    timeout(TEST_TIMEOUT, flush)
        .await
        .expect("flush should not wait for metadata accepted after its barrier")
        .expect("flush task should not panic")
        .expect("flush should succeed");
    let third_attempt = next_update(&mut updates).await;
    third_attempt
        .completion
        .send(Ok(()))
        .expect("metadata worker should still be waiting");
    live_thread
        .flush()
        .await
        .expect("final flush should drain later metadata");

    let calls = store.inner.calls().await;
    assert_eq!(calls.append_items, 4);
    assert_eq!(calls.update_thread_metadata, 3);
}

#[tokio::test]
async fn queued_generation_keeps_its_barrier_when_in_flight_update_fails() {
    let (store, mut updates) = ControlledThreadStore::new();
    let live_thread = create_live_thread(Arc::clone(&store)).await;

    live_thread
        .append_items(&[user_message("first message")])
        .await
        .expect("first append should succeed");
    let first_attempt = next_update(&mut updates).await;
    live_thread
        .append_items(&[user_message("second message")])
        .await
        .expect("second append should succeed");
    let mut flush = tokio::spawn({
        let live_thread = live_thread.clone();
        async move { live_thread.flush().await }
    });
    assert!(
        timeout(Duration::from_millis(20), &mut flush)
            .await
            .is_err(),
        "flush should wait for the queued generation"
    );

    first_attempt
        .completion
        .send(Err(test_error("superseded generation failed")))
        .expect("metadata worker should still be waiting");
    let retry_attempt = next_update(&mut updates).await;
    assert!(
        timeout(Duration::from_millis(20), &mut flush)
            .await
            .is_err(),
        "a failure covered by the queued generation must not fail its barrier"
    );
    retry_attempt
        .completion
        .send(Ok(()))
        .expect("metadata worker should still be waiting");
    timeout(TEST_TIMEOUT, flush)
        .await
        .expect("flush should finish after the queued generation is applied")
        .expect("flush task should not panic")
        .expect("flush should succeed");
    assert_eq!(store.inner.calls().await.update_thread_metadata, 1);
}

#[derive(Clone)]
struct WorkerSpanParentLayer {
    parent_name: Arc<StdMutex<Option<Option<String>>>>,
}

impl<S> Layer<S> for WorkerSpanParentLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        _attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: Context<'_, S>,
    ) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        if span.metadata().name() != "thread_store.metadata_update_worker" {
            return;
        }
        *self.parent_name.lock().expect("parent name lock") = Some(
            span.parent()
                .map(|parent| parent.metadata().name().to_string()),
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn metadata_worker_span_preserves_append_trace() {
    let parent_name = Arc::new(StdMutex::new(None));
    let _guard = tracing_subscriber::registry()
        .with(WorkerSpanParentLayer {
            parent_name: Arc::clone(&parent_name),
        })
        .set_default();

    // Register both production callsites before rebuilding the process-global interest cache.
    // The subscriber must be active first because tracing otherwise short-circuits at level OFF.
    {
        let (store, mut updates) = ControlledThreadStore::new();
        let live_thread = create_live_thread(Arc::clone(&store)).await;
        live_thread
            .append_items(&[user_message("warm up tracing callsites")])
            .await
            .expect("warm-up append should succeed");
        next_update(&mut updates)
            .await
            .completion
            .send(Ok(()))
            .expect("warm-up metadata worker should still be waiting");
        live_thread
            .flush()
            .await
            .expect("warm-up flush should drain metadata");
    }
    tracing::callsite::rebuild_interest_cache();
    *parent_name.lock().expect("parent name lock") = None;

    let (store, mut updates) = ControlledThreadStore::new();
    let live_thread = create_live_thread(Arc::clone(&store)).await;

    live_thread
        .append_items(&[user_message("trace me")])
        .await
        .expect("append should succeed");
    next_update(&mut updates)
        .await
        .completion
        .send(Ok(()))
        .expect("metadata worker should still be waiting");
    live_thread
        .flush()
        .await
        .expect("flush should drain metadata");

    let parent_name = parent_name.lock().expect("parent name lock");
    assert_eq!(
        parent_name.as_ref().map(Option::as_deref),
        Some(Some("thread_store.live_append.observe_metadata"))
    );
}

#[tokio::test]
async fn failed_async_update_is_retried_and_reported_by_barriers() {
    let (store, mut updates) = ControlledThreadStore::new();
    let live_thread = create_live_thread(Arc::clone(&store)).await;

    live_thread
        .append_items(&[user_message("retry me")])
        .await
        .expect("append should succeed independently of metadata");
    next_update(&mut updates)
        .await
        .completion
        .send(Err(test_error("background failure")))
        .expect("metadata worker should still be waiting");
    wait_until_worker_stops(&live_thread).await;

    let retrying_flush = tokio::spawn({
        let live_thread = live_thread.clone();
        async move { live_thread.flush().await }
    });
    next_update(&mut updates)
        .await
        .completion
        .send(Err(ThreadStoreError::Conflict {
            message: "barrier failure".to_string(),
        }))
        .expect("metadata worker should still be waiting");
    let error = retrying_flush
        .await
        .expect("flush task should not panic")
        .expect_err("barrier should report the repeated failure");
    assert!(matches!(
        error,
        ThreadStoreError::Conflict { message } if message == "barrier failure"
    ));
    wait_until_worker_stops(&live_thread).await;

    let successful_flush = tokio::spawn({
        let live_thread = live_thread.clone();
        async move { live_thread.flush().await }
    });
    next_update(&mut updates)
        .await
        .completion
        .send(Ok(()))
        .expect("metadata worker should still be waiting");
    successful_flush
        .await
        .expect("flush task should not panic")
        .expect("later barrier should retry pending metadata successfully");
    assert_eq!(store.inner.calls().await.update_thread_metadata, 1);
}

#[tokio::test]
async fn shutdown_waits_for_queued_metadata_before_closing_store() {
    let (store, mut updates) = ControlledThreadStore::new();
    let live_thread = create_live_thread(Arc::clone(&store)).await;

    live_thread
        .append_items(&[user_message("finish before shutdown")])
        .await
        .expect("append should succeed");
    let attempt = next_update(&mut updates).await;
    let mut shutdown = tokio::spawn({
        let live_thread = live_thread.clone();
        async move { live_thread.shutdown().await }
    });
    assert!(
        timeout(Duration::from_millis(20), &mut shutdown)
            .await
            .is_err(),
        "shutdown should wait for the in-flight metadata update"
    );
    assert_eq!(store.inner.calls().await.shutdown_thread, 0);

    attempt
        .completion
        .send(Ok(()))
        .expect("metadata worker should still be waiting");
    shutdown
        .await
        .expect("shutdown task should not panic")
        .expect("shutdown should succeed");
    assert_eq!(store.inner.calls().await.shutdown_thread, 1);
}

#[tokio::test]
async fn discard_waits_for_in_flight_metadata_before_discarding_store() {
    let (store, mut updates) = ControlledThreadStore::new();
    let live_thread = create_live_thread(Arc::clone(&store)).await;

    live_thread
        .append_items(&[user_message("discard me")])
        .await
        .expect("append should succeed");
    let attempt = next_update(&mut updates).await;
    let mut discard = tokio::spawn({
        let live_thread = live_thread.clone();
        async move { live_thread.discard().await }
    });
    assert!(
        timeout(Duration::from_millis(20), &mut discard)
            .await
            .is_err(),
        "discard should wait for the in-flight metadata update"
    );
    assert_eq!(store.inner.calls().await.discard_thread, 0);

    attempt
        .completion
        .send(Ok(()))
        .expect("metadata worker should still be waiting");
    discard
        .await
        .expect("discard task should not panic")
        .expect("discard should succeed");
    assert_eq!(store.inner.calls().await.discard_thread, 1);
}

async fn create_live_thread(store: Arc<ControlledThreadStore>) -> LiveThread {
    let thread_id = ThreadId::new();
    let thread_store: Arc<dyn ThreadStore> = store;
    LiveThread::create(thread_store, create_params(thread_id))
        .await
        .expect("live thread should be created")
}

fn create_params(thread_id: ThreadId) -> CreateThreadParams {
    CreateThreadParams {
        session_id: thread_id.into(),
        thread_id,
        extra_config: None,
        forked_from_id: None,
        parent_thread_id: None,
        source: SessionSource::Exec,
        thread_source: None,
        originator: "test_originator".to_string(),
        base_instructions: BaseInstructions::default(),
        dynamic_tools: Vec::new(),
        selected_capability_roots: Vec::new(),
        multi_agent_version: None,
        history_mode: ThreadHistoryMode::Legacy,
        initial_window_id: Uuid::now_v7().to_string(),
        metadata: ThreadPersistenceMetadata {
            cwd: None,
            model_provider: "test-provider".to_string(),
            memory_mode: ThreadMemoryMode::Enabled,
        },
    }
}

fn user_message(message: &str) -> RolloutItem {
    RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
        message: message.to_string(),
        ..Default::default()
    }))
}

async fn next_update(
    updates: &mut mpsc::UnboundedReceiver<MetadataUpdateAttempt>,
) -> MetadataUpdateAttempt {
    timeout(TEST_TIMEOUT, updates.recv())
        .await
        .expect("metadata update should start")
        .expect("metadata update channel should remain open")
}

async fn wait_until_worker_stops(live_thread: &LiveThread) {
    timeout(TEST_TIMEOUT, async {
        loop {
            if !live_thread.metadata_worker.state.lock().await.running {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("metadata worker should stop");
}

fn test_error(message: &str) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: message.to_string(),
    }
}
