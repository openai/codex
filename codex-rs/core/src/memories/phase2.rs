use crate::agent::AgentStatus;
use crate::agent::status::is_final as is_final_agent_status;
use crate::codex::Session;
use crate::memories::metrics;
use crate::memories::phase_two;
use codex_protocol::ThreadId;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tracing::debug;
use tracing::info;
use tracing::warn;

pub(in crate::memories) fn spawn_phase2_completion_task(
    session: &Session,
    ownership_token: String,
    completion_watermark: i64,
    consolidation_agent_id: ThreadId,
) {
    let state_db = session.services.state_db.clone();
    let agent_control = session.services.agent_control.clone();
    let otel_manager = session.services.otel_manager.clone();

    tokio::spawn(async move {
        let Some(state_db) = state_db else {
            return;
        };

        let status_rx = match agent_control.subscribe_status(consolidation_agent_id).await {
            Ok(status_rx) => status_rx,
            Err(err) => {
                warn!(
                    "failed to subscribe to global memory consolidation agent {consolidation_agent_id}: {err}"
                );
                otel_manager.counter(
                    metrics::MEMORY_PHASE_TWO_JOBS,
                    1,
                    &[("status", "failed_subscribe_status")],
                );
                mark_phase2_failed_with_recovery(
                    state_db.as_ref(),
                    &ownership_token,
                    "failed to subscribe to consolidation agent status",
                )
                .await;
                return;
            }
        };

        let final_status = run_phase2_completion_task(
            Arc::clone(&state_db),
            ownership_token,
            completion_watermark,
            consolidation_agent_id,
            status_rx,
        )
        .await;
        if matches!(final_status, AgentStatus::Shutdown | AgentStatus::NotFound) {
            otel_manager.counter(
                metrics::MEMORY_PHASE_TWO_JOBS,
                1,
                &[("status", "failed_agent_unavailable")],
            );
            return;
        }
        if is_phase2_success(&final_status) {
            otel_manager.counter(
                metrics::MEMORY_PHASE_TWO_JOBS,
                1,
                &[("status", "succeeded")],
            );
        } else {
            otel_manager.counter(metrics::MEMORY_PHASE_TWO_JOBS, 1, &[("status", "failed")]);
        }

        tokio::spawn(async move {
            if let Err(err) = agent_control.shutdown_agent(consolidation_agent_id).await {
                warn!(
                    "failed to auto-close global memory consolidation agent {consolidation_agent_id}: {err}"
                );
            }
        });
    });
}

async fn run_phase2_completion_task(
    state_db: Arc<codex_state::StateRuntime>,
    ownership_token: String,
    completion_watermark: i64,
    consolidation_agent_id: ThreadId,
    mut status_rx: watch::Receiver<AgentStatus>,
) -> AgentStatus {
    let final_status = {
        let mut heartbeat_interval =
            tokio::time::interval(Duration::from_secs(phase_two::JOB_HEARTBEAT_SECONDS));
        heartbeat_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            let status = status_rx.borrow().clone();
            if is_final_agent_status(&status) {
                break status;
            }

            tokio::select! {
                changed = status_rx.changed() => {
                    if changed.is_err() {
                        warn!(
                            "lost status updates for global memory consolidation agent {consolidation_agent_id}"
                        );
                        break status;
                    }
                }
                _ = heartbeat_interval.tick() => {
                    match state_db
                        .heartbeat_global_phase2_job(
                            &ownership_token,
                            phase_two::JOB_LEASE_SECONDS,
                        )
                        .await
                    {
                        Ok(true) => {}
                        Ok(false) => {
                            warn!(
                                "memory phase-2 heartbeat lost global ownership; finalizing as failure"
                            );
                            break AgentStatus::Errored(
                                "lost global phase-2 ownership during heartbeat".to_string(),
                            );
                        }
                        Err(err) => {
                            warn!(
                                "state db heartbeat_global_phase2_job failed during memories startup: {err}"
                            );
                            break AgentStatus::Errored(format!(
                                "phase-2 heartbeat update failed: {err}"
                            ));
                        }
                    }
                }
            }
        }
    };

    let phase2_success = is_phase2_success(&final_status);
    info!(
        "memory phase-2 global consolidation complete: agent_id={consolidation_agent_id} success={phase2_success} final_status={final_status:?}"
    );

    if phase2_success {
        match state_db
            .mark_global_phase2_job_succeeded(&ownership_token, completion_watermark)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                debug!(
                    "memory phase-2 success finalization skipped after global ownership changed"
                );
            }
            Err(err) => {
                warn!(
                    "state db mark_global_phase2_job_succeeded failed during memories startup: {err}"
                );
            }
        }
        return final_status;
    }

    let failure_reason = phase2_failure_reason(&final_status);
    mark_phase2_failed_with_recovery(state_db.as_ref(), &ownership_token, &failure_reason).await;
    warn!(
        "memory phase-2 global consolidation agent finished with non-success status: agent_id={consolidation_agent_id} final_status={final_status:?}"
    );
    final_status
}

