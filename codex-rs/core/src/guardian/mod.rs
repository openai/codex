//! Hosts approval decisions and the isolated synchronous reviewer.
//! The extension chooses policy and evidence; core enforces permissions and mandatory
//! review requirements. Each approval retains its issuing context and cancellation.

mod approval_request;
mod coverage;
mod decision;
mod feedback;
mod input_budget;
mod prompt;
pub(crate) use input_budget::PendingReviewContext;
pub(crate) use input_budget::check_pending as check_pending_guardian_input;
pub(crate) use input_budget::finalize as finalize_guardian_input;
mod request_budget;
pub(crate) use request_budget::ExhaustedReviewBudget;
pub(crate) use request_budget::check_prompt as check_guardian_prompt_budget;
pub(crate) use request_budget::observe as observe_guardian_request;
mod review;
mod review_session;
mod reviewer_config;
mod runtime;

use std::sync::Arc;

use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::Personality;
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
pub(crate) use codex_guardian_reviewer::guardian_timeout_message;
pub(crate) use decision::decide_approval;
pub(crate) use decision::spawn_approval_decision;
pub(crate) use prompt::BUNDLED_GUARDIAN_POLICY;
pub(crate) use prompt::BUNDLED_GUARDIAN_POLICY_TEMPLATE;
pub(crate) use prompt::guardian_truncate_text;
pub(crate) use review::GuardianReviewOptions;
pub(crate) use review::is_basic_session_source;
pub(crate) use review::new_guardian_review_id;
#[cfg(test)]
pub(crate) use review::record_guardian_denial_for_test;
pub(crate) use review::routes_approval_policy_to_guardian;
pub(crate) use review::routes_approval_to_guardian;
pub use review_session::GuardianReviewSessionHost;
pub(crate) use review_session::GuardianReviewSessionManager;
pub(crate) use review_session::prewarm_guardian_review_session;
pub(crate) use review_session::prompt_cache_key_override_for_review_session;
pub(crate) use runtime::ReviewAction;

pub(crate) use codex_guardian_reviewer::REVIEW_TIMEOUT as GUARDIAN_REVIEW_TIMEOUT;
pub(crate) const GUARDIAN_REVIEWER_NAME: &str = "guardian";
pub(crate) const AUTO_REVIEW_DENIED_ACTION_APPROVAL_DEVELOPER_PREFIX: &str =
    codex_guardian_context::MANUAL_APPROVAL_DEVELOPER_PREFIX;
const GUARDIAN_MAX_TOOL_ENTRY_TOKENS: usize = codex_guardian_context::ContextProfile::synchronous()
    .transcript
    .entry_limits
    .tool_tokens;
pub(crate) const GUARDIAN_MAX_ROOT_MESSAGE_TOKENS: usize = 900;
pub(crate) const GUARDIAN_MAX_NODE_REPL_TOOL_RESULT_TOKENS: usize = 6_000;

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
    pub(crate) model_info: Arc<ModelInfo>,
    pub(crate) reasoning_effort: Option<ReasoningEffort>,
    pub(crate) reasoning_summary: ReasoningSummary,
    pub(crate) personality: Option<Personality>,
    pub(crate) approval_policy: AskForApproval,
    pub(crate) approvals_reviewer: ApprovalsReviewer,
}

impl GuardianReviewContext {
    pub(crate) fn from_resolved_settings(
        turn: Arc<TurnContext>,
        settings: &ResolvedStepSettings,
        environments: &TurnEnvironmentSnapshot,
    ) -> Self {
        Self {
            parent_response_id: turn
                .extension_data
                .get::<codex_api::ResponseId>()
                .map(|id| id.0.clone()),
            environments: environments.clone(),
            model_info: Arc::clone(&settings.model_info),
            reasoning_effort: settings.reasoning_effort().cloned(),
            reasoning_summary: settings.reasoning_summary,
            personality: settings.personality(),
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
            personality: step.settings.personality(),
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
            personality: turn.personality(),
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

#[cfg(test)]
use codex_guardian_reviewer::guardian_output_schema;

pub(crate) use approval_request::format_guardian_action_pretty;
#[cfg(test)]
use approval_request::guardian_assessment_action;
#[cfg(test)]
use approval_request::guardian_request_turn_id;
#[cfg(test)]
use codex_guardian_reviewer::GuardianReviewOutcome;
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
use review::run_guardian_review_session_with_retry as run_guardian_review_session_for_test;
#[cfg(test)]
use review_session::build_guardian_review_session_config as build_guardian_review_session_config_for_test;

#[cfg(test)]
mod tests;
