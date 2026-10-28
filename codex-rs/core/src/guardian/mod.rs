//! Hosts approval decisions and the isolated synchronous reviewer.
//! The extension chooses policy and evidence; core enforces permissions and mandatory
//! review requirements. Each approval retains its issuing context and cancellation.

mod approval_request;
mod coverage;
mod decision;
mod feedback;
mod input_budget;
mod permissions;
mod prompt;
pub(crate) use input_budget::PendingReviewContext;
pub(crate) use input_budget::check_pending as check_pending_guardian_input;
pub(crate) use input_budget::finalize as finalize_guardian_input;
pub(crate) use input_budget::should_compact as should_compact_guardian_input;
pub(crate) use permissions::for_tool as tool_permission_context;
mod request_budget;
pub(crate) use request_budget::ExhaustedReviewBudget;
pub(crate) use request_budget::prepare_prompt as prepare_guardian_prompt;
mod review;
mod review_session;
mod reviewer_config;
pub(crate) use reviewer_config::resolve_review_model;
mod routing;
pub(crate) use routing::ParentResponse;
pub(crate) use routing::PendingReviewRouting;
mod runtime;
#[cfg(test)]
pub(crate) mod test_host;

use codex_protocol::items::ModelInvocationContext;
use std::sync::Arc;

use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GuardianAssessmentOutcome;

use crate::agents_md::LoadedAgentsMd;
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
pub(crate) use codex_guardian_reviewer::guardian_truncate_text;
pub(crate) use codex_guardian_reviewer::is_basic_session_source;
pub(crate) use decision::decide_approval;
pub(crate) use decision::spawn_approval_decision;
pub(crate) use review::GuardianReviewOptions;
pub(crate) use review::new_guardian_review_id;
pub(crate) use review::routes_approval_policy_to_guardian;
pub use review_session::GuardianReviewSession;
pub(crate) use review_session::GuardianReviewSessionManager;
pub use review_session::GuardianReviewState;
pub use review_session::PreparedGuardianContext;
pub use review_session::prepare_review_prewarm;

pub(crate) use codex_guardian_reviewer::prompt_cache_key_override_for_review_session;
pub(crate) use runtime::ReviewAction;
pub(crate) use runtime::from_approval_action;

pub(crate) use codex_guardian_reviewer::GUARDIAN_REVIEWER_NAME;
pub(crate) use codex_guardian_reviewer::REVIEW_TIMEOUT as GUARDIAN_REVIEW_TIMEOUT;
pub(crate) const AUTO_REVIEW_DENIED_ACTION_APPROVAL_DEVELOPER_PREFIX: &str =
    codex_guardian_context::MANUAL_APPROVAL_DEVELOPER_PREFIX;
const GUARDIAN_MAX_TOOL_ENTRY_TOKENS: usize = {
    const PROFILE: &codex_guardian_context::ContextProfile =
        &codex_guardian_context::ContextProfile::synchronous();
    PROFILE.transcript.entry_limits.tool_tokens
};
pub(crate) const GUARDIAN_MAX_ROOT_MESSAGE_TOKENS: usize = 900;
pub(crate) use codex_guardian_reviewer::GUARDIAN_MAX_NODE_REPL_TOOL_RESULT_TOKENS;

/// Captures review inputs from the issuing step without retaining its MCP bindings or tool router.
/// Background network approvals and Unix interception use the active task's resolved settings.
/// Startup reviewer prewarming intentionally uses turn-only inputs because it has no issuing step.
///
/// MCP elicitations use their live invocation's issuing step when available, otherwise turn inputs.
#[derive(Clone)]
pub(crate) struct GuardianReviewContext {
    /// The latest response ID and its connection affinity when review was requested.
    pub(crate) parent_response: Option<Arc<ParentResponse>>,
    turn: Arc<TurnContext>,
    environments: TurnEnvironmentSnapshot,
    project_instructions: Option<Arc<LoadedAgentsMd>>,
    // Model and reasoning inputs are carried for the follow-up Guardian and V2 migrations.
    pub(crate) model_info: Arc<ModelInfo>,
    pub(crate) reasoning_effort: Option<ReasoningEffort>,
    pub(crate) reasoning_summary: ReasoningSummary,
    pub(crate) personality: Option<Personality>,
    pub(crate) approval_policy: AskForApproval,
    pub(crate) approvals_reviewer: ApprovalsReviewer,
}

impl GuardianReviewContext {
    /// Keep the issuing instructions across retries even if a later turn selects other environments.
    async fn capture_project_instructions(&mut self, session: &crate::session::session::Session) {
        if self.project_instructions.is_none() {
            self.project_instructions = session
                .services
                .agents_md_manager
                .project_snapshot(&self.turn.config, &self.environments)
                .await;
        }
    }

    pub(crate) fn model_context(&self) -> ModelInvocationContext {
        ModelInvocationContext {
            model_slug: self.model_info.slug.clone(),
            reasoning_effort: self
                .reasoning_effort
                .as_ref()
                .or(self.model_info.default_reasoning_level.as_ref())
                .map(ToString::to_string),
        }
    }

    pub(crate) fn from_resolved_settings(
        turn: Arc<TurnContext>,
        settings: &ResolvedStepSettings,
        environments: &TurnEnvironmentSnapshot,
    ) -> Self {
        Self {
            parent_response: turn.extension_data.get::<ParentResponse>(),
            environments: environments.clone(),
            project_instructions: None,
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
            parent_response: step.turn.extension_data.get::<ParentResponse>(),
            turn: Arc::clone(&step.turn),
            environments: step.environments.clone(),
            project_instructions: Some(step.loaded_agents_md.clone().unwrap_or_default()),
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
            parent_response: turn.extension_data.get::<ParentResponse>(),
            environments: turn.initial_environments.clone(),
            project_instructions: None,
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
use prompt::collect_guardian_prompt_items_with_parent_turn;
#[cfg(test)]
use prompt::render_guardian_transcript_entries;
#[cfg(test)]
use review::run_guardian_review_session_with_retry as run_guardian_review_session_for_test;
#[cfg(test)]
use review_session::build_guardian_review_session_config as build_guardian_review_session_config_for_test;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "decision_timing_tests.rs"]
mod decision_timing_tests;
