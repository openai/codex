//! Captures one approval action for the extension-owned synchronous reviewer.

use codex_protocol::protocol::ReviewDecision;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use super::ApprovalRequestReasons;
use super::GuardianApprovalRequest;
use super::GuardianReviewContext;
use super::GuardianReviewOptions;
use super::approval_request::guardian_approval_request_to_json;
use crate::session::session::Session;

/// Carries the original action even when the legacy synchronous renderer cannot handle its paths.
/// This lets the extension return AskUser before invoking that renderer.
#[derive(Clone)]
pub(crate) struct ReviewAction {
    pub(crate) action: Result<serde_json::Value, String>,
    pub(crate) category: codex_protocol::openai_models::GuardianScope,
    pub(crate) request: Result<GuardianApprovalRequest, String>,
}

impl From<GuardianApprovalRequest> for ReviewAction {
    fn from(request: GuardianApprovalRequest) -> Self {
        Self {
            action: guardian_approval_request_to_json(&request).map_err(|error| error.to_string()),
            category: request.guardian_scope(),
            request: Ok(request),
        }
    }
}

impl ReviewAction {
    pub(crate) fn from_approval_action(
        action: crate::tools::sandboxing::ApprovalAction,
        exec_command_cwd_convention: Option<codex_utils_path_uri::PathConvention>,
    ) -> Self {
        let category = action.guardian_scope();
        let original = serde_json::to_value(&action).map_err(|error| error.to_string());
        match action.into_guardian_request(exec_command_cwd_convention) {
            Ok(request) => Self {
                category,
                ..Self::from(request)
            },
            Err(error) => Self {
                action: original,
                category,
                request: Err(error.to_string()),
            },
        }
    }
}

impl ReviewAction {
    /// Preserve the checks previously made before entering Guardian from tool approvals.
    pub(super) fn validate(
        &self,
        context: &GuardianReviewContext,
    ) -> Result<&GuardianApprovalRequest, ReviewDecision> {
        let request = self.request.as_ref().map_err(|error| {
            tracing::error!(%error, "failed to build automatic approval action");
            ReviewDecision::denied("automatic approval review could not prepare the action")
        })?;
        if let Some(environment_id) = request.target_environment_id()
            && !context
                .environments()
                .turn_environments()
                .any(|environment| environment.selection.environment_id == environment_id)
        {
            let message = if matches!(request, GuardianApprovalRequest::WriteStdin { .. }) {
                "automatic approval review cannot access the terminal's environment; select it before retrying"
            } else {
                "automatic approval review cannot access the request's environment"
            };
            return Err(ReviewDecision::denied(message));
        }
        Ok(request)
    }
}

#[derive(Clone)]
pub(super) struct ReviewRuntime {
    pub(super) session: Arc<Session>,
    pub(super) history_reset: CancellationToken,
    pub(super) context: GuardianReviewContext,
    pub(super) request: ReviewAction,
    pub(super) reasons: ApprovalRequestReasons,
    pub(super) options: GuardianReviewOptions,
}
