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
mod metrics;
mod prompt;
mod review;
mod review_session;

use std::sync::Arc;
use std::time::Duration;

use codex_protocol::protocol::GuardianAssessmentOutcome;
use serde::Deserialize;
use serde::Serialize;

use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::step_context::StepContext;
use crate::session::turn_context::TurnContext;
use crate::tools::sandboxing::ApprovalRequestReasons;

pub(crate) use approval_request::GuardianApprovalRequest;
pub(crate) use approval_request::GuardianMcpAnnotations;
pub(crate) use approval_request::GuardianNetworkAccessTrigger;
#[cfg(test)]
pub(crate) use approval_request::guardian_approval_request_to_json;
pub(crate) use prompt::BUNDLED_GUARDIAN_POLICY;
pub(crate) use prompt::guardian_truncate_text;
pub(crate) use review::GuardianReviewOptions;
pub(crate) use review::guardian_timeout_message;
pub(crate) use review::is_basic_session_source;
pub(crate) use review::new_guardian_review_id;
#[cfg(test)]
pub(crate) use review::record_guardian_denial_for_test;
pub(crate) use review::review_approval_request;
pub(crate) use review::review_approval_request_with_cancel;
pub(crate) use review::routes_approval_policy_to_guardian;
pub(crate) use review::routes_approval_to_guardian;
pub(crate) use review::spawn_approval_request_review;
pub(crate) use review_session::GuardianReviewSessionManager;
pub(crate) use review_session::prompt_cache_key_override_for_review_session;

pub(crate) const GUARDIAN_REVIEW_TIMEOUT: Duration = Duration::from_secs(90);
pub(crate) const GUARDIAN_REVIEWER_NAME: &str = "guardian";
pub(crate) const MAX_CONSECUTIVE_CYBER_GUARDIAN_DENIALS_PER_TURN: u32 = 1;
pub(crate) const MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN: u32 = 3;
pub(crate) const MAX_RECENT_CYBER_AUTO_REVIEW_DENIALS_PER_TURN: u32 = 1;
pub(crate) const MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN: u32 = 10;
pub(crate) const AUTO_REVIEW_DENIAL_WINDOW_SIZE: usize = 50;
pub(crate) const AUTO_REVIEW_DENIED_ACTION_APPROVAL_DEVELOPER_PREFIX: &str =
    "The user has manually approved a specific action that was previously `Rejected`.";
const GUARDIAN_MAX_MESSAGE_TRANSCRIPT_TOKENS: usize = 10_000;
const GUARDIAN_MAX_TOOL_TRANSCRIPT_TOKENS: usize = 10_000;
const GUARDIAN_MAX_MESSAGE_ENTRY_TOKENS: usize = 2_000;
const GUARDIAN_MAX_TOOL_ENTRY_TOKENS: usize = 1_000;
pub(crate) const GUARDIAN_MAX_NODE_REPL_TOOL_RESULT_TOKENS: usize = 6_000;
const GUARDIAN_MAX_ACTION_STRING_TOKENS: usize = 16_000;
const GUARDIAN_RECENT_ENTRY_LIMIT: usize = 40;
const TRUNCATION_TAG: &str = "truncated";

/// Built from the originating StepContext when available.
/// There are currently two exceptions-- turn-only callers (background network approvals, reviewer
/// prewarming, etc.) and interactive Unix shells that can outlive the step that started them.
/// TODO(sayan): See if we can find a way to model those as StepContext as well without holding
/// step-scoped things past their lifetime (like MCP bindings)
#[derive(Clone)]
pub(crate) struct GuardianReviewContext {
    turn: Arc<TurnContext>,
    environments: TurnEnvironmentSnapshot,
}

impl GuardianReviewContext {
    pub(crate) fn turn(&self) -> &Arc<TurnContext> {
        &self.turn
    }

    pub(crate) fn environments(&self) -> &TurnEnvironmentSnapshot {
        &self.environments
    }
}

impl From<&Arc<StepContext>> for GuardianReviewContext {
    fn from(step: &Arc<StepContext>) -> Self {
        Self {
            turn: Arc::clone(&step.turn),
            environments: step.environments.clone(),
        }
    }
}

impl From<Arc<TurnContext>> for GuardianReviewContext {
    fn from(turn: Arc<TurnContext>) -> Self {
        Self {
            environments: turn.environments.clone(),
            turn,
        }
    }
}

