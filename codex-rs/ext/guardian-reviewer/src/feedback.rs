//! Selects failed reviews for opt-in feedback and bounds each serialized record.
//! The host supplies captured context; records outlive short-lived reviewer sessions.

use crate::GuardianReviewSessionOutcome;
use crate::parse_guardian_assessment;
use codex_feedback::record_guardian_review_failure;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::GuardianAssessmentOutcome;
use serde::Serialize;
use std::io;
use std::io::Write;

const MAX_RECORD_BYTES: usize = 4 * 1024 * 1024;

#[derive(Serialize)]
struct ReviewFeedbackRecord<'a> {
    reviewed_thread_id: ThreadId,
    reviewed_turn_id: &'a str,
    target_item_id: Option<&'a str>,
    reviewer_thread_id: ThreadId,
    model: &'a str,
    status: &'a str,
    decision: Option<&'a str>,
    action: &'a str,
    action_truncated: bool,
    instructions: Option<&'a str>,
    history: Vec<&'a ResponseItem>,
    context_omitted: bool,
}

/// Context captured by the host only after feedback policy selects a failed review.
pub struct ReviewFeedbackContext<'a> {
    pub reviewed_thread_id: ThreadId,
    pub reviewed_turn_id: &'a str,
    pub target_item_id: Option<&'a str>,
    pub reviewer_thread_id: ThreadId,
    pub model: &'a str,
    pub action: &'a str,
    pub action_truncated: bool,
    pub instructions: Option<&'a str>,
    pub history: Vec<&'a ResponseItem>,
}

/// Host-supplied opt-in and persistence facts for the reviewed session.
pub struct ReviewFeedbackSettings {
    pub enabled: bool,
    pub ephemeral: bool,
}

pub struct FailedReviewFeedback<'a> {
    status: &'static str,
    decision: Option<&'a str>,
}

impl<'a> FailedReviewFeedback<'a> {
    pub fn for_outcome(
        outcome: &'a GuardianReviewSessionOutcome,
        settings: ReviewFeedbackSettings,
    ) -> Option<Self> {
        if !settings.enabled || settings.ephemeral {
            return None;
        }
        let (status, decision) = match outcome {
            GuardianReviewSessionOutcome::Completed(Ok(Some(decision))) => {
                match parse_guardian_assessment(Some(decision)) {
                    Ok(assessment) if assessment.outcome == GuardianAssessmentOutcome::Allow => {
                        return None;
                    }
                    Ok(_) => ("denied", Some(decision.as_str())),
                    Err(_) => ("invalid_decision", Some(decision.as_str())),
                }
            }
            GuardianReviewSessionOutcome::Completed(Ok(None)) => ("missing_decision", None),
            GuardianReviewSessionOutcome::Completed(Err(_))
            | GuardianReviewSessionOutcome::PromptBuildFailed(_)
            | GuardianReviewSessionOutcome::InputBudgetExceeded
            | GuardianReviewSessionOutcome::SessionFailed { .. } => ("failed", None),
            GuardianReviewSessionOutcome::TimedOut => ("timed_out", None),
            GuardianReviewSessionOutcome::Aborted => ("aborted", None),
        };
        Some(Self { status, decision })
    }

    pub fn store(self, context: ReviewFeedbackContext<'_>) {
        ReviewFeedbackRecord {
            reviewed_thread_id: context.reviewed_thread_id,
            reviewed_turn_id: context.reviewed_turn_id,
            target_item_id: context.target_item_id,
            reviewer_thread_id: context.reviewer_thread_id,
            model: context.model,
            action: context.action,
            action_truncated: context.action_truncated,
            instructions: context.instructions,
            history: context.history,
            status: self.status,
            decision: self.decision,
            context_omitted: false,
        }
        .store();
    }
}

impl ReviewFeedbackRecord<'_> {
    fn store(mut self) {
        let mut buffer = BoundedBuffer(Vec::new());
        if serde_json::to_writer(&mut buffer, &self).is_err() {
            // Preserve the action and decision even when optional reviewer history is too large.
            self.instructions = None;
            self.history.clear();
            self.context_omitted = true;
            buffer = BoundedBuffer(Vec::new());
            if serde_json::to_writer(&mut buffer, &self).is_err() {
                tracing::warn!("Guardian feedback action and decision exceed the record limit");
                return;
            }
        }
        record_guardian_review_failure(self.reviewed_thread_id, buffer.0);
    }
}

struct BoundedBuffer(Vec<u8>);

impl Write for BoundedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_RECORD_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other(
                "Guardian feedback record exceeds its size limit",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "feedback_tests.rs"]
mod tests;
