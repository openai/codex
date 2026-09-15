//! Selects persistent root threads for managed daemon recovery using normal resume semantics.
//! Only threads successfully persisted through the existing thread store become candidates.

use super::ThreadRequestProcessor;
use codex_app_server_transport::daemon_recovery::InterruptedTurn;
use codex_app_server_transport::daemon_recovery::RecoverySnapshot;
use codex_thread_store::PersistContext;
use tracing::warn;

impl ThreadRequestProcessor {
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "snapshot selection must be serialized against pending unloads"
    )]
    pub(crate) async fn daemon_recovery_snapshot(&self) -> RecoverySnapshot {
        let mut snapshot = RecoverySnapshot::default();
        for thread_id in self.thread_manager.list_thread_ids().await {
            let pending_unloads = self.pending_thread_unloads.lock().await;
            if pending_unloads.contains(&thread_id) {
                continue;
            }
            let Ok(thread) = self.thread_manager.get_thread(thread_id).await else {
                continue;
            };
            let config = thread.config_snapshot().await;
            if !config.ephemeral
                && config.parent_thread_id.is_none()
                && !config.session_source.is_non_root_agent()
            {
                let interrupted = thread.interrupted_turn().await;
                if let Err(err) = self
                    .thread_store
                    .persist_thread(thread_id, PersistContext::Standard)
                    .await
                {
                    warn!(%thread_id, %err, "skipping daemon restore for thread that could not be persisted");
                    continue;
                }
                snapshot.loaded.insert(thread_id.to_string());
                if let Some((turn_id, options)) = interrupted {
                    snapshot.interrupted.insert(
                        thread_id.to_string(),
                        InterruptedTurn {
                            turn_id,
                            output_schema: options.final_output_json_schema,
                            service_tier: options.service_tier,
                            cyber_access_program: options.cyber_access_program,
                        },
                    );
                }
            }
        }
        snapshot
    }
}
