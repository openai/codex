use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::request_processors::thread_processor::ThreadRequestProcessor;
use codex_rollout::StateDbHandle;
use codex_state::AUTOMATION_DISPATCH_BATCH_SIZE;
use codex_state::AUTOMATION_POLL_INTERVAL;
use codex_state::AutomationDispatchClaim;
use tracing::warn;

impl ThreadRequestProcessor {
    pub(crate) fn spawn_automation_worker(&self) {
        let (Some(state_db), Some(worker_id), Some(shutdown)) = (
            self.state_db.clone(),
            self.automation_worker_id.clone(),
            self.automation_worker_shutdown.clone(),
        ) else {
            return;
        };

        let processor = self.clone();
        self.background_tasks.spawn(async move {
            processor
                .run_automation_worker(state_db, worker_id, shutdown)
                .await;
        });
    }

    async fn run_automation_worker(
        self,
        state_db: StateDbHandle,
        worker_id: String,
        shutdown: tokio_util::sync::CancellationToken,
    ) {
        loop {
            if let Err(err) = self
                .automation_worker_tick(&state_db, worker_id.as_str())
                .await
            {
                warn!("automation worker tick failed: {err:#}");
            }

            tokio::select! {
                _ = shutdown.cancelled() => break,
                _ = tokio::time::sleep(AUTOMATION_POLL_INTERVAL) => {}
            }
        }
    }

    async fn automation_worker_tick(
        &self,
        state_db: &StateDbHandle,
        worker_id: &str,
    ) -> anyhow::Result<usize> {
        let mut processed = 0_usize;
        while processed < AUTOMATION_DISPATCH_BATCH_SIZE {
            let Some(claim) = state_db.claim_due_automation_dispatch(worker_id).await? else {
                break;
            };
            self.handle_claimed_automation(state_db, &claim).await;
            processed += 1;
        }
        Ok(processed)
    }

    async fn handle_claimed_automation(
        &self,
        state_db: &StateDbHandle,
        claim: &AutomationDispatchClaim,
    ) {
        if let Err(err) = self.dispatch_claimed_automation(state_db, claim).await {
            let result = if err.code == INVALID_REQUEST_ERROR_CODE {
                state_db
                    .mark_automation_dispatch_failed_terminal(
                        claim.automation.id.as_str(),
                        claim.ownership_token.as_str(),
                        err.message.as_str(),
                    )
                    .await
                    .map(|_| ())
            } else {
                state_db
                    .release_automation_dispatch_after_retryable_failure(
                        claim.automation.id.as_str(),
                        claim.ownership_token.as_str(),
                        err.message.as_str(),
                    )
                    .await
                    .map(|_| ())
            };
            if let Err(state_err) = result {
                warn!(
                    "failed to persist automation {} dispatch failure: {state_err:#}",
                    claim.automation.id
                );
            }
            warn!(
                "automation {} dispatch failed: {}",
                claim.automation.id, err.message
            );
        }
    }
}
