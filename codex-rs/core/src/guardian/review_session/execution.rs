//! Execution helpers for a single guardian review turn.
//!
//! These helpers run on an already-spawned guardian session. They own submission, event draining,
//! outer deadline/cancel handling, and the "is this session still healthy enough to reuse?"
//! decision that feeds back into the manager layer.

use std::future::Future;
use std::sync::atomic::Ordering;

use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::DeveloperInstructions;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;

use crate::codex::Codex;
use crate::protocol::SandboxPolicy;

use super::GUARDIAN_FOLLOWUP_REVIEW_REMINDER;
use super::GUARDIAN_INTERRUPT_DRAIN_TIMEOUT;
use super::GuardianReviewExecutionResult;
use super::GuardianReviewSession;
use super::GuardianReviewSessionOutcome;
use super::GuardianReviewSessionParams;

pub(super) async fn run_review_on_session(
    review_session: &GuardianReviewSession,
    params: &GuardianReviewSessionParams,
    deadline: tokio::time::Instant,
) -> GuardianReviewExecutionResult {
    // Follow-up reviews append a reminder directly into guardian history so the next review can
    // use earlier guardian output as context without treating it as binding precedent.
    if review_session.has_prior_review.load(Ordering::Relaxed) {
        append_guardian_followup_reminder(review_session).await;
    }

    let submit_result = run_before_review_deadline(
        deadline,
        params.external_cancel.as_ref(),
        Box::pin(async {
            params
                .parent_session
                .services
                .network_approval
                .sync_session_approved_hosts_to(
                    &review_session.codex.session.services.network_approval,
                )
                .await;

            review_session
                .codex
                .submit(Op::UserTurn {
                    items: params.prompt_items.clone(),
                    cwd: params.parent_turn.cwd.clone(),
                    approval_policy: AskForApproval::Never,
                    approvals_reviewer: None,
                    sandbox_policy: SandboxPolicy::new_read_only_policy(),
                    model: params.model.clone(),
                    effort: params.reasoning_effort,
                    summary: Some(ReasoningSummaryConfig::None),
                    service_tier: None,
                    final_output_json_schema: Some(params.schema.clone()),
                    collaboration_mode: None,
                    personality: None,
                })
                .await
        }),
    )
    .await;
    let submit_result = match submit_result {
        Ok(submit_result) => submit_result,
        Err(outcome) => {
            return GuardianReviewExecutionResult {
                outcome,
                session_healthy: false,
            };
        }
    };
    if let Err(err) = submit_result {
        return GuardianReviewExecutionResult {
            outcome: GuardianReviewSessionOutcome::Completed(Err(err.into())),
            session_healthy: false,
        };
    }

    let execution_result =
        wait_for_guardian_review(review_session, deadline, params.external_cancel.as_ref()).await;
    if matches!(
        execution_result.outcome,
        GuardianReviewSessionOutcome::Completed(_)
    ) {
        review_session
            .has_prior_review
            .store(true, Ordering::Relaxed);
    }
    execution_result
}

async fn append_guardian_followup_reminder(review_session: &GuardianReviewSession) {
    let turn_context = review_session.codex.session.new_default_turn().await;
    let reminder: ResponseItem =
        DeveloperInstructions::new(GUARDIAN_FOLLOWUP_REVIEW_REMINDER).into();
    review_session
        .codex
        .session
        .record_into_history(std::slice::from_ref(&reminder), turn_context.as_ref())
        .await;
}

