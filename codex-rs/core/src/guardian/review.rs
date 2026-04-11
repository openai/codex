use std::sync::Arc;

use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentDecisionSource;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::GuardianRiskLevel;
use codex_protocol::protocol::GuardianUserAuthorization;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::WarningEvent;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;

use super::GUARDIAN_REVIEWER_NAME;
use super::GuardianApprovalRequest;
use super::GuardianAssessment;
use super::GuardianAssessmentOutcome;
use super::GuardianRejection;
use super::approval_request::guardian_assessment_action;
use super::approval_request::guardian_request_target_item_id;
use super::approval_request::guardian_request_turn_id;
use super::prompt::guardian_output_schema;
use super::prompt::parse_guardian_assessment;
use super::review_session::GuardianReviewSessionOutcome;
use super::review_session::GuardianReviewSessionParams;
use super::review_session::build_guardian_review_session_config;

const GUARDIAN_REJECTION_INSTRUCTIONS: &str = concat!(
    "The agent must not attempt to achieve the same outcome via workaround, ",
    "indirect execution, or policy circumvention. ",
    "Proceed only with a materially safer alternative, ",
    "or if the user explicitly approves the action after being informed of the risk. ",
    "Otherwise, stop and request user input.",
);

pub(crate) fn new_guardian_review_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(crate) async fn guardian_rejection_message(session: &Session, review_id: &str) -> String {
    let rejection = session
        .services
        .guardian_rejections
        .lock()
        .await
        .remove(review_id)
        .filter(|rejection| !rejection.rationale.trim().is_empty())
        .unwrap_or_else(|| GuardianRejection {
            rationale: "Guardian denied the action without a specific rationale.".to_string(),
            source: GuardianAssessmentDecisionSource::Agent,
        });
    match rejection.source {
        GuardianAssessmentDecisionSource::Agent => format!(
            "This action was rejected due to unacceptable risk.\nReason: {}\n{}",
            rejection.rationale.trim(),
            GUARDIAN_REJECTION_INSTRUCTIONS
        ),
    }
}

