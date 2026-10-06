//! Owns a spawned child until its initial input is accepted.

use super::AgentTreeMembership;
use crate::thread_manager::ThreadManagerState;
use crate::thread_manager::thread_store_error_kind;
use codex_agent_graph_store::AgentGraphStoreError;
use codex_agent_graph_store::ThreadSpawnEdgeStatus;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErrKind;
use codex_thread_store::ThreadStoreError;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::warn;

pub(super) struct PendingSpawn {
    state: Arc<ThreadManagerState>,
    pending: Option<(ThreadId, AgentTreeMembership)>,
    edge_write: Option<JoinHandle<()>>,
}

impl PendingSpawn {
    pub(super) fn new(
        state: Arc<ThreadManagerState>,
        child: ThreadId,
        membership: AgentTreeMembership,
    ) -> Self {
        Self {
            state,
            pending: Some((child, membership)),
            edge_write: None,
        }
    }

    pub(super) fn set_edge_write(&mut self, edge_write: JoinHandle<()>) {
        self.edge_write = Some(edge_write);
    }

    pub(super) async fn wait_for_edge(&mut self) {
        if let Some(edge_write) = self.edge_write.as_mut() {
            assert!(
                edge_write.await.is_ok(),
                "spawn edge write task should complete"
            );
        }
        self.edge_write = None;
    }

    pub(super) fn disarm(mut self) -> AgentTreeMembership {
        let Some((_child, membership)) = self.pending.take() else {
            unreachable!("pending spawn must own agent-tree membership");
        };
        membership
    }
}

impl Drop for PendingSpawn {
    fn drop(&mut self) {
        let Some((child, membership)) = self.pending.take() else {
            return;
        };
        let state = Arc::clone(&self.state);
        let teardown = membership.into_teardown_guard("child_spawn_cleanup", Some(child));
        let edge_write = self.edge_write.take();
        drop(tokio::spawn(async move {
            if let Some(thread) = state.remove_thread(&child).await {
                if let Err(error) = thread.shutdown_and_wait().await {
                    teardown
                        .record_shutdown_failure("stop_child", CodexErrKind::from(&error).into());
                    warn!("failed to stop cancelled child spawn: {error}");
                }
                if let Some(live_thread) = thread.session.live_thread() {
                    match live_thread.discard().await {
                        Ok(()) | Err(ThreadStoreError::ThreadNotFound { .. }) => {}
                        Err(error) => {
                            teardown.record_shutdown_failure(
                                "discard_child_persistence",
                                thread_store_error_kind(&error),
                            );
                            warn!("failed to discard cancelled child spawn: {error}");
                        }
                    }
                }
            }
            // A pending Open write must finish before cleanup writes Closed.
            if let Some(edge_write) = edge_write {
                let _ = edge_write.await;
            }
            if let Some(store) = state.agent_graph_store()
                && let Err(error) = store
                    .set_thread_spawn_edge_status(child, ThreadSpawnEdgeStatus::Closed)
                    .await
            {
                let error_kind = match &error {
                    AgentGraphStoreError::InvalidRequest { .. } => {
                        "agent_graph_store_invalid_request"
                    }
                    AgentGraphStoreError::Internal { .. } => "agent_graph_store_internal",
                };
                teardown.record_shutdown_failure("close_spawn_edge", error_kind);
                warn!("failed to close cancelled child spawn edge: {error}");
            }
            teardown.complete();
        }));
    }
}
