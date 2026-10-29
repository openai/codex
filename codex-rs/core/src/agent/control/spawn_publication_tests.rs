#![cfg(test)]

//! Exercises spawn cleanup against real SQLite graph writes with controlled acknowledgements.
//! Publication must survive caller cancellation and finish before cleanup closes the edge.

use super::*;
use codex_agent_graph_store::AgentGraphStore;
use codex_agent_graph_store::AgentGraphStoreError;
use codex_agent_graph_store::AgentGraphStoreFuture;
use codex_agent_graph_store::LocalAgentGraphStore;
use codex_agent_graph_store::ThreadSpawnEdgeStatus;
use pretty_assertions::assert_eq;

#[derive(Clone, Copy)]
enum SpawnEdgeFailure {
    CancelledBeforeCommit,
    CancelledBeforeAck,
    ErrorAfterCommit,
}

enum SkippedPublication {
    Ephemeral,
    NoParent,
}

struct FaultingGraphStore {
    inner: LocalAgentGraphStore,
    failure: SpawnEdgeFailure,
    started: CancellationToken,
    held: CancellationToken,
    release: CancellationToken,
    cancelled: CancellationToken,
    close_attempted: CancellationToken,
}

impl FaultingGraphStore {
    async fn hold_write(&self) {
        self.held.cancel();
        self.release.cancelled().await;
    }
}

impl AgentGraphStore for FaultingGraphStore {
    fn upsert_thread_spawn_edge(
        &self,
        parent: ThreadId,
        child: ThreadId,
        status: ThreadSpawnEdgeStatus,
    ) -> AgentGraphStoreFuture<'_, ()> {
        Box::pin(async move {
            let cancellation_guard = self.cancelled.clone().drop_guard();
            self.started.cancel();
            if matches!(self.failure, SpawnEdgeFailure::CancelledBeforeCommit) {
                self.hold_write().await;
            }
            self.inner
                .upsert_thread_spawn_edge(parent, child, status)
                .await?;
            if matches!(self.failure, SpawnEdgeFailure::CancelledBeforeAck) {
                self.hold_write().await;
            }
            cancellation_guard.disarm();
            match self.failure {
                SpawnEdgeFailure::CancelledBeforeCommit | SpawnEdgeFailure::CancelledBeforeAck => {
                    Ok(())
                }
                SpawnEdgeFailure::ErrorAfterCommit => Err(AgentGraphStoreError::Internal {
                    message: "injected response loss after successful commit".to_string(),
                }),
            }
        })
    }

    fn set_thread_spawn_edge_status(
        &self,
        child: ThreadId,
        status: ThreadSpawnEdgeStatus,
    ) -> AgentGraphStoreFuture<'_, ()> {
        self.close_attempted.cancel();
        self.inner.set_thread_spawn_edge_status(child, status)
    }

    fn list_thread_spawn_children(
        &self,
        parent: ThreadId,
        status: Option<ThreadSpawnEdgeStatus>,
    ) -> AgentGraphStoreFuture<'_, Vec<ThreadId>> {
        self.inner.list_thread_spawn_children(parent, status)
    }

    fn list_thread_spawn_descendants(
        &self,
        root: ThreadId,
        status: Option<ThreadSpawnEdgeStatus>,
    ) -> AgentGraphStoreFuture<'_, Vec<ThreadId>> {
        self.inner.list_thread_spawn_descendants(root, status)
    }
}

async fn publication_harness(
    failure: SpawnEdgeFailure,
    generate_thread_id: impl Fn() -> ThreadId + Send + Sync + 'static,
) -> (AgentControlHarness, Arc<FaultingGraphStore>) {
    let mut harness = AgentControlHarness::new().await;
    let graph = Arc::new(FaultingGraphStore {
        inner: LocalAgentGraphStore::new(harness.state_db.clone().expect("state db")),
        failure,
        started: CancellationToken::new(),
        held: CancellationToken::new(),
        release: CancellationToken::new(),
        cancelled: CancellationToken::new(),
        close_attempted: CancellationToken::new(),
    });
    harness.manager = ThreadManager::new(
        &harness.config,
        harness.manager.auth_manager(),
        harness.manager.get_models_manager(),
        crate::CodexAppsToolsCache::default(),
        SessionSource::Exec,
        harness.manager.environment_manager(),
        empty_extension_registry(),
        Arc::new(crate::test_support::EmptyUserInstructionsProvider),
        /*analytics_events_client*/ None,
        harness.manager.image_store(),
        crate::thread_manager::thread_store_from_config(&harness.config, harness.state_db.clone()),
        Some(graph.clone()),
        "spawn-publication-cleanup-test".to_string(),
        /*attestation_provider*/ None,
        /*external_time_provider*/ None,
    )
    .with_thread_id_generator(generate_thread_id);
    harness.control = harness.manager.agent_control();
    (harness, graph)
}

async fn spawn_publication_child(
    harness: &AgentControlHarness,
    parent: ThreadId,
) -> CodexResult<ThreadId> {
    harness
        .control
        .spawn_agent(
            harness.config.clone(),
            text_input("child task"),
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: parent,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            })),
        )
        .await
}

async fn assert_publication_closed(
    harness: &AgentControlHarness,
    graph: &FaultingGraphStore,
    parent: ThreadId,
) {
    harness
        .control
        .runtime
        .request_shutdown()
        .wait()
        .await
        .expect("cleanup completes");
    let all = graph
        .list_thread_spawn_children(parent, /*status_filter*/ None)
        .await
        .expect("graph readable");
    assert_eq!(
        all.len(),
        1,
        "the store durably accepted the prepublication"
    );
    assert_thread_not_loaded(&harness.manager, all[0]).await;
    assert_eq!(
        graph
            .list_thread_spawn_children(parent, Some(ThreadSpawnEdgeStatus::Open))
            .await
            .expect("open graph"),
        Vec::<ThreadId>::new(),
        "rejected or cancelled spawn must close its committed edge",
    );
    assert_eq!(
        graph
            .list_thread_spawn_children(parent, Some(ThreadSpawnEdgeStatus::Closed))
            .await
            .expect("closed graph"),
        all,
    );
}

