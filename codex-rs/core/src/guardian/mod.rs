//! Hosts approval decisions and the isolated synchronous reviewer.
//! The extension chooses policy and evidence; core enforces permissions and mandatory
//! review requirements. Each approval retains its issuing context and cancellation.

mod approval_request;
mod assessment;
mod coverage;
mod decision;
mod feedback;
mod metrics;
mod prompt;
mod review;
mod review_session;
mod reviewer_config;
mod runtime;

use std::sync::Arc;
use std::time::Duration;

use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GuardianAssessmentOutcome;

use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::step_context::StepContext;
use crate::session::step_settings::ResolvedStepSettings;
use crate::session::turn_context::TurnContext;
use crate::tools::sandboxing::ApprovalRequestReasons;

pub(crate) use approval_request::GuardianApprovalRequest;
pub(crate) use approval_request::GuardianMcpAnnotations;
pub(crate) use approval_request::GuardianNetworkAccessTrigger;
#[cfg(test)]
pub(crate) use approval_request::guardian_approval_request_to_json;
pub(crate) use decision::decide_approval;
pub(crate) use decision::spawn_approval_decision;
pub(crate) use prompt::BUNDLED_GUARDIAN_POLICY;
pub(crate) use prompt::BUNDLED_GUARDIAN_POLICY_TEMPLATE;
pub(crate) use prompt::guardian_truncate_text;
pub(crate) use review::GuardianReviewOptions;
pub(crate) use review::guardian_timeout_message;
pub(crate) use review::is_basic_session_source;
pub(crate) use review::new_guardian_review_id;
#[cfg(test)]
pub(crate) use review::record_guardian_denial_for_test;
pub(crate) use review::routes_approval_policy_to_guardian;
pub(crate) use review::routes_approval_to_guardian;
pub use review_session::GuardianReviewSessionManager;
pub(crate) use review_session::prompt_cache_key_override_for_review_session;
pub(crate) use runtime::ReviewAction;

pub(crate) const GUARDIAN_REVIEW_TIMEOUT: Duration = Duration::from_secs(90);
pub(crate) const GUARDIAN_REVIEWER_NAME: &str = "guardian";
pub(crate) const MAX_CONSECUTIVE_CYBER_GUARDIAN_DENIALS_PER_TURN: u32 = 1;
pub(crate) const MAX_CONSECUTIVE_GUARDIAN_DENIALS_PER_TURN: u32 = 3;
pub(crate) const MAX_RECENT_CYBER_AUTO_REVIEW_DENIALS_PER_TURN: u32 = 1;
pub(crate) const MAX_RECENT_AUTO_REVIEW_DENIALS_PER_TURN: u32 = 10;
pub(crate) const AUTO_REVIEW_DENIAL_WINDOW_SIZE: usize = 50;
pub(crate) const AUTO_REVIEW_DENIED_ACTION_APPROVAL_DEVELOPER_PREFIX: &str =
    codex_guardian_context::MANUAL_APPROVAL_DEVELOPER_PREFIX;
const GUARDIAN_MAX_TOOL_ENTRY_TOKENS: usize = codex_guardian_context::ContextProfile::synchronous()
    .transcript
    .entry_limits
    .tool_tokens;
pub(crate) const GUARDIAN_MAX_ROOT_MESSAGE_TOKENS: usize = 900;
pub(crate) const GUARDIAN_MAX_NODE_REPL_TOOL_RESULT_TOKENS: usize = 6_000;
pub(crate) const GUARDIAN_MAX_ACTION_BYTES: usize = 50_000 * 4;
const GUARDIAN_MAX_ACTION_STRING_TOKENS: usize = 16_000;

/// Captures review inputs from the issuing step without retaining its MCP bindings or tool router.
/// Background network approvals and Unix interception use the active task's resolved settings.
/// Startup reviewer prewarming intentionally uses turn-only inputs because it has no issuing step.
///
/// MCP elicitation reviews continue to use turn-only inputs.
#[derive(Clone)]
pub(crate) struct GuardianReviewContext {
    /// The response currently handled in this execution context.
    pub(crate) parent_response_id: Option<String>,
    turn: Arc<TurnContext>,
    environments: TurnEnvironmentSnapshot,
    // Model and reasoning inputs are carried for the follow-up Guardian and V2 migrations.
    #[expect(dead_code)]
    pub(crate) model_info: Arc<ModelInfo>,
    #[expect(dead_code)]
    pub(crate) reasoning_effort: Option<ReasoningEffort>,
    #[expect(dead_code)]
    pub(crate) reasoning_summary: ReasoningSummary,
    pub(crate) approval_policy: AskForApproval,
    pub(crate) approvals_reviewer: ApprovalsReviewer,
}

impl GuardianReviewContext {
    pub(crate) fn from_resolved_settings(
        turn: Arc<TurnContext>,
        settings: &ResolvedStepSettings,
    ) -> Self {
        Self {
            parent_response_id: turn
                .extension_data
                .get::<codex_api::ResponseId>()
                .map(|id| id.0.clone()),
            environments: turn.environments.clone(),
            model_info: Arc::clone(&settings.model_info),
            reasoning_effort: settings.reasoning_effort().cloned(),
            reasoning_summary: settings.reasoning_summary,
            approval_policy: settings.approval_policy(),
            approvals_reviewer: settings.approvals_reviewer(),
            turn,
        }
    }

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
            parent_response_id: step
                .turn
                .extension_data
                .get::<codex_api::ResponseId>()
                .map(|id| id.0.clone()),
            turn: Arc::clone(&step.turn),
            environments: step.environments.clone(),
            model_info: Arc::clone(&step.settings.model_info),
            reasoning_effort: step.settings.reasoning_effort().cloned(),
            reasoning_summary: step.settings.reasoning_summary,
            approval_policy: step.settings.approval_policy(),
            approvals_reviewer: step.settings.approvals_reviewer(),
        }
    }
}

impl From<Arc<TurnContext>> for GuardianReviewContext {
    fn from(turn: Arc<TurnContext>) -> Self {
        Self {
            parent_response_id: turn
                .extension_data
                .get::<codex_api::ResponseId>()
                .map(|id| id.0.clone()),
            environments: turn.environments.clone(),
            model_info: Arc::clone(turn.model_info()),
            reasoning_effort: turn.reasoning_effort().cloned(),
            reasoning_summary: turn.reasoning_summary(),
            approval_policy: turn.approval_policy(),
            approvals_reviewer: turn.config.approvals_reviewer,
            turn,
        }
    }
}

impl From<&Arc<TurnContext>> for GuardianReviewContext {
    fn from(turn: &Arc<TurnContext>) -> Self {
        Self::from(Arc::clone(turn))
    }
}

pub use assessment::GuardianAssessment;
pub use assessment::guardian_output_schema;
pub use assessment::parse_guardian_assessment;
pub use reviewer_config::build_guardian_review_session_config;

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

pub(crate) use approval_request::format_guardian_action_pretty;
#[cfg(test)]
use approval_request::guardian_assessment_action;
#[cfg(test)]
use approval_request::guardian_request_turn_id;
#[cfg(test)]
use prompt::GuardianPromptMode;
#[cfg(test)]
use prompt::GuardianTranscriptCursor;
#[cfg(test)]
use prompt::build_guardian_prompt_items;
#[cfg(test)]
use prompt::build_guardian_prompt_items_with_parent_turn;
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
