use crate::agent::AgentStatus;
use crate::agent::status::is_final as is_final_agent_status;
use crate::codex::Session;
use codex_protocol::ThreadId;
use std::time::Duration;
use tracing::debug;
use tracing::info;
use tracing::warn;

use super::super::PHASE_TWO_JOB_HEARTBEAT_SECONDS;
use super::super::PHASE_TWO_JOB_LEASE_SECONDS;
use super::super::PHASE_TWO_JOB_RETRY_DELAY_SECONDS;
use super::MemoryScopeTarget;

pub(super) fn spawn_phase2_completion_task(
    session: &Session,
    scope: MemoryScopeTarget,
    ownership_token: String,
    completion_watermark: i64,
    consolidation_agent_id: ThreadId,
) {
    let state_db = session.services.state_db.clone();
    let agent_control = session.services.agent_control.clone();

    tokio::spawn(async move {
        let Some(state_db) = state_db.as_deref() else {
            return;
        };

        let mut status_rx = match agent_control.subscribe_status(consolidation_agent_id).await {
            Ok(status_rx) => status_rx,
            Err(err) => {
                warn!(
                    "failed to subscribe to memory consolidation agent {} for scope {}:{}: {err}",
                    consolidation_agent_id, scope.scope_kind, scope.scope_key
                );
                let _ = state_db
                    .mark_phase2_job_failed(
                        scope.scope_kind,
                        &scope.scope_key,
                        &ownership_token,
                        "failed to subscribe to consolidation agent status",
                        PHASE_TWO_JOB_RETRY_DELAY_SECONDS,
                    )
                    .await;
                return;
            }
        };

        let mut heartbeat_interval =
            tokio::time::interval(Duration::from_secs(PHASE_TWO_JOB_HEARTBEAT_SECONDS));
        heartbeat_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let final_status = loop {
            let status = status_rx.borrow().clone();
            if is_final_agent_status(&status) {
                break status;
            }

            tokio::select! {
                changed = status_rx.changed() => {
                    if changed.is_err() {
                        warn!(
                            "lost status updates for memory consolidation agent {} in scope {}:{}",
                            consolidation_agent_id, scope.scope_kind, scope.scope_key
                        );
                        break status;
                    }
                }
                _ = heartbeat_interval.tick() => {
                    match state_db
                        .heartbeat_phase2_job(
                            scope.scope_kind,
                            &scope.scope_key,
                            &ownership_token,
                            PHASE_TWO_JOB_LEASE_SECONDS,
                        )
                        .await
                    {
                        Ok(true) => {}
                        Ok(false) => {
                            debug!(
                                "memory phase-2 heartbeat lost ownership for scope {}:{}; skipping finalization",
                                scope.scope_kind, scope.scope_key
                            );
                            return;
                        }
                        Err(err) => {
                            warn!(
                                "state db heartbeat_phase2_job failed during memories startup: {err}"
                            );
                            return;
                        }
                    }
                }
            }
        };

        if is_phase2_success(&final_status) {
            match state_db
                .mark_phase2_job_succeeded(
                    scope.scope_kind,
                    &scope.scope_key,
                    &ownership_token,
                    completion_watermark,
                )
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    debug!(
                        "memory phase-2 success finalization skipped after ownership changed: scope={} scope_key={}",
                        scope.scope_kind, scope.scope_key
                    );
                }
                Err(err) => {
                    warn!(
                        "state db mark_phase2_job_succeeded failed during memories startup: {err}"
                    );
                }
            }
            info!(
                "memory phase-2 consolidation agent finished: scope={} scope_key={} agent_id={} final_status={final_status:?}",
                scope.scope_kind, scope.scope_key, consolidation_agent_id
            );
            return;
        }

        let failure_reason = phase2_failure_reason(&final_status);
        match state_db
            .mark_phase2_job_failed(
                scope.scope_kind,
                &scope.scope_key,
                &ownership_token,
                &failure_reason,
                PHASE_TWO_JOB_RETRY_DELAY_SECONDS,
            )
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                debug!(
                    "memory phase-2 failure finalization skipped after ownership changed: scope={} scope_key={}",
                    scope.scope_kind, scope.scope_key
                );
            }
            Err(err) => {
                warn!("state db mark_phase2_job_failed failed during memories startup: {err}");
            }
        }
        warn!(
            "memory phase-2 consolidation agent finished with non-success status: scope={} scope_key={} agent_id={} final_status={final_status:?}",
            scope.scope_kind, scope.scope_key, consolidation_agent_id
        );
    });
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
    use crate::agent::AgentStatus;

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
}
