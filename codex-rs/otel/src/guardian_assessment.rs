//! Exports opted-in terminal Guardian assessments with bounded rationale text.
//! Hosts supply the effective model outcome; review failures have no model outcome.

use crate::SessionTelemetry;
use crate::events::shared::log_event;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::GuardianAssessmentOutcome;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::GuardianRiskLevel;
use codex_protocol::protocol::GuardianUserAuthorization;
use codex_utils_string::take_bytes_at_char_boundary;

const MAX_RATIONALE_BYTES: usize = 65_536;

impl SessionTelemetry {
    /// Record one terminal review after the host checks the opt-in and OTLP destination.
    /// `outcome` is absent for errors, timeouts and cancellations, including fail-closed denials.
    pub fn guardian_assessment(
        &self,
        assessment: &GuardianAssessmentEvent,
        outcome: Option<GuardianAssessmentOutcome>,
    ) {
        let status = match assessment.status {
            GuardianAssessmentStatus::InProgress => return,
            GuardianAssessmentStatus::Approved => "approved",
            GuardianAssessmentStatus::Denied => "denied",
            GuardianAssessmentStatus::TimedOut => "timed_out",
            GuardianAssessmentStatus::Aborted => "aborted",
        };
        let outcome = outcome.map(|outcome| match outcome {
            GuardianAssessmentOutcome::Allow => "allow",
            GuardianAssessmentOutcome::Deny => "deny",
        });
        let risk_level = assessment.risk_level.map(|risk| match risk {
            GuardianRiskLevel::Low => "low",
            GuardianRiskLevel::Medium => "medium",
            GuardianRiskLevel::High => "high",
            GuardianRiskLevel::Critical => "critical",
        });
        let user_authorization =
            assessment
                .user_authorization
                .map(|authorization| match authorization {
                    GuardianUserAuthorization::Unknown => "unknown",
                    GuardianUserAuthorization::Low => "low",
                    GuardianUserAuthorization::Medium => "medium",
                    GuardianUserAuthorization::High => "high",
                });
        let rationale = assessment.rationale.as_deref();
        log_event!(
            self,
            event.name = "codex.guardian_assessment",
            review.id = assessment.id.as_str(),
            turn.id = assessment.turn_id.as_str(),
            item.id = assessment.target_item_id.as_deref(),
            status = status,
            outcome = outcome,
            risk_level = risk_level,
            user_authorization = user_authorization,
            started_at_ms = assessment.started_at_ms,
            completed_at_ms = assessment.completed_at_ms,
            rationale =
                rationale.map(|text| take_bytes_at_char_boundary(text, MAX_RATIONALE_BYTES)),
            rationale_length = rationale.map(|text| text.len() as u64),
            rationale_truncated = rationale.map(|text| text.len() > MAX_RATIONALE_BYTES),
        );
    }
}
