//! One typed housekeeper operation through the existing isolated-turn executor.

use codex_analytics::GuardianReviewAnalyticsResult;
use codex_guardian_reviewer::GuardianReviewSessionOutcome;
use codex_guardian_reviewer::ReviewDeadline;
use codex_guardian_reviewer::ReviewerRuntime;
use codex_guardian_reviewer::SessionDisposition;
use codex_guardian_reviewer::start_review_turn;
use codex_guardian_reviewer::wait_for_guardian_review;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::turn_input::TurnInput;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnStartOptions;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

/// The host must honor the runtime disposition, including when an operation fails.
#[must_use = "honor the disposition before reusing the isolated runtime"]
pub struct HousekeeperResult<T> {
    pub result: anyhow::Result<T>,
    pub disposition: SessionDisposition,
}

pub(crate) async fn sample<T: DeserializeOwned>(
    runtime: &impl ReviewerRuntime,
    input: &impl Serialize,
    schema: Value,
    deadline: ReviewDeadline,
    cancellation: &CancellationToken,
) -> HousekeeperResult<T> {
    let text = match serde_json::to_string(input) {
        Ok(text) => text,
        Err(error) => {
            return HousekeeperResult {
                result: Err(error.into()),
                disposition: SessionDisposition::Reusable,
            };
        }
    };
    let request =
        TurnInputRequest::new(TurnInput::ResponseItem(ResponseItem::FunctionCallOutput {
            id: None,
            call_id: None,
            name: Some("guardian_housekeeper_input".into()),
            namespace: Some("guardian".into()),
            output: FunctionCallOutputPayload::from_text(text),
            internal_chat_message_metadata_passthrough: None,
        }))
        .on_start(TurnStartOptions {
            final_output_json_schema: Some(schema),
            ..Default::default()
        });
    let turn_id = match start_review_turn(runtime, request, deadline, Some(cancellation)).await {
        Ok(turn_id) => turn_id,
        Err(_) => {
            return HousekeeperResult {
                result: Err(anyhow::anyhow!("housekeeper turn did not start")),
                disposition: SessionDisposition::Discard,
            };
        }
    };
    let turn = wait_for_guardian_review(
        runtime,
        &turn_id,
        deadline,
        Some(cancellation),
        &mut GuardianReviewAnalyticsResult::without_session(),
    )
    .await;
    let mut disposition = turn.disposition;
    let result = match turn.outcome {
        GuardianReviewSessionOutcome::Completed(Ok(Some(response))) => {
            serde_json::from_str(response.raw()).map_err(anyhow::Error::from)
        }
        GuardianReviewSessionOutcome::Completed(_) => {
            Err(anyhow::anyhow!("invalid housekeeper response"))
        }
        GuardianReviewSessionOutcome::PromptBuildFailed(error)
        | GuardianReviewSessionOutcome::SessionFailed { error, .. } => Err(error),
        GuardianReviewSessionOutcome::InputBudgetExceeded => {
            Err(anyhow::anyhow!("housekeeper input exceeded budget"))
        }
        GuardianReviewSessionOutcome::TimedOut => Err(anyhow::anyhow!("housekeeper timed out")),
        GuardianReviewSessionOutcome::Aborted => Err(anyhow::anyhow!("housekeeper aborted")),
        GuardianReviewSessionOutcome::StaleAuthorization => {
            Err(anyhow::anyhow!("housekeeper input became stale"))
        }
    };
    if result.is_err() && matches!(disposition, SessionDisposition::Reusable) {
        disposition = SessionDisposition::Rollback;
    }
    HousekeeperResult {
        result,
        disposition,
    }
}