async fn wait_for_guardian_review(
    review_session: &GuardianReviewSession,
    deadline: tokio::time::Instant,
    external_cancel: Option<&tokio_util::sync::CancellationToken>,
) -> GuardianReviewExecutionResult {
    let timeout = tokio::time::sleep_until(deadline);
    tokio::pin!(timeout);
    let mut last_error_message: Option<String> = None;

    loop {
        tokio::select! {
            _ = &mut timeout => {
                let session_healthy = interrupt_and_drain_turn(&review_session.codex).await.is_ok();
                return GuardianReviewExecutionResult {
                    outcome: GuardianReviewSessionOutcome::TimedOut,
                    session_healthy,
                };
            }
            _ = async {
                if let Some(cancel_token) = external_cancel {
                    cancel_token.cancelled().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                let session_healthy = interrupt_and_drain_turn(&review_session.codex).await.is_ok();
                return GuardianReviewExecutionResult {
                    outcome: GuardianReviewSessionOutcome::Aborted,
                    session_healthy,
                };
            }
            event = review_session.codex.next_event() => {
                match event {
                    Ok(event) => match event.msg {
                        EventMsg::TurnComplete(turn_complete) => {
                            // The guardian child may emit an `Error` event before `TurnComplete`.
                            // Preserve that message if the turn completes without a final answer.
                            if turn_complete.last_agent_message.is_none()
                                && let Some(error_message) = last_error_message
                            {
                                return GuardianReviewExecutionResult {
                                    outcome: GuardianReviewSessionOutcome::Completed(
                                        Err(anyhow::anyhow!(error_message)),
                                    ),
                                    session_healthy: true,
                                };
                            }
                            return GuardianReviewExecutionResult {
                                outcome: GuardianReviewSessionOutcome::Completed(
                                    Ok(turn_complete.last_agent_message),
                                ),
                                session_healthy: true,
                            };
                        }
                        EventMsg::Error(error) => {
                            last_error_message = Some(error.message);
                        }
                        EventMsg::TurnAborted(_) => {
                            return GuardianReviewExecutionResult {
                                outcome: GuardianReviewSessionOutcome::Aborted,
                                session_healthy: true,
                            };
                        }
                        _ => {}
                    },
                    Err(err) => {
                        return GuardianReviewExecutionResult {
                            outcome: GuardianReviewSessionOutcome::Completed(Err(err.into())),
                            session_healthy: false,
                        };
                    }
                }
            }
        }
    }
}

/// Runs a future under the guardian review deadline and external cancellation policy.
pub(super) async fn run_before_review_deadline<T>(
    deadline: tokio::time::Instant,
    external_cancel: Option<&tokio_util::sync::CancellationToken>,
    future: impl Future<Output = T>,
) -> Result<T, GuardianReviewSessionOutcome> {
    tokio::select! {
        _ = tokio::time::sleep_until(deadline) => Err(GuardianReviewSessionOutcome::TimedOut),
        result = future => Ok(result),
        _ = async {
            if let Some(cancel_token) = external_cancel {
                cancel_token.cancelled().await;
            } else {
                std::future::pending::<()>().await;
            }
        } => Err(GuardianReviewSessionOutcome::Aborted),
    }
}

/// Same as `run_before_review_deadline`, but also cancels a child-session-local token when the
/// outer review policy aborts or times out.
pub(super) async fn run_before_review_deadline_with_cancel<T>(
    deadline: tokio::time::Instant,
    external_cancel: Option<&tokio_util::sync::CancellationToken>,
    cancel_token: &tokio_util::sync::CancellationToken,
    future: impl Future<Output = T>,
) -> Result<T, GuardianReviewSessionOutcome> {
    let result = run_before_review_deadline(deadline, external_cancel, future).await;
    if result.is_err() {
        cancel_token.cancel();
    }
    result
}

/// Attempts to interrupt the current guardian turn and drain it to a terminal event.
///
/// This is used when the outer review deadline/cancel path fires after the child turn was already
/// submitted. If draining succeeds, the caller may keep reusing the session.
async fn interrupt_and_drain_turn(codex: &Codex) -> anyhow::Result<()> {
    let _ = codex.submit(Op::Interrupt).await;

    tokio::time::timeout(GUARDIAN_INTERRUPT_DRAIN_TIMEOUT, async {
        loop {
            let event = codex.next_event().await?;
            if matches!(
                event.msg,
                EventMsg::TurnAborted(_) | EventMsg::TurnComplete(_)
            ) {
                return Ok::<(), anyhow::Error>(());
            }
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out draining guardian review session after interrupt"))??;

    Ok(())
}
