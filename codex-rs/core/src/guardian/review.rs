use std::sync::Arc;
use std::time::Instant;

use codex_analytics::GuardianReviewDecision;
use codex_analytics::GuardianReviewFailureKind;
use codex_analytics::GuardianReviewTerminalStatus;
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
use super::approval_request::guardian_assessment_action;
use super::approval_request::guardian_request_id;
use super::approval_request::guardian_request_turn_id;
use super::prompt::build_guardian_prompt_items;
use super::prompt::guardian_output_schema;
use super::prompt::parse_guardian_assessment;
use super::review_analytics::GuardianReviewAnalyticsInput;
use super::review_analytics::duration_millis_u64;
use super::review_analytics::guardian_reviewed_action;
use super::review_analytics::now_unix_timestamp_secs;
use super::review_analytics::track_guardian_review;
use super::review_session::GuardianReviewSessionOutcome;
use super::review_session::GuardianReviewSessionParams;
use super::review_session::build_guardian_review_session_config;
use super::review_session_analytics::GuardianReviewSessionReport;

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
    Completed {
        result: anyhow::Result<GuardianAssessment>,
        report: Option<GuardianReviewSessionReport>,
        failure_kind: Option<GuardianReviewFailureKind>,
    },
    TimedOut {
        report: Option<GuardianReviewSessionReport>,
    },
    Aborted {
        report: Option<GuardianReviewSessionReport>,
    },
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
    delegated_review: bool,
) -> ReviewDecision {
    let review_started_at = Instant::now();
    let started_at = now_unix_timestamp_secs();
    let assessment_id = guardian_request_id(&request).to_string();
    let assessment_turn_id = guardian_request_turn_id(&request, &turn.sub_id).to_string();
    let action_summary = guardian_assessment_action(&request);
    let (trigger, reviewed_action, reviewed_action_truncated) = guardian_reviewed_action(&request);
    let retry_reason_for_analytics = retry_reason.clone();
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
                action: action_summary.clone(),
            }),
        )
        .await;

    if external_cancel
        .as_ref()
        .is_some_and(CancellationToken::is_cancelled)
    {
        track_guardian_review(
            session.as_ref(),
            GuardianReviewAnalyticsInput {
                review_id: assessment_id.clone(),
                target_item_id: assessment_id.clone(),
                turn_id: assessment_turn_id.clone(),
                trigger,
                retry_reason: retry_reason_for_analytics,
                delegated_review,
                reviewed_action,
                reviewed_action_truncated,
                decision: GuardianReviewDecision::Aborted,
                terminal_status: GuardianReviewTerminalStatus::Aborted,
                failure_kind: Some(GuardianReviewFailureKind::Cancelled),
                assessment: None,
                report: None,
                started_at,
                completed_at: Some(now_unix_timestamp_secs()),
                completion_latency_ms: Some(duration_millis_u64(review_started_at.elapsed())),
            },
        )
        .await;
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
                    action: action_summary,
                }),
            )
            .await;
        return ReviewDecision::Abort;
    }

    let schema = guardian_output_schema();
    let terminal_action = action_summary.clone();
    let outcome = match build_guardian_prompt_items(session.as_ref(), retry_reason, request).await {
        Ok(prompt_items) => {
            run_guardian_review_session(
                session.clone(),
                turn.clone(),
                prompt_items,
                schema,
                external_cancel,
            )
            .await
        }
        Err(err) => GuardianReviewOutcome::Completed {
            result: Err(err.into()),
            report: None,
            failure_kind: Some(GuardianReviewFailureKind::PromptBuildError),
        },
    };

    let (assessment, report, failure_kind) = match outcome {
        GuardianReviewOutcome::Completed {
            result: Ok(assessment),
            report,
            failure_kind,
        } => (assessment, report, failure_kind),
        GuardianReviewOutcome::Completed {
            result: Err(err),
            report,
            failure_kind,
        } => (
            GuardianAssessment {
                risk_level: GuardianRiskLevel::High,
                risk_score: 100,
                rationale: format!("Automatic approval review failed: {err}"),
                evidence: vec![],
            },
            report,
            failure_kind.or(Some(GuardianReviewFailureKind::SessionError)),
        ),
        GuardianReviewOutcome::TimedOut { report } => (
            GuardianAssessment {
                risk_level: GuardianRiskLevel::High,
                risk_score: 100,
                rationale:
                    "Automatic approval review timed out while evaluating the requested approval."
                        .to_string(),
                evidence: vec![],
            },
            report,
            Some(GuardianReviewFailureKind::Timeout),
        ),
        GuardianReviewOutcome::Aborted { report } => {
            track_guardian_review(
                session.as_ref(),
                GuardianReviewAnalyticsInput {
                    review_id: assessment_id.clone(),
                    target_item_id: assessment_id.clone(),
                    turn_id: assessment_turn_id.clone(),
                    trigger,
                    retry_reason: retry_reason_for_analytics,
                    delegated_review,
                    reviewed_action,
                    reviewed_action_truncated,
                    decision: GuardianReviewDecision::Aborted,
                    terminal_status: GuardianReviewTerminalStatus::Aborted,
                    failure_kind: Some(GuardianReviewFailureKind::Cancelled),
                    assessment: None,
                    report,
                    started_at,
                    completed_at: Some(now_unix_timestamp_secs()),
                    completion_latency_ms: Some(duration_millis_u64(review_started_at.elapsed())),
                },
            )
            .await;
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
                        action: action_summary,
                    }),
                )
                .await;
            return ReviewDecision::Abort;
        }
    };

    let approved = assessment.risk_score < GUARDIAN_APPROVAL_RISK_THRESHOLD;
    let decision = if approved {
        GuardianReviewDecision::Approved
    } else {
        GuardianReviewDecision::Denied
    };
    let terminal_status = if approved {
        GuardianReviewTerminalStatus::Approved
    } else if matches!(failure_kind, Some(GuardianReviewFailureKind::Timeout)) {
        GuardianReviewTerminalStatus::TimedOut
    } else if failure_kind.is_some() {
        GuardianReviewTerminalStatus::FailedClosed
    } else {
        GuardianReviewTerminalStatus::Denied
    };
    track_guardian_review(
        session.as_ref(),
        GuardianReviewAnalyticsInput {
            review_id: assessment_id.clone(),
            target_item_id: assessment_id.clone(),
            turn_id: assessment_turn_id.clone(),
            trigger,
            retry_reason: retry_reason_for_analytics,
            delegated_review,
            reviewed_action,
            reviewed_action_truncated,
            decision,
            terminal_status,
            failure_kind,
            assessment: Some(&assessment),
            report,
            started_at,
            completed_at: Some(now_unix_timestamp_secs()),
            completion_latency_ms: Some(duration_millis_u64(review_started_at.elapsed())),
        },
    )
    .await;
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
                action: terminal_action,
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
        /*external_cancel*/ None,
        /*delegated_review*/ false,
    )
    .await
}

