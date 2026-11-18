//! Captures failed-review context and retains it for opt-in feedback before reviewer cleanup.

use super::GuardianReviewSession;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::GuardianReviewContext;
use crate::guardian::approval_request::format_guardian_action_pretty;
use crate::guardian::approval_request::guardian_request_target_item_id;
use crate::guardian::approval_request::guardian_request_turn_id;
use crate::guardian::review_session::GuardianReviewSessionOutcome;
use crate::session::session::Session;
use codex_guardian_reviewer::ReviewFeedbackSettings;

impl GuardianReviewSession {
    pub(in crate::guardian) async fn record_failed_review(
        &self,
        parent: &Session,
        context: &GuardianReviewContext,
        request: &GuardianApprovalRequest,
        model: &str,
        settings: ReviewFeedbackSettings,
        outcome: &GuardianReviewSessionOutcome,
    ) {
        let Some(feedback) =
            codex_guardian_reviewer::FailedReviewFeedback::for_outcome(outcome, settings)
        else {
            return;
        };
        let Ok(action) = format_guardian_action_pretty(request) else {
            tracing::warn!("Could not serialize Guardian feedback action");
            return;
        };
        let instructions = self.session.get_prompt_base_instructions().await;
        let history = self.session.clone_history().await;
        let record = feedback.into_record(codex_guardian_reviewer::ReviewFeedbackContext {
            reviewed_thread_id: parent.thread_id(),
            reviewed_turn_id: guardian_request_turn_id(request, &context.turn().sub_id),
            target_item_id: guardian_request_target_item_id(request),
            reviewer_thread_id: self.session.thread_id(),
            model,
            action: &action,
            action_truncated: false,
            instructions: Some(&instructions.text),
            history: history.raw_items().collect(),
        });
        if let Some(record) = record {
            codex_feedback::record_guardian_review_failure(parent.state_db().as_deref(), record)
                .await;
        }
    }
}