#[test_case::test_case(SpawnEdgeFailure::CancelledBeforeCommit, 0; "before_commit")]
#[test_case::test_case(SpawnEdgeFailure::CancelledBeforeAck, 1; "before_acknowledgement")]
#[tokio::test]
async fn cancelled_spawn_edge_publication_cleanup(
    failure: SpawnEdgeFailure,
    committed_children: usize,
) {
    let (harness, graph) = publication_harness(failure, ThreadId::new).await;
    let parent = ThreadId::new();
    let mut spawn = Box::pin(spawn_publication_child(&harness, parent));
    tokio::select! {
        biased;
        () = graph.held.cancelled() => {},
        result = &mut spawn => panic!("spawn returned while publication was held: {result:?}"),
    }
    assert_eq!(
        graph
            .list_thread_spawn_children(parent, /*status_filter*/ None)
            .await
            .expect("graph before cancellation")
            .len(),
        committed_children
    );
    drop(spawn);
    // Dropping a directly-awaited store future cancels this token synchronously.
    // Keeping only the child ID in the guard cannot satisfy this ownership check.
    assert!(
        !graph.cancelled.is_cancelled(),
        "caller cancellation must retain the submitted write"
    );
    // Paused time advances only after runnable cleanup has been polled. No real
    // storage operation is pending while the graph-store future is held.
    tokio::time::pause();
    let early_close = timeout(Duration::from_secs(1), graph.close_attempted.cancelled()).await;
    tokio::time::resume();
    assert!(early_close.is_err(), "Closed must wait for Open");
    graph.release.cancel();
    assert_publication_closed(&harness, &graph, parent).await;
}

#[tokio::test]
async fn failed_spawn_edge_publication_cleanup() {
    let (harness, graph) =
        publication_harness(SpawnEdgeFailure::ErrorAfterCommit, ThreadId::new).await;
    let parent = ThreadId::new();
    let error = spawn_publication_child(&harness, parent)
        .await
        .expect_err("lost commit acknowledgement rejects spawn");
    assert_eq!(
        error.to_string(),
        "Fatal error: failed to persist spawned agent relationship"
    );
    assert_publication_closed(&harness, &graph, parent).await;
}

#[test_case::test_case(SkippedPublication::Ephemeral; "ephemeral")]
#[test_case::test_case(SkippedPublication::NoParent; "no_parent")]
#[tokio::test]
async fn failed_spawn_without_publication_does_not_close_graph(skipped: SkippedPublication) {
    let (mut harness, graph) =
        publication_harness(SpawnEdgeFailure::ErrorAfterCommit, ThreadId::new).await;
    let parent = ThreadId::new();
    let source = match skipped {
        SkippedPublication::Ephemeral => {
            harness.config.ephemeral = true;
            Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: parent,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            }))
        }
        SkippedPublication::NoParent => None,
    };
    let state = harness.control.runtime.upgrade().expect("live manager");
    let spawn =
        harness
            .control
            .spawn_agent(harness.config.clone(), text_input("child task"), source);
    tokio::pin!(spawn);
    let shutdown = {
        let _threads = state.threads.write().await;
        assert!(
            spawn
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        harness.control.runtime.request_shutdown()
    };
    assert_eq!(
        CodexErrKind::from(&spawn.await.expect_err("child startup rejects shutdown")),
        CodexErrKind::InvalidRequest,
    );
    shutdown
        .wait()
        .await
        .expect("cleanup completes without graph errors");
    assert!(!graph.started.is_cancelled(), "no Open write was issued");
    assert!(
        !graph.close_attempted.is_cancelled(),
        "skipped publication needs no Closed write"
    );
    assert_eq!(
        graph
            .list_thread_spawn_children(parent, /*status_filter*/ None)
            .await
            .expect("graph readable"),
        Vec::<ThreadId>::new()
    );
}

#[tokio::test]
async fn repeated_child_id_preserves_existing_thread_and_graph() {
    let repeated_id = ThreadId::new();
    let (harness, graph) =
        publication_harness(SpawnEdgeFailure::ErrorAfterCommit, move || repeated_id).await;
    let (parent, existing_thread) = harness.start_thread().await;
    assert_eq!(parent, repeated_id);
    let children_before = graph
        .list_thread_spawn_children(parent, /*status_filter*/ None)
        .await
        .expect("initial graph");
    let error = spawn_publication_child(&harness, parent)
        .await
        .expect_err("loaded IDs are rejected");
    assert_eq!(
        (error.agent_context(), CodexErrKind::from(&error)),
        (
            Some(AgentErrorContext::ChildStartup),
            CodexErrKind::InvalidRequest
        ),
    );
    assert!(Arc::ptr_eq(
        &existing_thread,
        &harness
            .manager
            .get_thread(parent)
            .await
            .expect("existing thread survives")
    ));
    assert!(
        !graph.started.is_cancelled(),
        "duplicate ID must be rejected before publication"
    );
    assert!(
        !graph.close_attempted.is_cancelled(),
        "existing thread is not provisional cleanup"
    );
    assert_eq!(
        graph
            .list_thread_spawn_children(parent, /*status_filter*/ None)
            .await
            .expect("graph after rejection"),
        children_before
    );
    existing_thread
        .shutdown_and_wait()
        .await
        .expect("stop existing thread");
}
