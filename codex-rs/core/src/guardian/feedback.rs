//! Captures failed-review context for the extension's bounded feedback recorder.

use super::approval_request::format_guardian_action_pretty;
use super::approval_request::guardian_request_target_item_id;
use super::approval_request::guardian_request_turn_id;
use super::review_session::GuardianReviewSessionOutcome;
use super::review_session::GuardianReviewSessionParams;
use crate::session::session::Session;

pub(super) async fn record_failed_review(
    reviewer: &Session,
    params: &GuardianReviewSessionParams,
    outcome: &GuardianReviewSessionOutcome,
) {
    let Some(feedback) = codex_guardian_reviewer::FailedReviewFeedback::for_outcome(
        outcome,
        codex_guardian_reviewer::ReviewFeedbackSettings {
            enabled: params.spawn_config.feedback_enabled,
            ephemeral: params.spawn_config.ephemeral,
        },
    ) else {
        return;
    };
    let Ok(action) = format_guardian_action_pretty(&params.request) else {
        tracing::warn!("Could not serialize Guardian feedback action");
        return;
    };
    let instructions = reviewer.get_prompt_base_instructions().await;
    let history = reviewer.clone_history().await;
    feedback.store(codex_guardian_reviewer::ReviewFeedbackContext {
        reviewed_thread_id: params.parent_session.thread_id(),
        reviewed_turn_id: guardian_request_turn_id(
            &params.request,
            &params.parent_context.turn().sub_id,
        ),
        target_item_id: guardian_request_target_item_id(&params.request),
        reviewer_thread_id: reviewer.thread_id(),
        model: &params.model,
        action: &action,
        action_truncated: false,
        instructions: Some(&instructions.text),
        history: history.raw_items().collect(),
    });
}
