//! Turn-start MCP reconciliation and cooperative discovery cancellation regressions.

use super::*;
use codex_mcp::McpResourceClient;
use codex_mcp::McpResourceClientCacheKey;
use pretty_assertions::assert_eq;

struct Recorder {
    client: McpResourceClient,
    requires_mcp: std::sync::atomic::AtomicBool,
    keys: std::sync::Mutex<Vec<McpResourceClientCacheKey>>,
    discovered: tokio::sync::Notify,
}

impl codex_extension_api::TurnLifecycleContributor for Recorder {
    fn turn_start_phase(
        &self,
        _thread_store: &codex_extension_api::ExtensionData,
    ) -> codex_extension_api::TurnStartPhase {
        codex_extension_api::TurnStartPhase::RegularTaskStart
    }

    fn requires_mcp_runtime(&self, _thread_store: &codex_extension_api::ExtensionData) -> bool {
        self.requires_mcp.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn on_turn_start<'a>(
        &'a self,
        _input: codex_extension_api::TurnStartInput<'a>,
    ) -> codex_extension_api::ExtensionFuture<'a, ()> {
        Box::pin(async move {
            self.keys
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(self.client.cache_key());
            self.discovered.notify_one();
            std::future::pending::<()>().await;
        })
    }
}

#[tokio::test]
async fn turn_start_refreshes_dirty_mcp_reuses_clean_runtime_and_cancels_discovery() {
    let (mut session, turn_context) = make_session_and_context().await;
    let recorder = Arc::new(Recorder {
        client: McpResourceClient::new(Arc::clone(&session.services.mcp_runtime)),
        requires_mcp: Default::default(),
        keys: Default::default(),
        discovered: Default::default(),
    });
    let initial_key = recorder.client.cache_key();
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_lifecycle_contributor(recorder.clone());
    session.services.extensions = Arc::new(builder.build());
    let session = Arc::new(session);
    let turn_context = Arc::new(turn_context);

    for turn in 0..4 {
        recorder
            .requires_mcp
            .store(turn > 0, std::sync::atomic::Ordering::Relaxed);
        if turn == 0 {
            session.mark_mcp_runtime_dirty();
        } else if turn == 3 {
            session.services.mcp_runtime.reconnect_on_next_refresh();
            session.request_mcp_runtime_refresh();
        }
        session
            .spawn_task(
                Arc::clone(&turn_context),
                Vec::new(),
                crate::tasks::RegularTask::new(),
            )
            .await;
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            recorder.discovered.notified(),
        )
        .await
        .expect("regular-task contributor should run");
        assert_eq!(session.mcp_refresh.is_pending(), turn == 0);
        assert_eq!(
            recorder
                .keys
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len(),
            turn + 1
        );
        if turn > 0 {
            session.refresh_mcp_if_dirty().await;
            assert!(
                recorder.client.cache_key()
                    == recorder
                        .keys
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)[turn],
                "a clean refresh must reuse the published MCP runtime"
            );
        }
        let (cancellation_token, done) = {
            let active_turn = session.active_turn.lock().await;
            let task = active_turn
                .as_ref()
                .and_then(|turn| turn.task.as_ref())
                .expect("discovery should run inside the registered task");
            (task.cancellation_token.clone(), Arc::clone(&task.done))
        };
        let completed = done.notified();
        tokio::pin!(completed);
        completed.as_mut().enable();
        cancellation_token.cancel();
        tokio::time::timeout(std::time::Duration::from_secs(5), completed)
            .await
            .expect("discovery cancellation must finish the task without forced abort");
        assert!(
            session.mcp_refresh.is_pending(),
            "cancelled discovery must request an MCP refresh before the task completes"
        );
        session.abort_all_tasks(TurnAbortReason::Interrupted).await;
    }
    let keys = recorder
        .keys
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(
        keys[0] == initial_key,
        "unrelated contributors must not reconcile MCP"
    );
    assert!(
        keys[1] != initial_key,
        "discovery must observe the reconciled generation"
    );
    assert!(
        keys[2] != keys[1],
        "cancelled cloud preparation must request reprojection on the next turn"
    );
    assert!(
        keys[3] != keys[2],
        "the next turn must consume a pending reconnect before discovery"
    );
}
