use std::time::Instant;

use codex_analytics::GuardianApprovalRequestSource;
use codex_analytics::GuardianReviewDecision;
use codex_analytics::GuardianReviewFailureReason;
use codex_analytics::GuardianReviewSessionKind;
use codex_analytics::GuardianReviewTerminalStatus;
use codex_analytics::GuardianReviewedAction;
use codex_analytics::now_unix_seconds;
use codex_features::Feature;
use codex_protocol::protocol::GuardianRiskLevel;
use codex_protocol::protocol::GuardianUserAuthorization;
use codex_protocol::protocol::TokenUsage;

use crate::session::session::Session;
use crate::session::turn_context::TurnContext;

use super::GUARDIAN_REVIEW_TIMEOUT;
use super::GuardianApprovalRequest;
use super::GuardianAssessmentOutcome;

#[derive(Debug, Clone)]
pub(super) struct GuardianReviewSessionMetadata {
    pub(super) guardian_thread_id: String,
    pub(super) guardian_session_kind: GuardianReviewSessionKind,
    pub(super) guardian_model: String,
    pub(super) guardian_reasoning_effort: Option<String>,
    pub(super) had_prior_review_context: bool,
    pub(super) completed_at: u64,
    pub(super) token_usage: Option<TokenUsage>,
}

pub(super) struct GuardianReviewAnalyticsContext {
    thread_id: String,
    turn_id: String,
    review_id: String,
    target_item_id: Option<String>,
    approval_request_source: GuardianApprovalRequestSource,
    reviewed_action: GuardianReviewedAction,
    started_at: u64,
    started_instant: Instant,
}

pub(super) struct GuardianReviewAnalyticsResult {
    pub(super) decision: GuardianReviewDecision,
    pub(super) terminal_status: GuardianReviewTerminalStatus,
    pub(super) failure_reason: Option<GuardianReviewFailureReason>,
    pub(super) risk_level: Option<GuardianRiskLevel>,
    pub(super) user_authorization: Option<GuardianUserAuthorization>,
    pub(super) outcome: Option<GuardianAssessmentOutcome>,
    pub(super) guardian_thread_id: Option<String>,
    pub(super) guardian_session_kind: Option<GuardianReviewSessionKind>,
    pub(super) guardian_model: Option<String>,
    pub(super) guardian_reasoning_effort: Option<String>,
    pub(super) had_prior_review_context: Option<bool>,
    pub(super) reviewed_action_truncated: bool,
    pub(super) token_usage: Option<TokenUsage>,
    pub(super) time_to_first_token_ms: Option<u64>,
    pub(super) completed_at: u64,
}

impl GuardianReviewAnalyticsContext {
    pub(super) fn new(
        session: &Session,
        turn_id: String,
        review_id: String,
        target_item_id: Option<String>,
        approval_request_source: GuardianApprovalRequestSource,
        request: &GuardianApprovalRequest,
    ) -> Self {
        Self {
            thread_id: session.conversation_id.to_string(),
            turn_id,
            review_id,
            target_item_id,
            approval_request_source,
            reviewed_action: guardian_reviewed_action(request),
            started_at: now_unix_seconds(),
            started_instant: Instant::now(),
        }
    }

