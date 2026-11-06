//! Typed Guardian operations through the existing isolated-turn executor.

use codex_analytics::GuardianReviewAnalyticsResult;
use codex_guardian_reviewer::GuardianReviewSessionOutcome;
use codex_guardian_reviewer::ReviewDeadline;
use codex_guardian_reviewer::ReviewerRuntime;
use codex_guardian_reviewer::SessionDisposition;
use codex_guardian_reviewer::start_review_turn;
use codex_guardian_reviewer::wait_for_guardian_review;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TruncationPolicy;
use codex_protocol::turn_input::TurnInput;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnStartOptions;
use codex_utils_string::to_json_string_bounded;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

/// The host must honor the runtime disposition, including when an operation fails.
#[must_use = "honor the disposition before reusing the isolated runtime"]
pub struct OperationResult<T> {
    pub result: anyhow::Result<T>,
    pub disposition: SessionDisposition,
}

/// Existing housekeeper callers share the same disposition contract.
pub type HousekeeperResult<T> = OperationResult<T>;

pub(crate) enum GuardianOperation {
    HousekeeperRouting,
    ChannelSelection,
}

impl GuardianOperation {
    fn input_name(&self) -> &'static str {
        match self {
            Self::HousekeeperRouting => "guardian_housekeeper_input",
            Self::ChannelSelection => "guardian_channel_selection_input",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::HousekeeperRouting => "Guardian housekeeper routing",
            Self::ChannelSelection => "Guardian channel selection",
        }
    }
}

pub(crate) fn context_input(
    name: &str,
    input: &impl Serialize,
    policy: TruncationPolicy,
) -> anyhow::Result<ResponseItem> {
    Ok(ResponseItem::FunctionCallOutput {
        id: None,
        call_id: None,
        name: Some(name.into()),
        namespace: Some("guardian".into()),
        output: FunctionCallOutputPayload::from_text(to_json_string_bounded(
            input,
            policy.byte_budget(),
        )?),
        internal_chat_message_metadata_passthrough: None,
    })
}

pub(crate) async fn sample<T: DeserializeOwned>(
    runtime: &impl ReviewerRuntime,
    operation: GuardianOperation,
    input: &impl Serialize,
    policy: TruncationPolicy,
    schema: Value,
    deadline: ReviewDeadline,
    cancellation: &CancellationToken,
) -> OperationResult<T> {
    let label = operation.label();
    let input = match context_input(operation.input_name(), input, policy) {
        Ok(input) => input,
        Err(error) => {
            return OperationResult {
                result: Err(error),
                disposition: SessionDisposition::Reusable,
            };
        }
    };
    let request =
        TurnInputRequest::new(TurnInput::ResponseItem(input)).on_start(TurnStartOptions {
            final_output_json_schema: Some(schema),
            ..Default::default()
        });
    let turn_id = match start_review_turn(runtime, request, deadline, Some(cancellation)).await {
        Ok(turn_id) => turn_id,
        Err(_) => {
            return OperationResult {
                result: Err(anyhow::anyhow!("{label} turn did not start")),
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
            Err(anyhow::anyhow!("{label} returned an invalid response"))
        }
        GuardianReviewSessionOutcome::PromptBuildFailed(error)
        | GuardianReviewSessionOutcome::SessionFailed { error, .. } => Err(error),
        GuardianReviewSessionOutcome::InputBudgetExceeded => {
            Err(anyhow::anyhow!("{label} input exceeded budget"))
        }
        GuardianReviewSessionOutcome::TimedOut => Err(anyhow::anyhow!("{label} timed out")),
        GuardianReviewSessionOutcome::Aborted => Err(anyhow::anyhow!("{label} aborted")),
        GuardianReviewSessionOutcome::StaleAuthorization => {
            Err(anyhow::anyhow!("{label} input became stale"))
        }
    };
    if result.is_err() && matches!(disposition, SessionDisposition::Reusable) {
        disposition = SessionDisposition::Rollback;
    }
    OperationResult {
        result,
        disposition,
    }
}
