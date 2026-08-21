use codex_analytics::GuardianApprovalRequestSource;
use codex_analytics::GuardianReviewAnalyticsResult;
use codex_analytics::GuardianReviewDecision;
use codex_analytics::GuardianReviewFailureReason;
use codex_analytics::GuardianReviewTerminalStatus;
use codex_analytics::GuardianReviewTrackContext;
use codex_analytics::GuardianReviewedAction;
use codex_core_plugins::PluginCommandAttribution;
use codex_extension_api::ThreadIdleCause;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::openai_models::MODEL_SPECIALTY_CYBER;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentDecisionSource;
use codex_protocol::protocol::GuardianAssessmentEvent;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::GuardianRiskLevel;
use codex_protocol::protocol::GuardianUserAuthorization;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::WarningEvent;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::time::Instant;
use tokio::time::sleep_until;
use tokio_util::sync::CancellationToken;

use crate::codex_thread::GuardianAuthorizationVersion;
use crate::context::GuardianReviewEvidence;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::turn_timing::now_unix_timestamp_ms;
use crate::util::backoff;

use super::AUTO_REVIEW_DENIAL_WINDOW_SIZE;
use super::ApprovalRequestReasons;
use super::GUARDIAN_REVIEW_TIMEOUT;
use super::GUARDIAN_REVIEWER_NAME;
use super::GuardianApprovalRequest;
use super::GuardianAssessment;
use super::GuardianAssessmentOutcome;
use super::GuardianRejectionCircuitBreakerAction;
use super::GuardianRejectionCircuitBreakerPolicy;
use super::GuardianReviewContext;
use super::approval_request::format_guardian_action_pretty;
use super::approval_request::guardian_approval_request_to_json;
use super::approval_request::guardian_assessment_action;
use super::approval_request::guardian_request_target_item_id;
use super::approval_request::guardian_request_turn_id;
use super::approval_request::guardian_reviewed_action;
use super::metrics::emit_guardian_review_metrics;
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

const GUARDIAN_TIMEOUT_INSTRUCTIONS: &str = concat!(
    "The automatic permission approval review did not finish before its deadline. ",
    "Do not assume the action is unsafe based on the timeout alone. ",
    "You may retry once, or ask the user for guidance or explicit approval.",
);

const GUARDIAN_REVIEW_MAX_ATTEMPTS: i64 = 3;

fn plugin_attribution_for_guardian_request(
    turn: &TurnContext,
    request: &GuardianApprovalRequest,
) -> Option<PluginCommandAttribution> {
    match request {
        GuardianApprovalRequest::ExecCommand { command, cwd, .. } => {
            turn.plugin_attribution_for_command(command, cwd)
        }
        #[cfg(unix)]
        GuardianApprovalRequest::Execve {
            program, argv, cwd, ..
        } => {
            let command = if argv.is_empty() {
                vec![program.clone()]
            } else {
                std::iter::once(program.clone())
                    .chain(argv.iter().skip(1).cloned())
                    .collect()
            };
            turn.plugin_attribution_for_command(&command, cwd)
        }
        _ => None,
    }
}

pub(crate) fn new_guardian_review_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(crate) fn guardian_timeout_message(model_info: &ModelInfo) -> String {
    model_info
        .model_messages
        .as_ref()
        .and_then(|messages| messages.auto_review.as_ref())
        .and_then(|messages| messages.timeout_instructions.as_deref())
        .unwrap_or(GUARDIAN_TIMEOUT_INSTRUCTIONS)
        .to_string()
}

#[derive(Debug)]
pub(super) enum GuardianReviewOutcome {
    Completed(GuardianAssessment),
    Error(GuardianReviewError),
}

#[derive(Debug)]
pub(super) enum GuardianReviewError {
    PromptBuild {
        message: String,
    },
    Session {
        message: String,
        error_info: Option<CodexErrorInfo>,
    },
    Parse {
        message: String,
    },
    Timeout,
    Cancelled,
}

impl GuardianReviewError {
    fn prompt_build(err: anyhow::Error) -> Self {
        Self::PromptBuild {
            message: err.to_string(),
        }
    }

    fn session(err: anyhow::Error) -> Self {
        Self::Session {
            message: err.to_string(),
            error_info: None,
        }
    }

    fn session_with_error_info(err: anyhow::Error, error_info: CodexErrorInfo) -> Self {
        Self::Session {
            message: err.to_string(),
            error_info: Some(error_info),
        }
    }

    fn parse(err: anyhow::Error) -> Self {
        Self::Parse {
            message: err.to_string(),
        }
    }

    fn failure_reason(&self) -> GuardianReviewFailureReason {
        match self {
            Self::PromptBuild { .. } => GuardianReviewFailureReason::PromptBuildError,
            Self::Session { .. } => GuardianReviewFailureReason::SessionError,
            Self::Parse { .. } => GuardianReviewFailureReason::ParseError,
            Self::Timeout => GuardianReviewFailureReason::Timeout,
            Self::Cancelled => GuardianReviewFailureReason::Cancelled,
        }
    }
}