#[cfg(test)]
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
        /*delegated_review*/ false,
    )
    .await
}

pub(crate) async fn review_delegated_approval_request_with_cancel(
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
        /*delegated_review*/ true,
    )
    .await
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
    prompt_items: Vec<codex_protocol::user_input::UserInput>,
    schema: serde_json::Value,
    external_cancel: Option<CancellationToken>,
) -> GuardianReviewOutcome {
    let live_network_config = match session.services.network_proxy.as_ref() {
        Some(network_proxy) => match network_proxy.proxy().current_cfg().await {
            Ok(config) => Some(config),
            Err(err) => {
                return GuardianReviewOutcome::Completed {
                    result: Err(err),
                    report: None,
                    failure_kind: Some(GuardianReviewFailureKind::SessionError),
                };
            }
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
        Err(err) => {
            return GuardianReviewOutcome::Completed {
                result: Err(err),
                report: None,
                failure_kind: Some(GuardianReviewFailureKind::SessionError),
            };
        }
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
        GuardianReviewSessionOutcome::Completed { result, report } => match result {
            Ok(last_agent_message) => {
                match parse_guardian_assessment(last_agent_message.as_deref()) {
                    Ok(assessment) => GuardianReviewOutcome::Completed {
                        result: Ok(assessment),
                        report,
                        failure_kind: None,
                    },
                    Err(err) => GuardianReviewOutcome::Completed {
                        result: Err(err),
                        report,
                        failure_kind: Some(GuardianReviewFailureKind::ParseError),
                    },
                }
            }
            Err(err) => GuardianReviewOutcome::Completed {
                result: Err(err),
                report,
                failure_kind: Some(GuardianReviewFailureKind::SessionError),
            },
        },
        GuardianReviewSessionOutcome::TimedOut { report } => {
            GuardianReviewOutcome::TimedOut { report }
        }
        GuardianReviewSessionOutcome::Aborted { report } => {
            GuardianReviewOutcome::Aborted { report }
        }
    }
}
