//! Persists explicit user goal mutations with user provenance before goal work resumes.
//! Unloaded threads receive the same item on replay; tool-created goals never use this path.

use super::*;
use codex_core::context::ContextualUserFragment;
use codex_core::context::UserGoalUpdate;
use codex_rollout::RolloutRecorderParams;

impl ThreadGoalRequestProcessor {
    pub(super) async fn record_user_goal_update(
        &self,
        thread_id: ThreadId,
        update: UserGoalUpdate,
    ) -> Result<(), GoalServiceError> {
        if let Ok(thread) = self.thread_manager.get_thread(thread_id).await {
            return thread.record_user_goal_update(update).await.map_err(|err| {
                GoalServiceError::Internal(format!("failed to record goal instruction: {err}"))
            });
        }
        let item = ContextualUserFragment::into(update);
        let record_error =
            |err| GoalServiceError::Internal(format!("failed to record goal instruction: {err}"));
        let writer_lock = Arc::new(self.writer_locks.acquire(thread_id).map_err(record_error)?);
        let path = codex_rollout::find_thread_path_by_id_str(
            &self.config.codex_home,
            &thread_id.to_string(),
            self.state_db.as_deref(),
        )
        .await
        .map_err(|err| {
            GoalServiceError::Internal(format!("failed to locate thread id {thread_id}: {err}"))
        })?
        .ok_or_else(|| {
            GoalServiceError::InvalidRequest(format!("thread not found: {thread_id}"))
        })?;
        // The writer owns the lock through background IO, even if this RPC is cancelled.
        let recorder = RolloutRecorder::new_with_writer_lock(
            self.config.as_ref(),
            RolloutRecorderParams::resume(path),
            writer_lock,
        )
        .await
        .map_err(record_error)?;
        recorder
            .record_canonical_items(&[RolloutItem::ResponseItem(item.into())])
            .await
            .map_err(record_error)?;
        // Shutdown flushes queued items before acknowledging the goal mutation.
        recorder.shutdown().await.map_err(record_error)
    }
}