async fn mark_phase2_failed_with_recovery(
    state_db: &codex_state::StateRuntime,
    ownership_token: &str,
    failure_reason: &str,
) {
    match state_db
        .mark_global_phase2_job_failed(
            ownership_token,
            failure_reason,
            phase_two::JOB_RETRY_DELAY_SECONDS,
        )
        .await
    {
        Ok(true) => {}
        Ok(false) => match state_db
            .mark_global_phase2_job_failed_if_unowned(
                ownership_token,
                failure_reason,
                phase_two::JOB_RETRY_DELAY_SECONDS,
            )
            .await
        {
            Ok(true) => {
                debug!(
                    "memory phase-2 failure finalization applied fallback update for unowned running job"
                );
            }
            Ok(false) => {
                debug!(
                    "memory phase-2 failure finalization skipped after global ownership changed"
                );
            }
            Err(err) => {
                warn!(
                    "state db mark_global_phase2_job_failed_if_unowned failed during memories startup: {err}"
                );
            }
        },
        Err(err) => {
            warn!("state db mark_global_phase2_job_failed failed during memories startup: {err}");
        }
    }
}

fn is_phase2_success(final_status: &AgentStatus) -> bool {
    matches!(final_status, AgentStatus::Completed(_))
}

fn phase2_failure_reason(final_status: &AgentStatus) -> String {
    format!("consolidation agent finished with status {final_status:?}")
}

#[cfg(test)]
mod tests {
    use super::is_phase2_success;
    use super::phase2_failure_reason;
    use super::run_phase2_completion_task;
    use crate::agent::AgentStatus;
    use codex_protocol::ThreadId;
    use codex_state::Phase2JobClaimOutcome;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;

    #[test]
    fn phase2_success_only_for_completed_status() {
        assert!(is_phase2_success(&AgentStatus::Completed(None)));
        assert!(!is_phase2_success(&AgentStatus::Running));
        assert!(!is_phase2_success(&AgentStatus::Errored(
            "oops".to_string()
        )));
    }

    #[test]
    fn phase2_failure_reason_includes_status() {
        let status = AgentStatus::Errored("boom".to_string());
        let reason = phase2_failure_reason(&status);
        assert!(reason.contains("consolidation agent finished with status"));
        assert!(reason.contains("boom"));
    }