impl From<&Arc<TurnContext>> for GuardianReviewContext {
    fn from(turn: &Arc<TurnContext>) -> Self {
        Self::from(Arc::clone(turn))
    }
}

/// Structured output contract that the guardian reviewer must satisfy.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct GuardianAssessment {
    pub(crate) risk_level: codex_protocol::protocol::GuardianRiskLevel,
    pub(crate) user_authorization: codex_protocol::protocol::GuardianUserAuthorization,
    pub(crate) outcome: GuardianAssessmentOutcome,
    pub(crate) rationale: String,
}

#[derive(Debug, Default)]
pub(crate) struct GuardianRejectionCircuitBreaker {
    turns: std::collections::HashMap<String, GuardianRejectionCircuitBreakerTurn>,
}

#[derive(Debug, Default)]
struct GuardianRejectionCircuitBreakerTurn {
    consecutive_denials: u32,
    recent_denials: std::collections::VecDeque<bool>,
    interrupt_triggered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GuardianRejectionCircuitBreakerPolicy {
    Standard,
    CyberModel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GuardianRejectionCircuitBreakerAction {
    Continue,
    InterruptTurn {
        consecutive_denials: u32,
        recent_denials: u32,
    },
}

impl GuardianRejectionCircuitBreaker {
    pub(crate) fn clear_turn(&mut self, turn_id: &str) {
        self.turns.remove(turn_id);
    }

    pub(crate) fn record_denial(
        &mut self,
        turn_id: &str,
        policy: GuardianRejectionCircuitBreakerPolicy,
    ) -> GuardianRejectionCircuitBreakerAction {
        let turn = self.turns.entry(turn_id.to_string()).or_default();
        turn.consecutive_denials = turn.consecutive_denials.saturating_add(1);
        Self::record_recent_review(turn, /*denied*/ true);
        let recent_denials = turn.recent_denials.iter().filter(|denied| **denied).count() as u32;
        let (max_consecutive_denials, max_recent_denials) = match policy {
            GuardianRejectionCircuitBreakerPolicy::Standard => (
                MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN,
                MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN,
            ),
            GuardianRejectionCircuitBreakerPolicy::CyberModel => (
                MAX_CONSECUTIVE_CYBER_GUARDIAN_DENIALS_PER_TURN,
                MAX_RECENT_CYBER_AUTO_REVIEW_DENIALS_PER_TURN,
            ),
        };
        if !turn.interrupt_triggered
            && (turn.consecutive_denials >= max_consecutive_denials
                || recent_denials >= max_recent_denials)
        {
            turn.interrupt_triggered = true;
            GuardianRejectionCircuitBreakerAction::InterruptTurn {
                consecutive_denials: turn.consecutive_denials,
                recent_denials,
            }
        } else {
            GuardianRejectionCircuitBreakerAction::Continue
        }
    }

    pub(crate) fn record_non_denial(&mut self, turn_id: &str) {
        let turn = self.turns.entry(turn_id.to_string()).or_default();
        turn.consecutive_denials = 0;
        Self::record_recent_review(turn, /*denied*/ false);
    }

    fn record_recent_review(turn: &mut GuardianRejectionCircuitBreakerTurn, denied: bool) {
        turn.recent_denials.push_back(denied);
        if turn.recent_denials.len() > AUTO_REVIEW_DENIAL_WINDOW_SIZE {
            turn.recent_denials.pop_front();
        }
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
use prompt::GuardianTranscriptCursor;
#[cfg(test)]
use prompt::GuardianTranscriptEntry;
#[cfg(test)]
use prompt::GuardianTranscriptEntryKind;
#[cfg(test)]
use prompt::build_guardian_prompt_items;
#[cfg(test)]
use prompt::build_guardian_prompt_items_with_parent_turn;
#[cfg(test)]
use prompt::collect_guardian_transcript_entries;
#[cfg(test)]
use prompt::guardian_output_schema;
#[cfg(test)]
use prompt::parse_guardian_assessment;
#[cfg(test)]
use prompt::render_guardian_transcript_entries;
#[cfg(test)]
use review::GuardianReviewOutcome;
#[cfg(test)]
use review::run_guardian_review_session_with_retry as run_guardian_review_session_for_test;
#[cfg(test)]
use review_session::build_guardian_review_session_config as build_guardian_review_session_config_for_test;

#[cfg(test)]
mod tests;
