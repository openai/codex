//! Step discovery observes ready MCP connections; prewarm skips discovery and cancellation releases preparation.

use super::*;
use crate::McpRefreshReason;
use codex_mcp::McpResourceClient;
use codex_mcp::McpResourceClientCacheKey;
use pretty_assertions::assert_eq;

struct Recorder {
    client: McpResourceClient,
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
            self.discovered.notify_one();
            if self.pause.swap(false, std::sync::atomic::Ordering::Relaxed) {
                std::future::pending::<()>().await;
            }
            false
        })
    }
}

#[tokio::test]
async fn step_discovery_reconciles_mcp_skips_prewarm_and_cancels_preparation() {
    let (mut session, turn_context) = make_session_and_context().await;
    let recorder = Arc::new(Recorder {
        client: McpResourceClient::new(Arc::clone(&session.services.mcp_runtime)),
        pause: std::sync::atomic::AtomicBool::new(true),
        keys: Default::default(),
        discovered: Default::default(),
    });
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_lifecycle_contributor(recorder.clone());
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
    .expect("cancelled discovery must allow the next step")
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