#[derive(Debug)]
pub(super) enum GuardianReviewOutcome {
    Completed(anyhow::Result<GuardianAssessment>),
    TimedOut,
    Aborted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuardianApprovalReviewResult {
    pub(crate) decision: ReviewDecision,
    pub(crate) review: GuardianApprovalReview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuardianApprovalReview {
    pub(crate) status: GuardianApprovalReviewStatus,
    pub(crate) decision: Option<GuardianAssessmentOutcome>,
    pub(crate) risk_level: Option<GuardianRiskLevel>,
    pub(crate) user_authorization: Option<GuardianUserAuthorization>,
    pub(crate) rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GuardianApprovalReviewStatus {
    Approved,
    Denied,
    Aborted,
    Failed,
    TimedOut,
}

fn guardian_risk_level_str(level: GuardianRiskLevel) -> &'static str {
    match level {
        GuardianRiskLevel::Low => "low",
        GuardianRiskLevel::Medium => "medium",
        GuardianRiskLevel::High => "high",
        GuardianRiskLevel::Critical => "critical",
    }
}

/// Whether this turn should route `on-request` approval prompts through the
/// guardian reviewer instead of surfacing them to the user. ARC may still
/// block actions earlier in the flow.
pub(crate) fn routes_approval_to_guardian(turn: &TurnContext) -> bool {
    turn.approval_policy.value() == AskForApproval::OnRequest
        && turn.config.approvals_reviewer == ApprovalsReviewer::GuardianSubagent
}

pub(crate) fn is_guardian_reviewer_source(
    session_source: &codex_protocol::protocol::SessionSource,
) -> bool {
    matches!(
        session_source,
        codex_protocol::protocol::SessionSource::SubAgent(SubAgentSource::Other(name))
            if name == GUARDIAN_REVIEWER_NAME
    )
}

/// This function always fails closed: any timeout, review-session failure, or
/// parse failure is treated as a high-risk denial.
async fn run_guardian_review(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    review_id: String,
    request: GuardianApprovalRequest,
    retry_reason: Option<String>,
    external_cancel: Option<CancellationToken>,
) -> GuardianApprovalReviewResult {
    let target_item_id = guardian_request_target_item_id(&request).map(str::to_string);
    let assessment_turn_id = guardian_request_turn_id(&request, &turn.sub_id).to_string();
    let action_summary = guardian_assessment_action(&request);
    session
        .send_event(
            turn.as_ref(),
            EventMsg::GuardianAssessment(GuardianAssessmentEvent {
                id: review_id.clone(),
                target_item_id: target_item_id.clone(),
                turn_id: assessment_turn_id.clone(),
                status: GuardianAssessmentStatus::InProgress,
                risk_level: None,
                user_authorization: None,
                rationale: None,
                decision_source: None,
                action: action_summary.clone(),
            }),
        )
        .await;

    if external_cancel
        .as_ref()
        .is_some_and(CancellationToken::is_cancelled)
    {
        session
            .send_event(
                turn.as_ref(),
                EventMsg::GuardianAssessment(GuardianAssessmentEvent {
                    id: review_id,
                    target_item_id,
                    turn_id: assessment_turn_id,
                    status: GuardianAssessmentStatus::Aborted,
                    risk_level: None,
                    user_authorization: None,
                    rationale: None,
                    decision_source: Some(GuardianAssessmentDecisionSource::Agent),
                    action: action_summary,
                }),
            )
            .await;
        return GuardianApprovalReviewResult {
            decision: ReviewDecision::Abort,
            review: GuardianApprovalReview {
                status: GuardianApprovalReviewStatus::Aborted,
                decision: None,
                risk_level: None,
                user_authorization: None,
                rationale: None,
            },
        };
    }

    let schema = guardian_output_schema();
    let terminal_action = action_summary.clone();
    let outcome = run_guardian_review_session(
        session.clone(),
        turn.clone(),
        request,
        retry_reason,
        schema,
        external_cancel,
    )
    .await;

    let (assessment, advisory_status) = match outcome {
        GuardianReviewOutcome::Completed(Ok(assessment)) => (assessment, None),
        GuardianReviewOutcome::Completed(Err(err)) => (
            GuardianAssessment {
                risk_level: GuardianRiskLevel::High,
                user_authorization: GuardianUserAuthorization::Unknown,
                outcome: GuardianAssessmentOutcome::Deny,
                rationale: format!("Automatic approval review failed: {err}"),
            },
            Some(GuardianApprovalReviewStatus::Failed),
        ),
        GuardianReviewOutcome::TimedOut => (
            GuardianAssessment {
                risk_level: GuardianRiskLevel::High,
                user_authorization: GuardianUserAuthorization::Unknown,
                outcome: GuardianAssessmentOutcome::Deny,
                rationale:
                    "Automatic approval review timed out while evaluating the requested approval."
                        .to_string(),
            },
            Some(GuardianApprovalReviewStatus::TimedOut),
        ),
        GuardianReviewOutcome::Aborted => {
            session
                .send_event(
                    turn.as_ref(),
                    EventMsg::GuardianAssessment(GuardianAssessmentEvent {
                        id: review_id,
                        target_item_id,
                        turn_id: assessment_turn_id,
                        status: GuardianAssessmentStatus::Aborted,
                        risk_level: None,
                        user_authorization: None,
                        rationale: None,
                        decision_source: Some(GuardianAssessmentDecisionSource::Agent),
                        action: action_summary,
                    }),
                )
                .await;
            return GuardianApprovalReviewResult {
                decision: ReviewDecision::Abort,
                review: GuardianApprovalReview {
                    status: GuardianApprovalReviewStatus::Aborted,
                    decision: None,
                    risk_level: None,
                    user_authorization: None,
                    rationale: None,
                },
            };
        }
    };

    let approved = match assessment.outcome {
        GuardianAssessmentOutcome::Allow => true,
        GuardianAssessmentOutcome::Deny => false,
    };
    let verdict = if approved { "approved" } else { "denied" };
    let user_authorization = match assessment.user_authorization {
        GuardianUserAuthorization::Unknown => "unknown",
        GuardianUserAuthorization::Low => "low",
        GuardianUserAuthorization::Medium => "medium",
        GuardianUserAuthorization::High => "high",
    };
    let warning = format!(
        "Automatic approval review {verdict} (risk: {}, authorization: {user_authorization}): {}",
        guardian_risk_level_str(assessment.risk_level),
        assessment.rationale
    );
    session
        .send_event(
            turn.as_ref(),
            EventMsg::Warning(WarningEvent { message: warning }),
        )
        .await;
    let status = if approved {
        GuardianAssessmentStatus::Approved
    } else {
        GuardianAssessmentStatus::Denied
    };
    let advisory_status = advisory_status.unwrap_or(if approved {
        GuardianApprovalReviewStatus::Approved
    } else {
        GuardianApprovalReviewStatus::Denied
    });
    let decision = if approved {
        ReviewDecision::Approved
    } else {
        ReviewDecision::Denied
    };
    let review = GuardianApprovalReview {
        status: advisory_status,
        decision: Some(assessment.outcome),
        risk_level: Some(assessment.risk_level),
        user_authorization: Some(assessment.user_authorization),
        rationale: Some(assessment.rationale.clone()),
    };
    {
        let mut rationales = session.services.guardian_rejections.lock().await;
        if approved {
            rationales.remove(&review_id);
        } else {
            let rejection = GuardianRejection {
                rationale: assessment.rationale.clone(),
                source: GuardianAssessmentDecisionSource::Agent,
            };
            rationales.insert(review_id.clone(), rejection);
        }
    }
    session
        .send_event(
            turn.as_ref(),
            EventMsg::GuardianAssessment(GuardianAssessmentEvent {
                id: review_id,
                target_item_id,
                turn_id: assessment_turn_id,
                status,
                risk_level: Some(assessment.risk_level),
                user_authorization: Some(assessment.user_authorization),
                rationale: Some(assessment.rationale.clone()),
                decision_source: Some(GuardianAssessmentDecisionSource::Agent),
                action: terminal_action,
            }),
        )
        .await;

    GuardianApprovalReviewResult { decision, review }
}

/// Runs guardian and returns both the approval decision and hook-visible review data.
pub(crate) async fn review_approval_request_with_review(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    review_id: String,
    request: GuardianApprovalRequest,
    retry_reason: Option<String>,
) -> GuardianApprovalReviewResult {
    run_guardian_review(
        Arc::clone(session),
        Arc::clone(turn),
        review_id,
        request,
        retry_reason,
        /*external_cancel*/ None,
    )
    .await
}

/// Public entrypoint for approval requests that should be reviewed by guardian.
pub(crate) async fn review_approval_request(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    review_id: String,
    request: GuardianApprovalRequest,
    retry_reason: Option<String>,
) -> ReviewDecision {
    review_approval_request_with_review(session, turn, review_id, request, retry_reason)
        .await
        .decision
}

pub(crate) async fn review_approval_request_with_cancel(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    review_id: String,
    request: GuardianApprovalRequest,
    retry_reason: Option<String>,
    cancel_token: CancellationToken,
) -> ReviewDecision {
    run_guardian_review(
        Arc::clone(session),
        Arc::clone(turn),
        review_id,
        request,
        retry_reason,
        Some(cancel_token),
    )
    .await
    .decision
}

/// Runs the guardian in a locked-down reusable review session.
///
/// The guardian itself should not mutate state or trigger further approvals, so
/// it is pinned to a read-only sandbox with `approval_policy = never` and
/// nonessential agent features disabled. When the cached trunk session is idle,
/// later approvals append onto that same guardian conversation to preserve a
/// stable prompt-cache key. If the trunk is already busy, the review runs in an
/// ephemeral fork from the last committed trunk rollout so parallel approvals
/// do not block each other or mutate the cached thread. The trunk is recreated
/// when the effective review-session config changes, and any future compaction
/// must continue to preserve the guardian policy as exact top-level developer
/// context. It may still reuse the parent's managed-network allowlist for
/// read-only checks, but it intentionally runs without inherited exec-policy
/// rules.
pub(super) async fn run_guardian_review_session(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    request: GuardianApprovalRequest,
    retry_reason: Option<String>,
    schema: serde_json::Value,
    external_cancel: Option<CancellationToken>,
) -> GuardianReviewOutcome {
    let live_network_config = match session.services.network_proxy.as_ref() {
        Some(network_proxy) => match network_proxy.proxy().current_cfg().await {
            Ok(config) => Some(config),
            Err(err) => return GuardianReviewOutcome::Completed(Err(err)),
        },
        None => None,
    };
    let available_models = session
        .services
        .models_manager
        .list_models(codex_models_manager::manager::RefreshStrategy::Offline)
        .await;
    let preferred_reasoning_effort = |supports_low: bool, fallback| {
        if supports_low {
            Some(codex_protocol::openai_models::ReasoningEffort::Low)
        } else {
            fallback
        }
    };
    let preferred_model = available_models
        .iter()
        .find(|preset| preset.model == super::GUARDIAN_PREFERRED_MODEL);
    let (guardian_model, guardian_reasoning_effort) = if let Some(preset) = preferred_model {
        let reasoning_effort = preferred_reasoning_effort(
            preset
                .supported_reasoning_efforts
                .iter()
                .any(|effort| effort.effort == codex_protocol::openai_models::ReasoningEffort::Low),
            Some(preset.default_reasoning_effort),
        );
        (
            super::GUARDIAN_PREFERRED_MODEL.to_string(),
            reasoning_effort,
        )
    } else {
        let reasoning_effort = preferred_reasoning_effort(
            turn.model_info
                .supported_reasoning_levels
                .iter()
                .any(|preset| preset.effort == codex_protocol::openai_models::ReasoningEffort::Low),
            turn.reasoning_effort
                .or(turn.model_info.default_reasoning_level),
        );
        (turn.model_info.slug.clone(), reasoning_effort)
    };
    let guardian_config = build_guardian_review_session_config(
        turn.config.as_ref(),
        live_network_config.clone(),
        guardian_model.as_str(),
        guardian_reasoning_effort,
    );
    let guardian_config = match guardian_config {
        Ok(config) => config,
        Err(err) => return GuardianReviewOutcome::Completed(Err(err)),
    };

    match session
        .guardian_review_session
        .run_review(GuardianReviewSessionParams {
            parent_session: Arc::clone(&session),
            parent_turn: turn.clone(),
            spawn_config: guardian_config,
            request,
            retry_reason,
            schema,
            model: guardian_model,
            reasoning_effort: guardian_reasoning_effort,
            reasoning_summary: turn.reasoning_summary,
            personality: turn.personality,
            external_cancel,
        })
        .await
    {
        GuardianReviewSessionOutcome::Completed(Ok(last_agent_message)) => {
            GuardianReviewOutcome::Completed(parse_guardian_assessment(
                last_agent_message.as_deref(),
            ))
        }
        GuardianReviewSessionOutcome::Completed(Err(err)) => {
            GuardianReviewOutcome::Completed(Err(err))
        }
        GuardianReviewSessionOutcome::TimedOut => GuardianReviewOutcome::TimedOut,
        GuardianReviewSessionOutcome::Aborted => GuardianReviewOutcome::Aborted,
    }
}
