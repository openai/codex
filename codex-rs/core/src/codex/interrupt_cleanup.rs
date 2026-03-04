use super::*;

use tracing::warn;

impl Session {
    pub(crate) async fn close_unified_exec_processes(&self) {
        self.services
            .unified_exec_manager
            .terminate_all_processes()
            .await;
    }

    pub(crate) async fn cleanup_after_interrupt(&self, turn_context: &Arc<TurnContext>) {
        self.close_unified_exec_processes().await;

        if let Some(manager) = turn_context.js_repl.manager_if_initialized()
            && let Err(err) = manager.interrupt_turn_exec(&turn_context.sub_id).await
        {
            warn!("failed to interrupt js_repl kernel: {err}");
        }
    }
}
