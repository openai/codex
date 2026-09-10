//! Executes reviewer turns, including deadlines, cancellation and terminal event matching.
//! A session is reusable only after its submitted turn has completed or drained.

use std::future::Future;
use std::time::Duration;

use anyhow::anyhow;
use codex_analytics::GuardianReviewAnalyticsResult;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnInputSubmission;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::GuardianReviewSessionOutcome;

const GUARDIAN_INTERRUPT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// Access to a private runtime. Event reads must be cancellation-safe; context
/// admission runs separately after a read so cancellation cannot consume an event
/// before the context builder has recorded it. Hosts do not interpret review outcomes.
pub trait ReviewerRuntime: Sync {
    fn submit_turn(
        &self,
        request: TurnInputRequest,
    ) -> impl Future<Output = anyhow::Result<TurnInputSubmission>> + Send;
    fn next_event(&self) -> impl Future<Output = anyhow::Result<Event>> + Send;
    fn admit_context(&self, event: &Event) -> impl Future<Output = ()> + Send;
    fn interrupt(&self) -> impl Future<Output = anyhow::Result<()>> + Send;
    fn retry_at(&self, turn_id: &str) -> Option<Instant>;
}

/// Submit exactly one review turn within the review's remaining deadline.
pub async fn start_review_turn(
    runtime: &impl ReviewerRuntime,
    request: TurnInputRequest,
    deadline: Instant,
    external_cancel: Option<&CancellationToken>,
) -> Result<String, GuardianReviewSessionOutcome> {
    match crate::run_before_review_deadline(deadline, external_cancel, runtime.submit_turn(request))
        .await?
    {
        Ok(TurnInputSubmission::Started { turn_id }) => Ok(turn_id),
        result => Err(GuardianReviewSessionOutcome::SessionFailed {
            error: match result {
                Ok(submission) => anyhow!("guardian review input was not started: {submission:?}"),
                Err(error) => error,
            },
            error_info: None,
            retry_at: None,
        }),
    }
}

pub async fn wait_for_guardian_review(
    runtime: &impl ReviewerRuntime,
    expected_turn_id: &str,
    deadline: tokio::time::Instant,
    external_cancel: Option<&CancellationToken>,
    analytics_result: &mut GuardianReviewAnalyticsResult,
) -> (GuardianReviewSessionOutcome, bool, bool) {
    let timeout = tokio::time::sleep_until(deadline);
    tokio::pin!(timeout);
    let mut last_error: Option<ErrorEvent> = None;

    loop {
        tokio::select! {
            _ = &mut timeout => {
                let keep_review_session = interrupt_and_drain_turn(
                    runtime,
                    expected_turn_id,
                )
                .await
                .is_ok();
                return (GuardianReviewSessionOutcome::TimedOut, keep_review_session, false);
            }
            _ = async {
                if let Some(cancel_token) = external_cancel {
                    cancel_token.cancelled().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                let keep_review_session = interrupt_and_drain_turn(
                    runtime,
                    expected_turn_id,
                )
                .await
                .is_ok();
                return (GuardianReviewSessionOutcome::Aborted, keep_review_session, false);
            }
            event = runtime.next_event() => {
                match event {
                    Ok(event) if !event_matches_turn(&event, expected_turn_id) => {}
                    Ok(event) if matches!(&event.msg, EventMsg::ItemCompleted(_)) => {
                        runtime.admit_context(&event).await;
                    }
                    Ok(event) => match event.msg {
                        EventMsg::TurnComplete(turn_complete) => {
                            analytics_result.time_to_first_token_ms = turn_complete
                                .time_to_first_token_ms
                                .and_then(|ms| u64::try_from(ms).ok());
                            if turn_complete.last_agent_message.is_none()
                                && let Some(error) = last_error
                            {
                                return (
                                    GuardianReviewSessionOutcome::SessionFailed {
                                        error: anyhow!(error.message),
                                        error_info: error.codex_error_info,
                                        retry_at: runtime.retry_at(expected_turn_id),
                                    },
                                    true,
                                    true,
                                );
                            }
                            return (
                                GuardianReviewSessionOutcome::Completed(Ok(turn_complete.last_agent_message)),
                                true,
                                true,
                            );
                        }
                        EventMsg::Error(error) => {
                            last_error = Some(error);
                        }
                        EventMsg::TurnAborted(_) => {
                            return (GuardianReviewSessionOutcome::Aborted, true, false);
                        }
                        _ => {}
                    },
                    Err(err) => {
                        return (
                            GuardianReviewSessionOutcome::Completed(Err(err)),
                            false,
                            false,
                        );
                    }
                }
            }
        }
    }
}

fn event_matches_turn(event: &Event, expected_turn_id: &str) -> bool {
    if event.id != expected_turn_id {
        return false;
    }

    match &event.msg {
        EventMsg::TurnComplete(turn_complete) => turn_complete.turn_id == expected_turn_id,
        EventMsg::TurnAborted(turn_aborted) => {
            turn_aborted.turn_id.as_deref() == Some(expected_turn_id)
        }
        _ => true,
    }
}

async fn interrupt_and_drain_turn(
    runtime: &impl ReviewerRuntime,
    expected_turn_id: &str,
) -> anyhow::Result<()> {
    let _ = runtime.interrupt().await;

    tokio::time::timeout(GUARDIAN_INTERRUPT_DRAIN_TIMEOUT, async {
        loop {
            let event = runtime.next_event().await?;
            if !event_matches_turn(&event, expected_turn_id) {
                continue;
            }
            runtime.admit_context(&event).await;
            if matches!(
                event.msg,
                EventMsg::TurnAborted(_) | EventMsg::TurnComplete(_)
            ) {
                return Ok::<(), anyhow::Error>(());
            }
        }
    })
    .await
    .map_err(|_| anyhow!("timed out draining guardian review session after interrupt"))??;

    Ok(())
}