fn guardian_risk_level_str(level: GuardianRiskLevel) -> &'static str {
    match level {
        GuardianRiskLevel::Low => "low",
        GuardianRiskLevel::Medium => "medium",
        GuardianRiskLevel::High => "high",
        GuardianRiskLevel::Critical => "critical",
    }
}

/// Whether this turn should route allowed approval prompts through the guardian
/// reviewer instead of surfacing them to the user. ARC may still block actions
/// earlier in the flow.
pub(crate) fn routes_approval_to_guardian(turn: &TurnContext) -> bool {
    routes_approval_to_guardian_with_reviewer(turn, turn.config.approvals_reviewer)
}

/// Whether an approval with its own reviewer selection should be routed through guardian.
pub(crate) fn routes_approval_to_guardian_with_reviewer(
    turn: &TurnContext,
    approvals_reviewer: ApprovalsReviewer,
) -> bool {
    routes_approval_policy_to_guardian(turn.approval_policy(), approvals_reviewer)
}

/// Whether an exact approval policy and reviewer should route through Guardian.
pub(crate) fn routes_approval_policy_to_guardian(
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
) -> bool {
    matches!(
        approval_policy,
        AskForApproval::OnRequest | AskForApproval::Granular(_)
    ) && approvals_reviewer == ApprovalsReviewer::AutoReview
}

pub(crate) fn is_guardian_reviewer_source(
    session_source: &codex_protocol::protocol::SessionSource,
) -> bool {
    matches!(
        session_source,
        codex_protocol::protocol::SessionSource::SubAgent(SubAgentSource::Other(label))
            if label == GUARDIAN_REVIEWER_NAME
    )
}

fn track_guardian_review(
    session: &Session,
    tracking: &GuardianReviewTrackContext,
    approval_request_source: GuardianApprovalRequestSource,
    reviewed_action: &GuardianReviewedAction,
    result: GuardianReviewAnalyticsResult,
    completed_at_ms: u64,
) {
    emit_guardian_review_metrics(
        &session.services.session_telemetry,
        &result,
        approval_request_source,
        reviewed_action,
        completed_at_ms.saturating_sub(tracking.started_at_ms),
    );
    session
        .services
        .analytics_events_client
        .track_guardian_review(tracking, result, completed_at_ms);
}

async fn record_guardian_non_denial(session: &Arc<Session>, turn_id: &str) {
    session
        .services
        .guardian_rejection_circuit_breaker
        .lock()
        .await
        .record_non_denial(turn_id);
}

async fn record_guardian_denial(session: &Arc<Session>, turn: &Arc<TurnContext>, turn_id: &str) {
    let policy = if turn.model_info.model_specialty.as_deref() == Some(MODEL_SPECIALTY_CYBER) {
        GuardianRejectionCircuitBreakerPolicy::CyberModel
    } else {
        GuardianRejectionCircuitBreakerPolicy::Standard
    };
    let action = session
        .services
        .guardian_rejection_circuit_breaker
        .lock()
        .await
        .record_denial(turn_id, policy);
    let GuardianRejectionCircuitBreakerAction::InterruptTurn {
        consecutive_denials,
        recent_denials,
    } = action
    else {
        return;
    };

    if session.turn_context_for_sub_id(turn_id).await.is_none() {
        return;
    }

    session
        .send_event(
            turn.as_ref(),
            EventMsg::GuardianWarning(WarningEvent {
                message: format!(
                    "Automatic approval review rejected too many approval requests for this turn ({consecutive_denials} consecutive, {recent_denials} in the last {AUTO_REVIEW_DENIAL_WINDOW_SIZE} reviews); interrupting the turn."
                ),
            }),
        )
        .await;

    let runtime_handle = session.services.runtime_handle.clone();
    let session = Arc::clone(session);
    let turn_id = turn_id.to_string();
    let _abort_task = runtime_handle.spawn(async move {
        let aborted = session
            .abort_turn_if_active(&turn_id, TurnAbortReason::Interrupted)
            .await;
        if aborted {
            // Guardian aborts bypass normal task completion, so emit its idle lifecycle here.
            // User interrupts deliberately do not take this path.
            session
                .emit_thread_idle_lifecycle_if_idle(ThreadIdleCause::Interrupted)
                .await;
        }
    });
}

#[cfg(test)]
pub(crate) async fn record_guardian_denial_for_test(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    turn_id: &str,
) {
    record_guardian_denial(session, turn, turn_id).await;
}