    #[tokio::test]
    async fn phase2_completion_marks_succeeded_for_completed_status() {
        let codex_home = tempfile::tempdir().expect("create temp codex home");
        let state_db = Arc::new(
            codex_state::StateRuntime::init(
                codex_home.path().to_path_buf(),
                "test-provider".to_string(),
                None,
            )
            .await
            .expect("initialize state runtime"),
        );
        let owner = ThreadId::new();
        state_db
            .enqueue_global_consolidation(123)
            .await
            .expect("enqueue global consolidation");
        let claim = state_db
            .try_claim_global_phase2_job(owner, 3_600)
            .await
            .expect("claim global phase-2 job");
        let ownership_token = match claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token, ..
            } => ownership_token,
            other => panic!("unexpected phase-2 claim outcome: {other:?}"),
        };

        let (_status_tx, status_rx) = tokio::sync::watch::channel(AgentStatus::Completed(None));
        run_phase2_completion_task(
            Arc::clone(&state_db),
            ownership_token.clone(),
            123,
            ThreadId::new(),
            status_rx,
        )
        .await;

        let up_to_date_claim = state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim up-to-date global job");
        assert_eq!(up_to_date_claim, Phase2JobClaimOutcome::SkippedNotDirty);

        state_db
            .enqueue_global_consolidation(124)
            .await
            .expect("enqueue advanced consolidation watermark");
        let rerun_claim = state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim rerun global job");
        assert!(
            matches!(rerun_claim, Phase2JobClaimOutcome::Claimed { .. }),
            "advanced watermark should be claimable after success finalization"
        );
    }

    #[tokio::test]
    async fn phase2_completion_marks_failed_when_status_updates_are_lost() {
        let codex_home = tempfile::tempdir().expect("create temp codex home");
        let state_db = Arc::new(
            codex_state::StateRuntime::init(
                codex_home.path().to_path_buf(),
                "test-provider".to_string(),
                None,
            )
            .await
            .expect("initialize state runtime"),
        );
        state_db
            .enqueue_global_consolidation(456)
            .await
            .expect("enqueue global consolidation");
        let claim = state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim global phase-2 job");
        let ownership_token = match claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token, ..
            } => ownership_token,
            other => panic!("unexpected phase-2 claim outcome: {other:?}"),
        };

        let (status_tx, status_rx) = tokio::sync::watch::channel(AgentStatus::Running);
        drop(status_tx);
        run_phase2_completion_task(
            Arc::clone(&state_db),
            ownership_token,
            456,
            ThreadId::new(),
            status_rx,
        )
        .await;

        let claim = state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim after failure finalization");
        assert_eq!(
            claim,
            Phase2JobClaimOutcome::SkippedNotDirty,
            "failure finalization should leave global job in retry-backoff, not running ownership"
        );
    }

    #[tokio::test]
    async fn phase2_completion_heartbeat_loss_does_not_steal_active_other_owner() {
        let codex_home = tempfile::tempdir().expect("create temp codex home");
        let state_db = Arc::new(
            codex_state::StateRuntime::init(
                codex_home.path().to_path_buf(),
                "test-provider".to_string(),
                None,
            )
            .await
            .expect("initialize state runtime"),
        );
        state_db
            .enqueue_global_consolidation(789)
            .await
            .expect("enqueue global consolidation");
        let claim = state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim global phase-2 job");
        let claimed_token = match claim {
            Phase2JobClaimOutcome::Claimed {
                ownership_token, ..
            } => ownership_token,
            other => panic!("unexpected phase-2 claim outcome: {other:?}"),
        };

        let (_status_tx, status_rx) = tokio::sync::watch::channel(AgentStatus::Running);
        run_phase2_completion_task(
            Arc::clone(&state_db),
            "non-owner-token".to_string(),
            789,
            ThreadId::new(),
            status_rx,
        )
        .await;

        let claim = state_db
            .try_claim_global_phase2_job(ThreadId::new(), 3_600)
            .await
            .expect("claim after heartbeat ownership loss");
        assert_eq!(
            claim,
            Phase2JobClaimOutcome::SkippedRunning,
            "heartbeat ownership-loss handling should not steal a live owner lease"
        );
        assert_eq!(
            state_db
                .mark_global_phase2_job_succeeded(claimed_token.as_str(), 789)
                .await
                .expect("mark original owner success"),
            true,
            "the original owner should still be able to finalize"
        );
    }
}
