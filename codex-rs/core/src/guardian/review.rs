use std::sync::Arc;

use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::GuardianRiskLevel;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::WarningEvent;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;

use super::GUARDIAN_APPROVAL_RISK_THRESHOLD;
use super::GUARDIAN_REVIEWER_NAME;
use super::GuardianApprovalRequest;
use super::GuardianAssessment;
use super::approval_request::guardian_assessment_action_value;
use super::approval_request::guardian_request_id;
use super::approval_request::guardian_request_turn_id;
use super::prompt::build_guardian_prompt_items;
use super::prompt::guardian_output_schema;
use super::prompt::parse_guardian_assessment;
use super::review_session::GuardianReviewSessionOutcome;
use super::review_session::GuardianReviewSessionParams;
use super::review_session::build_guardian_review_session_config;

pub(crate) const GUARDIAN_REJECTION_MESSAGE: &str = concat!(
    "This action was rejected due to unacceptable risk. ",
    "The agent must not attempt to achieve the same outcome via workaround, ",
    "indirect execution, or policy circumvention. ",
    "Proceed only with a materially safer alternative, ",
    "or if the user explicitly approves the action after being informed of the risk. ",
    "Otherwise, stop and request user input.",
);

#[derive(Debug)]
pub(super) enum GuardianReviewOutcome {
    Completed(anyhow::Result<GuardianAssessment>),
    TimedOut,
    Aborted,
}

fn guardian_risk_level_str(level: GuardianRiskLevel) -> &'static str {
    match level {
        GuardianRiskLevel::Low => "low",
        GuardianRiskLevel::Medium => "medium",
        GuardianRiskLevel::High => "high",
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
    request: GuardianApprovalRequest,
    retry_reason: Option<String>,
    external_cancel: Option<CancellationToken>,
) -> ReviewDecision {
    let assessment_id = guardian_request_id(&request).to_string();
    let assessment_turn_id = guardian_request_turn_id(&request, &turn.sub_id).to_string();
    let action_summary = guardian_assessment_action_value(&request);
    session
        .send_event(
            turn.as_ref(),
            EventMsg::GuardianAssessment(GuardianAssessmentEvent {
                id: assessment_id.clone(),
                turn_id: assessment_turn_id.clone(),
                status: GuardianAssessmentStatus::InProgress,
                risk_score: None,
                risk_level: None,
                rationale: None,
                action: Some(action_summary.clone()),
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
                    id: assessment_id,
                    turn_id: assessment_turn_id,
                    status: GuardianAssessmentStatus::Aborted,
                    risk_score: None,
                    risk_level: None,
                    rationale: None,
                    action: Some(action_summary),
                }),
            )
            .await;
        return ReviewDecision::Abort;
    }

    let terminal_action = action_summary.clone();
    let prompt_items = build_guardian_prompt_items(session.as_ref(), retry_reason, request).await;
    let schema = guardian_output_schema();
    let outcome = run_guardian_review_session(
        session.clone(),
        turn.clone(),
        prompt_items,
        schema,
        external_cancel,
    )
    .await;

    let assessment = match outcome {
        GuardianReviewOutcome::Completed(Ok(assessment)) => assessment,
        GuardianReviewOutcome::Completed(Err(err)) => GuardianAssessment {
            risk_level: GuardianRiskLevel::High,
            risk_score: 100,
            rationale: format!("Automatic approval review failed: {err}"),
            evidence: vec![],
        },
        GuardianReviewOutcome::TimedOut => GuardianAssessment {
            risk_level: GuardianRiskLevel::High,
            risk_score: 100,
            rationale:
                "Automatic approval review timed out while evaluating the requested approval."
                    .to_string(),
            evidence: vec![],
        },
        GuardianReviewOutcome::Aborted => {
            session
                .send_event(
                    turn.as_ref(),
                    EventMsg::GuardianAssessment(GuardianAssessmentEvent {
                        id: assessment_id,
                        turn_id: assessment_turn_id,
                        status: GuardianAssessmentStatus::Aborted,
                        risk_score: None,
                        risk_level: None,
                        rationale: None,
                        action: Some(action_summary),
                    }),
                )
                .await;
            return ReviewDecision::Abort;
        }
    };

    let approved = assessment.risk_score < GUARDIAN_APPROVAL_RISK_THRESHOLD;
    let verdict = if approved { "approved" } else { "denied" };
    let warning = format!(
        "Automatic approval review {verdict} (risk: {}): {}",
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
    session
        .send_event(
            turn.as_ref(),
            EventMsg::GuardianAssessment(GuardianAssessmentEvent {
                id: assessment_id,
                turn_id: assessment_turn_id,
                status,
                risk_score: Some(assessment.risk_score),
                risk_level: Some(assessment.risk_level),
                rationale: Some(assessment.rationale.clone()),
                action: Some(terminal_action),
            }),
        )
        .await;

    if approved {
        ReviewDecision::Approved
    } else {
        ReviewDecision::Denied
    }
}

/// Public entrypoint for approval requests that should be reviewed by guardian.
pub(crate) async fn review_approval_request(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    request: GuardianApprovalRequest,
    retry_reason: Option<String>,
) -> ReviewDecision {
    run_guardian_review(
        Arc::clone(session),
        Arc::clone(turn),
        request,
        retry_reason,
        None,
    )
    .await
}

pub(crate) async fn review_approval_request_with_cancel(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    request: GuardianApprovalRequest,
    retry_reason: Option<String>,
    cancel_token: CancellationToken,
) -> ReviewDecision {
    run_guardian_review(
        Arc::clone(session),
        Arc::clone(turn),
        request,
        retry_reason,
        Some(cancel_token),
    )
    .await
}

/// Runs the guardian in a locked-down reusable review session.
///
/// The guardian itself should not mutate state or trigger further approvals, so
/// it is pinned to a read-only sandbox with `approval_policy = never` and
/// nonessential agent features disabled. The child session is reused across
/// approvals to preserve a stable prompt-cache key, but it is recreated when
/// the effective review-session config changes and its history is cleared
/// before each review so prior guardian decisions do not bleed into later
/// ones. It may still reuse the parent's managed-network allowlist for
/// read-only checks, but it intentionally runs without inherited exec-policy
/// rules.
pub(super) async fn run_guardian_review_session(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    prompt_items: Vec<codex_protocol::user_input::UserInput>,
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
        .list_models(crate::models_manager::manager::RefreshStrategy::Offline)
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
            prompt_items,
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