    pub(super) fn track(
        &self,
        session: &Session,
        turn: &TurnContext,
        result: GuardianReviewAnalyticsResult,
    ) {
        if !turn.config.features.enabled(Feature::GeneralAnalytics) {
            return;
        }
        let completion_latency_ms = self.started_instant.elapsed().as_millis() as u64;
        session
            .services
            .analytics_events_client
            .track_guardian_review(codex_analytics::GuardianReviewEventParams {
                thread_id: self.thread_id.clone(),
                turn_id: self.turn_id.clone(),
                review_id: self.review_id.clone(),
                target_item_id: self.target_item_id.clone(),
                approval_request_source: self.approval_request_source,
                reviewed_action: self.reviewed_action.clone(),
                reviewed_action_truncated: result.reviewed_action_truncated,
                decision: result.decision,
                terminal_status: result.terminal_status,
                failure_reason: result.failure_reason,
                risk_level: result.risk_level,
                user_authorization: result.user_authorization,
                outcome: result.outcome,
                guardian_thread_id: result.guardian_thread_id,
                guardian_session_kind: result.guardian_session_kind,
                guardian_model: result.guardian_model,
                guardian_reasoning_effort: result.guardian_reasoning_effort,
                had_prior_review_context: result.had_prior_review_context,
                review_timeout_ms: GUARDIAN_REVIEW_TIMEOUT.as_millis() as u64,
                // TODO(rhan-oai): plumb nested Guardian review session tool-call counts.
                tool_call_count: None,
                time_to_first_token_ms: result.time_to_first_token_ms,
                completion_latency_ms: Some(completion_latency_ms),
                started_at: self.started_at,
                completed_at: Some(result.completed_at),
                input_tokens: result.token_usage.as_ref().map(|usage| usage.input_tokens),
                cached_input_tokens: result
                    .token_usage
                    .as_ref()
                    .map(|usage| usage.cached_input_tokens),
                output_tokens: result.token_usage.as_ref().map(|usage| usage.output_tokens),
                reasoning_output_tokens: result
                    .token_usage
                    .as_ref()
                    .map(|usage| usage.reasoning_output_tokens),
                total_tokens: result.token_usage.as_ref().map(|usage| usage.total_tokens),
            });
    }
}

impl From<Option<GuardianReviewSessionMetadata>> for GuardianReviewAnalyticsResult {
    fn from(metadata: Option<GuardianReviewSessionMetadata>) -> Self {
        let completed_at = metadata
            .as_ref()
            .map_or_else(now_unix_seconds, |metadata| metadata.completed_at);
        let mut result = Self {
            decision: GuardianReviewDecision::Denied,
            terminal_status: GuardianReviewTerminalStatus::FailedClosed,
            failure_reason: None,
            risk_level: None,
            user_authorization: None,
            outcome: None,
            guardian_thread_id: None,
            guardian_session_kind: None,
            guardian_model: None,
            guardian_reasoning_effort: None,
            had_prior_review_context: None,
            reviewed_action_truncated: false,
            token_usage: None,
            time_to_first_token_ms: None,
            completed_at,
        };

        if let Some(metadata) = metadata {
            result.guardian_thread_id = Some(metadata.guardian_thread_id);
            result.guardian_session_kind = Some(metadata.guardian_session_kind);
            result.guardian_model = Some(metadata.guardian_model);
            result.guardian_reasoning_effort = metadata.guardian_reasoning_effort;
            result.had_prior_review_context = Some(metadata.had_prior_review_context);
            result.token_usage = metadata.token_usage;
        }

        result
    }
}

fn guardian_reviewed_action(request: &GuardianApprovalRequest) -> GuardianReviewedAction {
    match request {
        GuardianApprovalRequest::Shell {
            sandbox_permissions,
            additional_permissions,
            ..
        } => GuardianReviewedAction::Shell {
            sandbox_permissions: *sandbox_permissions,
            additional_permissions: additional_permissions.clone(),
        },
        GuardianApprovalRequest::ExecCommand {
            sandbox_permissions,
            additional_permissions,
            tty,
            ..
        } => GuardianReviewedAction::UnifiedExec {
            sandbox_permissions: *sandbox_permissions,
            additional_permissions: additional_permissions.clone(),
            tty: *tty,
        },
        #[cfg(unix)]
        GuardianApprovalRequest::Execve {
            source,
            program,
            additional_permissions,
            ..
        } => GuardianReviewedAction::Execve {
            source: *source,
            program: program.clone(),
            additional_permissions: additional_permissions.clone(),
        },
        GuardianApprovalRequest::ApplyPatch { .. } => GuardianReviewedAction::ApplyPatch {},
        GuardianApprovalRequest::NetworkAccess { protocol, port, .. } => {
            GuardianReviewedAction::NetworkAccess {
                protocol: *protocol,
                port: *port,
            }
        }
        GuardianApprovalRequest::McpToolCall {
            server,
            tool_name,
            connector_id,
            connector_name,
            tool_title,
            ..
        } => GuardianReviewedAction::McpToolCall {
            server: server.clone(),
            tool_name: tool_name.clone(),
            connector_id: connector_id.clone(),
            connector_name: connector_name.clone(),
            tool_title: tool_title.clone(),
        },
    }
}
