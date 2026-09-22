//! Exercises read overlap and ordered identity registration when a V2 root resumes cold.

use super::ROLE_MODEL;
use super::ROLE_NAME;
use super::body_contains;
use super::configure_multi_agent_v2_with_role;
use super::mount_root_collaboration_call;
use super::request_has_input_type;
use super::request_has_model;
use anyhow::Context;
use anyhow::Result;
use codex_features::Feature;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_thread_store::AppendThreadItemsParams;
use codex_thread_store::ArchiveThreadParams;
use codex_thread_store::CreateThreadParams;
use codex_thread_store::DeleteThreadParams;
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
use core_test_support::responses::ev_completed;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::timeout;

struct GatedChildMetadataStore {
    inner: Arc<dyn ThreadStore>,
    gates: Mutex<HashMap<ThreadId, oneshot::Receiver<()>>>,
    failed_child: ThreadId,
    started: mpsc::UnboundedSender<ThreadId>,
    completed: mpsc::UnboundedSender<ThreadId>,
}

macro_rules! delegate_store_methods {
    ($(fn $name:ident($param:ident: $params:ty) -> $result:ty;)*) => {
        $(fn $name(&self, $param: $params) -> ThreadStoreFuture<'_, $result> {
            self.inner.$name($param)
        })*
    };
}

impl ThreadStore for GatedChildMetadataStore {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn default_history_mode(&self) -> ThreadHistoryMode {
        self.inner.default_history_mode()
    }

    delegate_store_methods! {
        fn create_thread(params: CreateThreadParams) -> ();
        fn resume_thread(params: ResumeThreadParams) -> ();
        fn append_items(params: AppendThreadItemsParams) -> ();
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

    fn persist_thread(
        &self,
        thread_id: ThreadId,
        context: PersistContext,
    ) -> ThreadStoreFuture<'_, ()> {
        self.inner.persist_thread(thread_id, context)
    }

    fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreFuture<'_, StoredThread> {
        Box::pin(async move {
            let thread_id = params.thread_id;
            let gate = self
                .gates
                .lock()
                .expect("child read gates")
                .remove(&thread_id);
            let Some(gate) = gate else {
                return self.inner.read_thread(params).await;
            };
            assert!(!params.include_history, "restoration only reads metadata");
            self.started.send(thread_id).expect("read started receiver");
            gate.await.expect("release child read");

            let result = if thread_id == self.failed_child {
                Err(ThreadStoreError::Internal {
                    message: "injected child metadata failure".to_owned(),
                })
            } else {
                self.inner.read_thread(params).await.map(|mut thread| {
                    // Only one child can register this path, exposing which result is applied first.
                    thread.agent_path = Some("/root/restored".to_owned());
                    thread
                })
            };
            self.completed
                .send(thread_id)
                .expect("read completed receiver");
            result
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cold_root_resume_overlaps_child_reads_and_applies_identities_in_graph_order() -> Result<()>
{
    let server = start_mock_server().await;
    let initial_url = format!("{}/v1", server.uri());
    let initial = test_codex()
        .with_config(move |config| {
            configure_multi_agent_v2_with_role(config, &initial_url);
            config
                .features
                .enable(Feature::Sqlite)
                .expect("enable SQLite");
            config.multi_agent_v2.max_concurrent_threads_per_session = 4;
        })
        .build_with_auto_env(&server)
        .await?;
    let root = initial.session_configured.thread_id;

    for (name, prompt, call_id, task) in [
        ("alpha", "start alpha", "spawn-alpha", "alpha child task"),
        ("beta", "start beta", "spawn-beta", "beta child task"),
        ("gamma", "start gamma", "spawn-gamma", "gamma child task"),
    ] {
        mount_root_collaboration_call(
            &server,
            prompt,
            call_id,
            "spawn_agent",
            &json!({ "message": task, "task_name": name, "agent_type": ROLE_NAME, "fork_turns": "none" })
                .to_string(),
        )
        .await;
        mount_sse_once_match(
            &server,
            move |request: &wiremock::Request| {
                request_has_model(request, ROLE_MODEL)
                    && request_has_input_type(request, "agent_message")
                    && body_contains(request, task)
            },
            sse(vec![ev_completed(&format!("{call_id}-child"))]),
        )
        .await;
        initial.submit_turn(prompt).await?;
    }

    let subtree = initial
        .thread_manager
        .list_agent_subtree_thread_ids(root)
        .await?;
    let [listed_root, first, failed, last] = subtree.as_slice() else {
        anyhow::bail!("expected a root and three durable children: {subtree:?}");
    };
    let [first, failed, last] = [*first, *failed, *last];
    assert_eq!(*listed_root, root);
    let mut ordered_children = vec![first, failed, last];
    ordered_children.sort_by_key(ToString::to_string);
    assert_eq!(ordered_children, vec![first, failed, last]);
    for child_id in [first, failed, last] {
        let child = initial.thread_manager.get_thread(child_id).await?;
        wait_for_event(child.as_ref(), |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        child.flush_rollout().await?;
        child.shutdown_and_wait().await?;
    }
    initial.codex.flush_rollout().await?;

    let (started_tx, mut started) = mpsc::unbounded_channel();
    let (completed_tx, mut completed) = mpsc::unbounded_channel();
    let (first_release, first_gate) = oneshot::channel();
    let (failed_release, failed_gate) = oneshot::channel();
    let (last_release, last_gate) = oneshot::channel();
    let store = Arc::new(GatedChildMetadataStore {
        inner: Arc::clone(&initial.thread_store),
        gates: Mutex::new(HashMap::from([
            (first, first_gate),
            (failed, failed_gate),
            (last, last_gate),
        ])),
        failed_child: failed,
        started: started_tx,
        completed: completed_tx,
    });
    let resume_url = format!("{}/v1", server.uri());
    let mut resume_builder = test_codex()
        .with_thread_store(store)
        .with_config(move |config| {
            configure_multi_agent_v2_with_role(config, &resume_url);
            config
                .features
                .enable(Feature::Sqlite)
                .expect("enable SQLite");
            config.multi_agent_v2.max_concurrent_threads_per_session = 4;
        });

    let (resumed, ()) = tokio::try_join!(resume_builder.restart(&server, &initial), async {
        timeout(Duration::from_secs(10), async {
            // Releasing nothing until all reads start makes a serial implementation fail.
            let mut reads = Vec::new();
            for _ in 0..3 {
                reads.push(started.recv().await.context("child read started")?);
            }
            reads.sort_by_key(ToString::to_string);
            assert_eq!(reads, ordered_children);

            last_release.send(()).expect("release last child");
            assert_eq!(completed.recv().await, Some(last));
            failed_release.send(()).expect("release failed child");
            assert_eq!(completed.recv().await, Some(failed));
            first_release.send(()).expect("release first child");
            assert_eq!(completed.recv().await, Some(first));
            Ok::<(), anyhow::Error>(())
        })
        .await
        .context("child metadata reads should overlap")?
    })?;

    assert_eq!(resumed.thread_manager.list_thread_ids().await, vec![root]);
    for child_id in [last, failed] {
        assert!(matches!(
            resumed.thread_manager.ensure_multi_agent_v2_child_loaded(child_id).await,
            Err(error) if matches!(error.details(), CodexErrorDetails::ThreadNotFound(id) if *id == child_id)
        ));
    }
    resumed
        .thread_manager
        .ensure_multi_agent_v2_child_loaded(first)
        .await?;
    assert!(resumed.thread_manager.get_thread(first).await.is_ok());
    assert!(resumed.thread_manager.get_thread(last).await.is_err());
    assert!(resumed.thread_manager.get_thread(failed).await.is_err());
    Ok(())
}
