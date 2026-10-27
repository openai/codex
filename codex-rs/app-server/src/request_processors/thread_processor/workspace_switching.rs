//! Unloads old-workspace runtimes while turn admission is paused by the account switch.
//! Resuming a thread builds fresh policy and transport state for the new workspace.

use super::*;

impl ThreadRequestProcessor {
    pub(crate) async fn unload_for_workspace_switch(&self) -> Result<(), JSONRPCErrorError> {
        let report = self
            .thread_manager
            .shutdown_all_threads_bounded(Duration::from_secs(/*secs*/ 10))
            .await;
        for thread_id in report.completed {
            self.finalize_thread_teardown(thread_id).await;
            self.outgoing
                .send_server_notification(ServerNotification::ThreadClosed(
                    codex_app_server_protocol::ThreadClosedNotification {
                        thread_id: thread_id.to_string(),
                    },
                ))
                .await;
        }
        // A delegated child may have started while its parent was shutting down.
        // Never replace credentials until every runtime has stopped.
        if !report.submit_failed.is_empty()
            || !report.timed_out.is_empty()
            || !self.thread_manager.list_thread_ids().await.is_empty()
        {
            return Err(internal_error(
                "Some threads are still running; retry the workspace switch",
            ));
        }
        Ok(())
    }
}
