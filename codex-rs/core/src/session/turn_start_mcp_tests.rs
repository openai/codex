//! Concurrent catalog discovery retains publication and releases preparation on cancellation.

use super::*;
use crate::McpRefreshReason;
use codex_mcp::McpResourceClient;
use codex_mcp::McpResourceClientCacheKey;
use pretty_assertions::assert_eq;

struct Recorder {
    client: McpResourceClient,
    ready: Arc<tokio::sync::Barrier>,
    pause: std::sync::atomic::AtomicBool,
    keys: std::sync::Mutex<Vec<McpResourceClientCacheKey>>,
    discovered: tokio::sync::Notify,
}

impl codex_extension_api::TurnLifecycleContributor for Recorder {
    fn refresh_catalog_for_step<'a>(
        &'a self,
        _input: codex_extension_api::CatalogRefreshInput<'a>,
    ) -> codex_extension_api::ExtensionFuture<'a, bool> {
        Box::pin(async move {
            self.keys
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(self.client.cache_key());
            self.ready.wait().await;
            self.discovered.notify_one();
            if self.pause.swap(false, std::sync::atomic::Ordering::Relaxed) {
                std::future::pending::<()>().await;
            }
            false
        })
    }
}

/// Step discovery overlaps providers on ready connections; cancellation releases the catalog lock.
#[tokio::test]
async fn step_discovery_reconciles_mcp_skips_prewarm_and_releases_lock_on_cancellation() {
    let (mut session, turn_context) = make_session_and_context().await;
    let ready = Arc::new(tokio::sync::Barrier::new(2));
    let recorder = Arc::new(Recorder {
        client: McpResourceClient::new(Arc::clone(&session.services.mcp_runtime)),
        ready: Arc::clone(&ready),
        pause: std::sync::atomic::AtomicBool::new(true),
        keys: Default::default(),
        discovered: Default::default(),
    });
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_lifecycle_contributor(recorder.clone());
    builder.turn_lifecycle_contributor(Arc::new(Recorder {
        client: recorder.client.clone(),
        ready,
        pause: std::sync::atomic::AtomicBool::new(false),
        keys: Default::default(),
        discovered: Default::default(),
    }));
    session.services.extensions = Arc::new(builder.build());
    let session = Arc::new(session);
    let turn_context = Arc::new(turn_context);
    let prewarm = session
        .new_startup_prewarm_turn_with_sub_id(INITIAL_SUBMIT_ID.to_owned())
        .await;
    session
        .mcp_runtime_for_step(&prewarm, &[], &[], &HashSet::new())
        .await;
    assert!(recorder.keys.lock().expect("recorded keys").is_empty());
    let initial_key = recorder.client.cache_key();

    session
        .spawn_task(
            Arc::clone(&turn_context),
            Vec::new(),
            HeldStepTask {
                kind: TaskKind::Regular,
                finish: Arc::new(tokio::sync::Notify::new()),
            },
        )
        .await;
    session.mark_mcp_runtime_dirty(McpRefreshReason::ExplicitInvalidation);
    let cancellation = CancellationToken::new();
    let capture = session.capture_step_context(Arc::clone(&turn_context), &cancellation);
    tokio::pin!(capture);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::select! {
            _ = &mut capture => panic!("step must wait for discovery"),
            () = recorder.discovered.notified() => {}
        }
    })
    .await
    .expect("regular step should discover catalogs");
    assert!(!session.mcp_refresh.is_pending());
    let prepared_key = recorder.client.cache_key();
    assert!(prepared_key != initial_key);
    assert!(recorder.keys.lock().expect("recorded keys")[0] == prepared_key);

    // Prewarm also skips discovery while a regular turn is preparing a step.
    session
        .mcp_runtime_for_step(&prewarm, &[], &[], &HashSet::new())
        .await;
    assert_eq!(recorder.keys.lock().expect("recorded keys").len(), 1);
    cancellation.cancel();
    assert!(capture.await.is_err());

    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        session.capture_step_context(Arc::clone(&turn_context), &CancellationToken::new()),
    )
    .await
    .expect("cancelled discovery must release the catalog lock")
    .expect("next step should prepare");
    assert!(recorder.client.cache_key() == prepared_key);

    session.services.mcp_runtime.reconnect_on_next_refresh();
    session.request_mcp_runtime_refresh(McpRefreshReason::ExplicitInvalidation);
    session
        .capture_step_context(turn_context, &CancellationToken::new())
        .await
        .expect("step should consume pending reconnect before discovery");
    let keys = recorder.keys.lock().expect("recorded keys").clone();
    assert_eq!(keys.len(), 3);
    assert!(keys[1] == prepared_key);
    assert!(keys[2] != prepared_key);
    assert!(keys[2] == recorder.client.cache_key());
    session.abort_all_tasks(TurnAbortReason::Interrupted).await;
}

/// Explicit refresh overlaps providers under one lock and retains publication while a sibling waits.
#[tokio::test]
async fn explicit_cloud_refresh_runs_concurrently_and_awaits_all_results() {
    struct CatalogGate {
        entered: Arc<tokio::sync::Barrier>,
        finish: tokio::sync::Notify,
        result: Result<(), &'static str>,
    }

    impl codex_extension_api::TurnLifecycleContributor for CatalogGate {
        fn refresh_cloud_catalog<'a>(
            &'a self,
            _input: codex_extension_api::CloudCatalogRefreshInput<'a>,
        ) -> codex_extension_api::ExtensionFuture<'a, Result<(), String>> {
            Box::pin(async move {
                self.entered.wait().await;
                self.finish.notified().await;
                self.result.map_err(str::to_string)
            })
        }
    }

    let (mut session, _) = make_session_and_context().await;
    let entered = Arc::new(tokio::sync::Barrier::new(3));
    let failing = Arc::new(CatalogGate {
        entered: Arc::clone(&entered),
        finish: Default::default(),
        result: Err("plugin discovery failed"),
    });
    let succeeding = Arc::new(CatalogGate {
        entered: Arc::clone(&entered),
        finish: Default::default(),
        result: Ok(()),
    });
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_lifecycle_contributor(failing.clone());
    builder.turn_lifecycle_contributor(succeeding.clone());
    session.services.extensions = Arc::new(builder.build());
    let session = Arc::new(session);
    let mut refresh = Box::pin(session.refresh_cloud_plugins());
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::select! {
            result = &mut refresh => panic!("refresh completed before both providers started: {result:?}"),
            _ = entered.wait() => {}
        }
    })
    .await
    .expect("both providers must start before either finishes");
    assert!(!session.mcp_refresh.is_pending());

    let mut lock = Box::pin(session.lock_cloud_catalogs());
    assert!(futures::poll!(&mut lock).is_pending());
    drop(lock);

    failing.finish.notify_one();
    assert!(futures::poll!(&mut refresh).is_pending());
    assert!(session.mcp_refresh.is_pending());
    succeeding.finish.notify_one();
    assert_eq!(
        refresh.await.expect_err("one provider failed").to_string(),
        "plugin discovery failed"
    );
    drop(
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            session.lock_cloud_catalogs(),
        )
        .await
        .expect("completed refresh must release the catalog lock"),
    );
}