/// Runs Guardian unless an installed extension explicitly claims the review.
/// Guardian timeouts, review-session failures, and parse failures all block
/// execution, with timeouts surfaced separately from explicit denials.
async fn run_guardian_review(
    session: Arc<Session>,
    context: GuardianReviewContext,
    review_id: String,
    request: GuardianApprovalRequest,
    reasons: ApprovalRequestReasons,
    options: GuardianReviewOptions,
) -> ReviewDecision {
    let turn = Arc::clone(context.turn());
    let requires_synchronous_review = reasons.retry.is_some()
        || matches!(
            &request,
            GuardianApprovalRequest::ExecCommand {
                sandbox_permissions,
                ..
            } if sandbox_permissions.requires_escalated_permissions()
        );
    // Guardian V2 may satisfy ordinary reviews, including required-model reviews, but broader
    // permission requests and retries must run Guardian synchronously.
    if (!turn
        .config
        .config_layer_stack
        .requirements()
        .auto_review_required_for_model(&turn.model_info.slug)
        || turn.config.features.enabled(Feature::GuardianV2))
        && !requires_synchronous_review
        && options
            .external_cancel
            .as_ref()
            .is_none_or(|cancel| !cancel.is_cancelled())
        && let Ok(action) = guardian_approval_request_to_json(&request)
        && let Some(decision) = session
            .services
            .extensions
            .approval_review(
                &session.services.session_extension_data,
                &session.services.thread_extension_data,
                &action.to_string(),
                Some(crate::session::extension_metrics::from_session_telemetry(
                    turn.session_telemetry.clone(),
                )),
            )
            .await
    {
        if decision == ReviewDecision::Approved {
            record_guardian_non_denial(&session, guardian_request_turn_id(&request, &turn.sub_id))
                .await;
        }
        return decision;
    }

    let GuardianReviewOptions {
        plugin_attribution_override,
        approval_request_source,
        external_cancel,
    } = options;
    let target_item_id = guardian_request_target_item_id(&request).map(str::to_string);
    let assessment_turn_id = guardian_request_turn_id(&request, &turn.sub_id).to_string();
    let plugin_attribution = plugin_attribution_override
        .or_else(|| plugin_attribution_for_guardian_request(turn.as_ref(), &request));
    let (plugin_id, script_path) = plugin_attribution
        .as_ref()
        .map(PluginCommandAttribution::serialized_fields)
        .unzip();
    let action_summary = guardian_assessment_action(&request);
    let reviewed_action = guardian_reviewed_action(&request);
    let review_tracking = GuardianReviewTrackContext::new(
        session.thread_id.to_string(),
        assessment_turn_id.clone(),
        review_id.clone(),
        target_item_id.clone(),
        approval_request_source,
        reviewed_action.clone(),
        GUARDIAN_REVIEW_TIMEOUT.as_millis() as u64,
    );
    let started_at_ms = review_tracking.started_at_ms.try_into().unwrap_or_default();
    session
        .send_event(
            turn.as_ref(),
            EventMsg::GuardianAssessment(GuardianAssessmentEvent {
                id: review_id.clone(),
                target_item_id: target_item_id.clone(),
                plugin_id: plugin_id.clone(),
                script_path: script_path.clone(),
                turn_id: assessment_turn_id.clone(),
                started_at_ms,
                completed_at_ms: None,
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
        let completed_at_ms = now_unix_timestamp_ms();
        track_guardian_review(
            session.as_ref(),
            &review_tracking,
            approval_request_source,
            &reviewed_action,
            GuardianReviewAnalyticsResult {
                decision: GuardianReviewDecision::Aborted,
                terminal_status: GuardianReviewTerminalStatus::Aborted,
                failure_reason: Some(GuardianReviewFailureReason::Cancelled),
                ..GuardianReviewAnalyticsResult::without_session()
            },
            completed_at_ms.try_into().unwrap_or_default(),
        );
        session
            .send_event(
                turn.as_ref(),
                EventMsg::GuardianAssessment(GuardianAssessmentEvent {
                    id: review_id,
                    target_item_id,
                    plugin_id: plugin_id.clone(),
                    script_path: script_path.clone(),
                    turn_id: assessment_turn_id.clone(),
                    started_at_ms,
                    completed_at_ms: Some(completed_at_ms),
                    status: GuardianAssessmentStatus::Aborted,
                    risk_level: None,
                    user_authorization: None,
                    rationale: None,
                    decision_source: Some(GuardianAssessmentDecisionSource::Agent),
                    action: action_summary,
                }),
            )
            .await;
        record_guardian_non_denial(&session, &assessment_turn_id).await;
        return ReviewDecision::Abort;
    }

    let schema = guardian_output_schema();
    let terminal_action = action_summary.clone();
    let review_evidence = if let Some(evidence) = session
        .services
        .thread_extension_data
        .get::<GuardianReviewEvidence>()
    {
        // Root rewrites and new user messages during this review make its evidence
        // stale even if it later completes against a newer prompt snapshot.
        let history = session.conversation_history_snapshot().await;
        let authorization_version = GuardianAuthorizationVersion::from_history(history.as_ref());
        let root_authorization_version = session
            .services
            .agent_control
            .root_user_authorization(session.thread_id)
            .await
            .map(|snapshot| snapshot.authorization_version);
        format_guardian_action_pretty(&request).ok().map(|action| {
            (
                evidence,
                action.text,
                authorization_version,
                root_authorization_version,
            )
        })
    } else {
        None
    };
    let (outcome, analytics_result) = Box::pin(run_guardian_review_session_with_retry(
        session.clone(),
        context,
        request,
        reasons,
        schema,
        external_cancel,
        GUARDIAN_REVIEW_MAX_ATTEMPTS,
    ))
    .await;

    let completed_at_ms = now_unix_timestamp_ms();
    let completed_review = matches!(&outcome, GuardianReviewOutcome::Completed(_));
    let (assessment, count_denial_for_circuit_breaker) = match outcome {
        GuardianReviewOutcome::Completed(assessment) => {
            let approved = matches!(assessment.outcome, GuardianAssessmentOutcome::Allow);
            track_guardian_review(
                session.as_ref(),
                &review_tracking,
                approval_request_source,
                &reviewed_action,
                GuardianReviewAnalyticsResult {
                    decision: if approved {
                        GuardianReviewDecision::Approved
                    } else {
                        GuardianReviewDecision::Denied
                    },
                    terminal_status: if approved {
                        GuardianReviewTerminalStatus::Approved
                    } else {
                        GuardianReviewTerminalStatus::Denied
                    },
                    failure_reason: None,
                    risk_level: Some(assessment.risk_level),
                    user_authorization: Some(assessment.user_authorization),
                    outcome: Some(assessment.outcome),
                    ..analytics_result
                },
                completed_at_ms.try_into().unwrap_or_default(),
            );
            let count_denial_for_circuit_breaker =
                matches!(assessment.outcome, GuardianAssessmentOutcome::Deny);
            (assessment, count_denial_for_circuit_breaker)
        }
        GuardianReviewOutcome::Error(error) => match error {
            GuardianReviewError::Timeout => {
                let rationale =
                    "Automatic approval review timed out while evaluating the requested approval."
                        .to_string();
                track_guardian_review(
                    session.as_ref(),
                    &review_tracking,
                    approval_request_source,
                    &reviewed_action,
                    GuardianReviewAnalyticsResult {
                        decision: GuardianReviewDecision::Denied,
                        terminal_status: GuardianReviewTerminalStatus::TimedOut,
                        failure_reason: Some(error.failure_reason()),
                        ..analytics_result
                    },
                    completed_at_ms.try_into().unwrap_or_default(),
                );
                session
                    .send_event(
                        turn.as_ref(),
                        EventMsg::GuardianWarning(WarningEvent {
                            message: rationale.clone(),
                        }),
                    )
                    .await;
                session
                    .send_event(
                        turn.as_ref(),
                        EventMsg::GuardianAssessment(GuardianAssessmentEvent {
                            id: review_id,
                            target_item_id,
                            plugin_id: plugin_id.clone(),
                            script_path: script_path.clone(),
                            turn_id: assessment_turn_id.clone(),
                            started_at_ms,
                            completed_at_ms: Some(completed_at_ms),
                            status: GuardianAssessmentStatus::TimedOut,
                            risk_level: None,
                            user_authorization: None,
                            rationale: Some(rationale),
                            decision_source: Some(GuardianAssessmentDecisionSource::Agent),
                            action: terminal_action,
                        }),
                    )
                    .await;
                record_guardian_non_denial(&session, &assessment_turn_id).await;
                return ReviewDecision::TimedOut;
            }
            GuardianReviewError::Cancelled => {
                track_guardian_review(
                    session.as_ref(),
                    &review_tracking,
                    approval_request_source,
                    &reviewed_action,
                    GuardianReviewAnalyticsResult {
                        decision: GuardianReviewDecision::Aborted,
                        terminal_status: GuardianReviewTerminalStatus::Aborted,
                        failure_reason: Some(error.failure_reason()),
                        ..analytics_result
                    },
                    completed_at_ms.try_into().unwrap_or_default(),
                );
                session
                    .send_event(
                        turn.as_ref(),
                        EventMsg::GuardianAssessment(GuardianAssessmentEvent {
                            id: review_id,
                            target_item_id,
                            plugin_id: plugin_id.clone(),
                            script_path: script_path.clone(),
                            turn_id: assessment_turn_id.clone(),
                            started_at_ms,
                            completed_at_ms: Some(completed_at_ms),
                            status: GuardianAssessmentStatus::Aborted,
                            risk_level: None,
                            user_authorization: None,
                            rationale: None,
                            decision_source: Some(GuardianAssessmentDecisionSource::Agent),
                            action: action_summary,
                        }),
                    )
                    .await;
                record_guardian_non_denial(&session, &assessment_turn_id).await;
                return ReviewDecision::Abort;
            }
            GuardianReviewError::PromptBuild { .. }
            | GuardianReviewError::Session { .. }
            | GuardianReviewError::Parse { .. } => {
                let message = match &error {
                    GuardianReviewError::PromptBuild { message }
                    | GuardianReviewError::Session { message, .. }
                    | GuardianReviewError::Parse { message } => message,
                    GuardianReviewError::Timeout | GuardianReviewError::Cancelled => {
                        "guardian review failed"
                    }
                };
                let rationale = format!("Automatic approval review failed: {message}");
                track_guardian_review(
                    session.as_ref(),
                    &review_tracking,
                    approval_request_source,
                    &reviewed_action,
                    GuardianReviewAnalyticsResult {
                        decision: GuardianReviewDecision::Denied,
                        terminal_status: GuardianReviewTerminalStatus::FailedClosed,
                        failure_reason: Some(error.failure_reason()),
                        ..analytics_result
                    },
                    completed_at_ms.try_into().unwrap_or_default(),
                );
                (
                    GuardianAssessment {
                        risk_level: GuardianRiskLevel::High,
                        user_authorization: GuardianUserAuthorization::Unknown,
                        outcome: GuardianAssessmentOutcome::Deny,
                        rationale,
                    },
                    false,
                )
            }
        },
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
            EventMsg::GuardianWarning(WarningEvent { message: warning }),
        )
        .await;
    let status = if approved {
        GuardianAssessmentStatus::Approved
    } else {
        GuardianAssessmentStatus::Denied
    };
    let assessment_event = GuardianAssessmentEvent {
        id: review_id,
        target_item_id,
        plugin_id: plugin_id.clone(),
        script_path: script_path.clone(),
        turn_id: assessment_turn_id.clone(),
        started_at_ms,
        completed_at_ms: Some(completed_at_ms),
        status,
        risk_level: Some(assessment.risk_level),
        user_authorization: Some(assessment.user_authorization),
        rationale: Some(assessment.rationale.clone()),
        decision_source: Some(GuardianAssessmentDecisionSource::Agent),
        action: terminal_action,
    };
    if completed_review
        && let Some((evidence, action, authorization_version, root_authorization_version)) =
            review_evidence
    {
        evidence.record(
            &assessment_event,
            &action,
            authorization_version,
            root_authorization_version,
        );
    }
    session
        .send_event(
            turn.as_ref(),
            EventMsg::GuardianAssessment(assessment_event),
        )
        .await;

    if count_denial_for_circuit_breaker {
        record_guardian_denial(&session, &turn, &assessment_turn_id).await;
    } else {
        record_guardian_non_denial(&session, &assessment_turn_id).await;
    }

    if approved {
        ReviewDecision::Approved
    } else {
        let rationale = if assessment.rationale.trim().is_empty() {
            "Auto-reviewer denied the action without a specific rationale."
        } else {
            assessment.rationale.trim()
        };
        let rejection_instructions = turn
            .model_info
            .model_messages
            .as_ref()
            .and_then(|messages| messages.auto_review.as_ref())
            .and_then(|messages| messages.rejection_instructions.as_deref())
            .unwrap_or(GUARDIAN_REJECTION_INSTRUCTIONS);
        ReviewDecision::denied(format!(
            "This action was rejected due to unacceptable risk.\nReason: {rationale}\n{rejection_instructions}"
        ))
    }
}

pub(crate) struct GuardianReviewOptions {
    pub(crate) plugin_attribution_override: Option<PluginCommandAttribution>,
    pub(crate) approval_request_source: GuardianApprovalRequestSource,
    pub(crate) external_cancel: Option<CancellationToken>,
}

/// Public entrypoint for approval requests that should be reviewed by guardian.
pub(crate) async fn review_approval_request(
    session: &Arc<Session>,
    context: impl Into<GuardianReviewContext>,
    review_id: String,
    request: GuardianApprovalRequest,
    reasons: ApprovalRequestReasons,
) -> ReviewDecision {
    // Box the delegated review future so callers do not inline the entire
    // guardian session state machine into their own async stack.
    Box::pin(run_guardian_review(
        Arc::clone(session),
        context.into(),
        review_id,
        request,
        reasons,
        GuardianReviewOptions {
            plugin_attribution_override: None,
            approval_request_source: GuardianApprovalRequestSource::MainTurn,
            external_cancel: None,
        },
    ))
    .await
}

pub(crate) async fn review_approval_request_with_cancel(
    session: &Arc<Session>,
    context: impl Into<GuardianReviewContext>,
    review_id: String,
    request: GuardianApprovalRequest,
    retry_reason: Option<String>,
    options: GuardianReviewOptions,
) -> ReviewDecision {
    run_guardian_review(
        Arc::clone(session),
        context.into(),
        review_id,
        request,
        ApprovalRequestReasons {
            approval: None,
            retry: retry_reason,
        },
        options,
    )
    .await
}

pub(crate) fn spawn_approval_request_review(
    session: Arc<Session>,
    context: impl Into<GuardianReviewContext>,
    review_id: String,
    request: GuardianApprovalRequest,
    reasons: ApprovalRequestReasons,
    options: GuardianReviewOptions,
) -> oneshot::Receiver<ReviewDecision> {
    let context = context.into();
    let (tx, rx) = oneshot::channel();
    let runtime = session.services.runtime_handle.clone();
    let spawn_result = std::thread::Builder::new()
        .name("codex-approval-review".to_string())
        .spawn(move || {
            let decision = runtime.block_on(run_guardian_review(
                session, context, review_id, request, reasons, options,
            ));
            let _ = tx.send(decision);
        });
    if let Err(err) = spawn_result {
        tracing::error!(%err, "failed to spawn automatic approval review worker");
    }
    rx
}

pub(super) struct GuardianReviewSessionConfig {
    pub(super) spawn_config: crate::config::Config,
    model: String,
    reasoning_effort: Option<codex_protocol::openai_models::ReasoningEffort>,
    default_review_model_id: String,
    catalog_contains_auto_review: bool,
    model_overridden: bool,
    model_override: Option<String>,
}

pub(super) async fn guardian_review_session_config(
    session: &Session,
    turn: &TurnContext,
) -> anyhow::Result<GuardianReviewSessionConfig> {
    let network_proxy = session.services.network_proxy.load_full();
    let live_network_config = match network_proxy.as_ref() {
        Some(network_proxy) => Some(network_proxy.proxy().current_cfg().await?),
        None => None,
    };
    let available_models = session
        .services
        .models_manager
        .list_models(
            codex_models_manager::manager::RefreshStrategy::Offline,
            turn.config.http_client_factory(),
        )
        .await;
    let default_review_model_id = turn.provider.approval_review_preferred_model();
    let preferred_reasoning_effort = |supports_low: bool, fallback| {
        if supports_low {
            Some(codex_protocol::openai_models::ReasoningEffort::Low)
        } else {
            fallback
        }
    };
    let model_override = turn.model_info.auto_review_model_override.as_deref();
    let review_model_id = model_override.unwrap_or(default_review_model_id);
    let review_model = available_models
        .iter()
        .find(|preset| preset.model == review_model_id);
    let guardian_catalog_contains_auto_review = available_models
        .iter()
        .any(|preset| preset.model == default_review_model_id);
    let guardian_review_model_overridden = model_override.is_some();
    let guardian_review_model_override = model_override.map(str::to_string);
    let (guardian_model, guardian_reasoning_effort) = if let Some(preset) = review_model {
        let reasoning_effort = preferred_reasoning_effort(
            preset
                .supported_reasoning_efforts
                .iter()
                .any(|effort| effort.effort == codex_protocol::openai_models::ReasoningEffort::Low),
            Some(preset.default_reasoning_effort.clone()),
        );
        (review_model_id.to_string(), reasoning_effort)
    } else {
        let reasoning_effort = preferred_reasoning_effort(
            turn.model_info
                .supported_reasoning_levels
                .iter()
                .any(|preset| preset.effort == codex_protocol::openai_models::ReasoningEffort::Low),
            turn.reasoning_effort
                .clone()
                .or_else(|| turn.model_info.default_reasoning_level.clone()),
        );
        (
            model_override
                .unwrap_or(turn.model_info.slug.as_str())
                .to_string(),
            reasoning_effort,
        )
    };

    let guardian_model_info = session
        .services
        .models_manager
        .get_model_info(
            guardian_model.as_str(),
            &turn.config.to_models_manager_config(),
        )
        .await;
    let mut spawn_config = build_guardian_review_session_config(
        turn.config.as_ref(),
        live_network_config,
        guardian_model.as_str(),
        guardian_reasoning_effort.clone(),
        guardian_model_info.model_messages.as_ref(),
    )?;
    if turn.model_info.node_repl_auto_review_required {
        spawn_config
            .features
            .enable(Feature::RetainClientDeveloperMessages)
            .map_err(|error| {
                anyhow::anyhow!(
                    "guardian review session could not preserve Node REPL developer policy: {error}"
                )
            })?;
    }
    if guardian_model != turn.model_info.slug {
        spawn_config.model_context_window = None;
        spawn_config.model_auto_compact_token_limit = None;
    }
    Ok(GuardianReviewSessionConfig {
        spawn_config,
        model: guardian_model,
        reasoning_effort: guardian_reasoning_effort,
        default_review_model_id: default_review_model_id.to_string(),
        catalog_contains_auto_review: guardian_catalog_contains_auto_review,
        model_overridden: guardian_review_model_overridden,
        model_override: guardian_review_model_override,
    })
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
async fn run_guardian_review_session_before_deadline(
    session: Arc<Session>,
    context: GuardianReviewContext,
    request: GuardianApprovalRequest,
    reasons: ApprovalRequestReasons,
    schema: serde_json::Value,
    external_cancel: Option<CancellationToken>,
    deadline: Instant,
) -> (GuardianReviewOutcome, GuardianReviewAnalyticsResult) {
    let turn = context.turn();
    let session_config = match guardian_review_session_config(session.as_ref(), turn.as_ref()).await
    {
        Ok(session_config) => session_config,
        Err(err) => {
            return (
                GuardianReviewOutcome::Error(GuardianReviewError::prompt_build(err)),
                GuardianReviewAnalyticsResult::without_session(),
            );
        }
    };
    let (session_outcome, session_analytics_result) = Box::pin(
        session
            .guardian_review_session
            .run_review(GuardianReviewSessionParams {
                parent_session: Arc::clone(&session),
                parent_context: context.clone(),
                spawn_config: session_config.spawn_config,
                request,
                reasons,
                schema,
                model: session_config.model,
                reasoning_effort: session_config.reasoning_effort,
                guardian_default_review_model_id: session_config.default_review_model_id,
                guardian_catalog_contains_auto_review: session_config.catalog_contains_auto_review,
                guardian_review_model_overridden: session_config.model_overridden,
                guardian_review_model_override: session_config.model_override,
                reasoning_summary: turn.reasoning_summary,
                personality: turn.personality,
                external_cancel,
                deadline,
            }),
    )
    .await;

    match session_outcome {
        GuardianReviewSessionOutcome::Completed(Ok(last_agent_message)) => match last_agent_message
        {
            Some(last_agent_message) => {
                match parse_guardian_assessment(Some(&last_agent_message)) {
                    Ok(assessment) => (
                        GuardianReviewOutcome::Completed(assessment),
                        session_analytics_result,
                    ),
                    Err(err) => (
                        GuardianReviewOutcome::Error(GuardianReviewError::parse(err)),
                        session_analytics_result,
                    ),
                }
            }
            None => (
                GuardianReviewOutcome::Error(GuardianReviewError::session(anyhow::anyhow!(
                    "guardian review completed without an assessment payload"
                ))),
                session_analytics_result,
            ),
        },
        GuardianReviewSessionOutcome::Completed(Err(err)) => (
            GuardianReviewOutcome::Error(GuardianReviewError::session(err)),
            session_analytics_result,
        ),
        GuardianReviewSessionOutcome::PromptBuildFailed(err) => (
            GuardianReviewOutcome::Error(GuardianReviewError::prompt_build(err)),
            session_analytics_result,
        ),
        GuardianReviewSessionOutcome::SessionFailed { error, error_info } => {
            let error = match error_info {
                Some(error_info) => GuardianReviewError::session_with_error_info(error, error_info),
                None => GuardianReviewError::session(error),
            };
            (
                GuardianReviewOutcome::Error(error),
                session_analytics_result,
            )
        }
        GuardianReviewSessionOutcome::TimedOut => (
            GuardianReviewOutcome::Error(GuardianReviewError::Timeout),
            session_analytics_result,
        ),
        GuardianReviewSessionOutcome::Aborted => (
            GuardianReviewOutcome::Error(GuardianReviewError::Cancelled),
            session_analytics_result,
        ),
    }
}

pub(super) async fn run_guardian_review_session_with_retry(
    session: Arc<Session>,
    context: impl Into<GuardianReviewContext>,
    request: GuardianApprovalRequest,
    reasons: ApprovalRequestReasons,
    schema: serde_json::Value,
    external_cancel: Option<CancellationToken>,
    max_attempts: i64,
) -> (GuardianReviewOutcome, GuardianReviewAnalyticsResult) {
    let context = context.into();
    assert!(max_attempts > 0, "guardian review must run at least once");
    let deadline = Instant::now() + GUARDIAN_REVIEW_TIMEOUT;
    let mut attempt_count = 1;
    loop {
        let (outcome, mut analytics_result) = run_guardian_review_session_before_deadline(
            Arc::clone(&session),
            context.clone(),
            request.clone(),
            reasons.clone(),
            schema.clone(),
            external_cancel.clone(),
            deadline,
        )
        .await;
        analytics_result.attempt_count = attempt_count;
        if attempt_count >= max_attempts || !should_retry_guardian_review(&outcome) {
            return (outcome, analytics_result);
        }
        if let Some(error) =
            wait_before_guardian_retry(attempt_count, deadline, external_cancel.as_ref()).await
        {
            return (GuardianReviewOutcome::Error(error), analytics_result);
        }
        attempt_count += 1;
    }
}

async fn wait_before_guardian_retry(
    attempt_count: i64,
    deadline: Instant,
    external_cancel: Option<&CancellationToken>,
) -> Option<GuardianReviewError> {
    let retry_delay = backoff(attempt_count as u64);
    let retry_at = (Instant::now() + retry_delay).min(deadline);
    tokio::select! {
        _ = sleep_until(retry_at) => {
            (Instant::now() >= deadline).then_some(GuardianReviewError::Timeout)
        }
        _ = async {
            if let Some(cancel_token) = external_cancel {
                cancel_token.cancelled().await;
            } else {
                std::future::pending::<()>().await;
            }
        } => Some(GuardianReviewError::Cancelled),
    }
}

fn should_retry_guardian_review(outcome: &GuardianReviewOutcome) -> bool {
    matches!(
        outcome,
        GuardianReviewOutcome::Error(
            GuardianReviewError::Session {
                error_info: Some(
                    CodexErrorInfo::ServerOverloaded
                        | CodexErrorInfo::HttpConnectionFailed { .. }
                        | CodexErrorInfo::ResponseStreamConnectionFailed { .. }
                        | CodexErrorInfo::InternalServerError
                        | CodexErrorInfo::ResponseStreamDisconnected { .. }
                ),
                ..
            } | GuardianReviewError::Parse { .. }
        )
    )
}

#[cfg(test)]
mod review_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn guardian_review_error_reason_distinguishes_error_kinds() {
        let parse_error = GuardianReviewError::parse(anyhow::anyhow!("bad guardian JSON"));
        let prompt_error = GuardianReviewError::prompt_build(anyhow::anyhow!("bad prompt/config"));
        let session_error =
            GuardianReviewError::session(anyhow::anyhow!("guardian runtime failed"));
        let structured_session_error = GuardianReviewError::session_with_error_info(
            anyhow::anyhow!("temporary guardian failure"),
            CodexErrorInfo::ServerOverloaded,
        );

        assert!(matches!(
            parse_error.failure_reason(),
            GuardianReviewFailureReason::ParseError
        ));
        assert!(matches!(
            prompt_error.failure_reason(),
            GuardianReviewFailureReason::PromptBuildError
        ));
        assert!(matches!(
            session_error.failure_reason(),
            GuardianReviewFailureReason::SessionError
        ));
        assert!(matches!(
            structured_session_error.failure_reason(),
            GuardianReviewFailureReason::SessionError
        ));
    }

    #[test]
    fn guardian_review_retry_only_retries_transient_session_and_parse_errors() {
        let assessment = GuardianAssessment {
            risk_level: GuardianRiskLevel::High,
            user_authorization: GuardianUserAuthorization::Unknown,
            outcome: GuardianAssessmentOutcome::Deny,
            rationale: "deny".to_string(),
        };
        let transient_error_info = [
            CodexErrorInfo::ServerOverloaded,
            CodexErrorInfo::HttpConnectionFailed {
                http_status_code: Some(502),
            },
            CodexErrorInfo::ResponseStreamConnectionFailed {
                http_status_code: Some(503),
            },
            CodexErrorInfo::InternalServerError,
            CodexErrorInfo::ResponseStreamDisconnected {
                http_status_code: None,
            },
        ];
        let mut outcomes = transient_error_info
            .into_iter()
            .map(|error_info| {
                (
                    GuardianReviewOutcome::Error(GuardianReviewError::session_with_error_info(
                        anyhow::anyhow!("transient session"),
                        error_info,
                    )),
                    true,
                )
            })
            .collect::<Vec<_>>();
        outcomes.extend([
            (GuardianReviewOutcome::Completed(assessment), false),
            (
                GuardianReviewOutcome::Error(GuardianReviewError::prompt_build(anyhow::anyhow!(
                    "prompt"
                ))),
                false,
            ),
            (
                GuardianReviewOutcome::Error(GuardianReviewError::session(anyhow::anyhow!(
                    "session"
                ))),
                false,
            ),
            (
                GuardianReviewOutcome::Error(GuardianReviewError::session_with_error_info(
                    anyhow::anyhow!("bad request"),
                    CodexErrorInfo::BadRequest,
                )),
                false,
            ),
            (
                GuardianReviewOutcome::Error(GuardianReviewError::parse(anyhow::anyhow!("parse"))),
                true,
            ),
            (
                GuardianReviewOutcome::Error(GuardianReviewError::Timeout),
                false,
            ),
            (
                GuardianReviewOutcome::Error(GuardianReviewError::Cancelled),
                false,
            ),
        ]);

        for (outcome, expected) in outcomes {
            assert_eq!(should_retry_guardian_review(&outcome), expected);
        }
    }

    #[tokio::test]
    async fn guardian_review_retry_wait_honors_cancellation() {
        let cancel_token = CancellationToken::new();
        cancel_token.cancel();

        let error = wait_before_guardian_retry(
            /*attempt_count*/ 1,
            Instant::now() + Duration::from_secs(/*secs*/ 1),
            Some(&cancel_token),
        )
        .await;

        assert!(matches!(error, Some(GuardianReviewError::Cancelled)));
    }

    #[tokio::test]
    async fn guardian_review_retry_wait_honors_deadline() {
        let error = wait_before_guardian_retry(
            /*attempt_count*/ 1,
            Instant::now(),
            /*external_cancel*/ None,
        )
        .await;

        assert!(matches!(error, Some(GuardianReviewError::Timeout)));
    }
}
