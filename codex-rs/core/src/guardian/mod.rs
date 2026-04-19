//! Guardian review decides whether an `on-request` approval should be granted
//! automatically instead of shown to the user.
//!
//! High-level approach:
//! 1. Reconstruct a compact transcript that preserves user intent plus the most
//!    relevant recent assistant and tool context.
//! 2. Ask a dedicated guardian review session to assess the exact planned
//!    action and return strict JSON.
//!    The guardian clones the parent config, so it inherits any managed
//!    network proxy / allowlist that the parent turn already had.
//! 3. Fail closed on timeout, execution failure, or malformed output.
//! 4. Apply the guardian's explicit allow/deny outcome.

mod approval_request;
mod observability;
mod prompt;
mod review;
mod review_session;

use std::time::Duration;

use codex_protocol::protocol::GuardianAssessmentDecisionSource;
use codex_protocol::protocol::GuardianAssessmentOutcome;
use serde::Deserialize;
use serde::Serialize;

pub(crate) use approval_request::GuardianApprovalRequest;
pub(crate) use approval_request::GuardianMcpAnnotations;
pub(crate) use approval_request::GuardianNetworkAccessTrigger;
pub(crate) use approval_request::guardian_approval_request_to_json;
pub(crate) use review::guardian_rejection_message;
#[cfg(test)]
pub(crate) use review::guardian_review_trace_for_test;
pub(crate) use review::guardian_timeout_message;
pub(crate) use review::is_guardian_reviewer_source;
pub(crate) use review::new_guardian_review_id;
#[cfg(test)]
pub(crate) use review::record_guardian_denial_for_test;
pub(crate) use review::review_approval_request;
#[cfg(test)]
pub(crate) use review::review_approval_request_with_cancel;
pub(crate) use review::review_approval_request_with_cancel_and_trace;
pub(crate) use review::routes_approval_to_guardian;
pub(crate) use review::spawn_approval_request_review;
pub(crate) use review_session::GuardianReviewSessionManager;
#[cfg(test)]
pub(crate) use review_session::guardian_review_submit_trace_for_test;

const GUARDIAN_PREFERRED_MODEL: &str = "codex-auto-review";
pub(crate) const GUARDIAN_REVIEW_TIMEOUT: Duration = Duration::from_secs(90);
pub(crate) const GUARDIAN_REVIEWER_NAME: &str = "guardian";
pub(crate) const MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN: u32 = 3;
pub(crate) const MAX_TOTAL_GUARDIAN_DENIALS_PER_TURN: u32 = 10;
const GUARDIAN_MAX_MESSAGE_TRANSCRIPT_TOKENS: usize = 10_000;
const GUARDIAN_MAX_TOOL_TRANSCRIPT_TOKENS: usize = 10_000;
const GUARDIAN_MAX_MESSAGE_ENTRY_TOKENS: usize = 2_000;
const GUARDIAN_MAX_TOOL_ENTRY_TOKENS: usize = 1_000;
const GUARDIAN_MAX_ACTION_STRING_TOKENS: usize = 16_000;
const GUARDIAN_RECENT_ENTRY_LIMIT: usize = 40;
const TRUNCATION_TAG: &str = "truncated";

/// Structured output contract that the guardian reviewer must satisfy.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct GuardianAssessment {
    pub(crate) risk_level: codex_protocol::protocol::GuardianRiskLevel,
    pub(crate) user_authorization: codex_protocol::protocol::GuardianUserAuthorization,
    pub(crate) outcome: GuardianAssessmentOutcome,
    pub(crate) rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuardianRejection {
    pub(crate) rationale: String,
    pub(crate) source: GuardianAssessmentDecisionSource,
}

#[derive(Debug, Default)]
pub(crate) struct GuardianRejectionCircuitBreaker {
    turns: std::collections::HashMap<String, GuardianRejectionCircuitBreakerTurn>,
}

#[derive(Debug, Default)]
struct GuardianRejectionCircuitBreakerTurn {
    consecutive_denials: u32,
    total_denials: u32,
    interrupt_triggered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GuardianRejectionCircuitBreakerAction {
    Continue,
    InterruptTurn {
        consecutive_denials: u32,
        total_denials: u32,
    },
}

impl GuardianRejectionCircuitBreaker {
    pub(crate) fn clear_turn(&mut self, turn_id: &str) {
        self.turns.remove(turn_id);
    }

    pub(crate) fn record_denial(&mut self, turn_id: &str) -> GuardianRejectionCircuitBreakerAction {
        let turn = self.turns.entry(turn_id.to_string()).or_default();
        turn.consecutive_denials = turn.consecutive_denials.saturating_add(1);
        turn.total_denials = turn.total_denials.saturating_add(1);
        if !turn.interrupt_triggered
            && (turn.consecutive_denials >= MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN
                || turn.total_denials >= MAX_TOTAL_GUARDIAN_DENIALS_PER_TURN)
        {
            turn.interrupt_triggered = true;
            GuardianRejectionCircuitBreakerAction::InterruptTurn {
                consecutive_denials: turn.consecutive_denials,
                total_denials: turn.total_denials,
            }
        } else {
            GuardianRejectionCircuitBreakerAction::Continue
        }
    }

    pub(crate) fn record_non_denial(&mut self, turn_id: &str) {
        let turn = self.turns.entry(turn_id.to_string()).or_default();
        turn.consecutive_denials = 0;
    }
}

#[cfg(test)]
use approval_request::format_guardian_action_pretty;
#[cfg(test)]
use approval_request::guardian_assessment_action;
#[cfg(test)]
use approval_request::guardian_request_turn_id;
#[cfg(test)]
use prompt::GuardianPromptMode;
#[cfg(test)]
use prompt::GuardianPromptModeKind;
#[cfg(test)]
use prompt::GuardianTranscriptCursor;
#[cfg(test)]
use prompt::GuardianTranscriptEntry;
#[cfg(test)]
use prompt::GuardianTranscriptEntryKind;
#[cfg(test)]
use prompt::build_guardian_prompt_items;
#[cfg(test)]
use prompt::collect_guardian_transcript_entries;
#[cfg(test)]
use prompt::guardian_output_schema;
#[cfg(test)]
pub(crate) use prompt::guardian_policy_prompt;
#[cfg(test)]
pub(crate) use prompt::guardian_policy_prompt_with_config;
#[cfg(test)]
use prompt::guardian_truncate_text;
#[cfg(test)]
use prompt::parse_guardian_assessment;
#[cfg(test)]
use prompt::render_guardian_transcript_entries;
#[cfg(test)]
use review::GuardianReviewOutcome;
#[cfg(test)]
use review::run_guardian_review_session as run_guardian_review_session_for_test;
#[cfg(test)]
use review_session::build_guardian_review_session_config as build_guardian_review_session_config_for_test;

#[cfg(test)]
mod tests;
