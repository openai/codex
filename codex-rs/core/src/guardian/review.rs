use std::sync::Arc;
use std::time::Instant;

use codex_analytics::GuardianApprovalRequestSource;
use codex_analytics::GuardianReviewDecision;
use codex_analytics::GuardianReviewFailureReason;
use codex_analytics::GuardianReviewSessionKind;
use codex_analytics::GuardianReviewTerminalStatus;
use codex_analytics::GuardianReviewedAction;
use codex_analytics::now_unix_seconds;
use codex_features::Feature;
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
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::WarningEvent;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;

use super::GUARDIAN_REVIEW_TIMEOUT;
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
use super::review_session::GuardianReviewSessionMetadata;
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

pub(crate) fn guardian_timeout_message() -> String {
    GUARDIAN_TIMEOUT_INSTRUCTIONS.to_string()
}

#[derive(Debug)]
pub(super) enum GuardianReviewOutcome {
    Completed(
        anyhow::Result<GuardianAssessment>,
        Option<GuardianReviewSessionMetadata>,
    ),
    Failed(GuardianReviewFailure, Option<GuardianReviewSessionMetadata>),
    TimedOut(Option<GuardianReviewSessionMetadata>),
    Aborted(Option<GuardianReviewSessionMetadata>),
}

#[derive(Debug)]
pub(super) enum GuardianReviewFailure {
    PromptBuild(anyhow::Error),
    Session(anyhow::Error),
    Parse(anyhow::Error),
}

impl GuardianReviewFailure {
    fn reason(&self) -> GuardianReviewFailureReason {
        match self {
            Self::PromptBuild(_) => GuardianReviewFailureReason::PromptBuildError,
            Self::Session(_) => GuardianReviewFailureReason::SessionError,
            Self::Parse(_) => GuardianReviewFailureReason::ParseError,
        }
    }

    fn error(&self) -> &anyhow::Error {
        match self {
            Self::PromptBuild(err) | Self::Session(err) | Self::Parse(err) => err,
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

struct GuardianReviewAnalyticsContext {
    thread_id: String,
    turn_id: String,
    review_id: String,
    target_item_id: Option<String>,
    approval_request_source: GuardianApprovalRequestSource,
    reviewed_action: GuardianReviewedAction,
    started_at: u64,
    started_instant: Instant,
}

struct GuardianReviewAnalyticsResult {
    decision: GuardianReviewDecision,
    terminal_status: GuardianReviewTerminalStatus,
    failure_reason: Option<GuardianReviewFailureReason>,
    risk_level: Option<GuardianRiskLevel>,
    user_authorization: Option<GuardianUserAuthorization>,
    outcome: Option<GuardianAssessmentOutcome>,
    guardian_thread_id: Option<String>,
    guardian_session_kind: Option<GuardianReviewSessionKind>,
    guardian_model: Option<String>,
    guardian_reasoning_effort: Option<String>,
    had_prior_review_context: Option<bool>,
    reviewed_action_truncated: bool,
    token_usage: Option<TokenUsage>,
    time_to_first_token_ms: Option<u64>,
    completed_at: u64,
}

impl GuardianReviewAnalyticsResult {
    fn from_session_metadata(
        metadata: Option<GuardianReviewSessionMetadata>,
        completed_at: u64,
    ) -> Self {
        let mut terminal = Self {
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
            terminal.guardian_thread_id = Some(metadata.guardian_thread_id);
            terminal.guardian_session_kind = Some(metadata.guardian_session_kind);
            terminal.guardian_model = Some(metadata.guardian_model);
            terminal.guardian_reasoning_effort = metadata.guardian_reasoning_effort;
            terminal.had_prior_review_context = Some(metadata.had_prior_review_context);
            terminal.reviewed_action_truncated = metadata.reviewed_action_truncated;
            terminal.token_usage = metadata.token_usage;
        }

        terminal
    }
}

impl GuardianReviewAnalyticsContext {
    fn track(
        &self,
        session: &Session,
        turn: &TurnContext,
        terminal: GuardianReviewAnalyticsResult,
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
                reviewed_action_truncated: terminal.reviewed_action_truncated,
                decision: terminal.decision,
                terminal_status: terminal.terminal_status,
                failure_reason: terminal.failure_reason,
                risk_level: terminal.risk_level,
                user_authorization: terminal.user_authorization,
                outcome: terminal.outcome,
                guardian_thread_id: terminal.guardian_thread_id,
                guardian_session_kind: terminal.guardian_session_kind,
                guardian_model: terminal.guardian_model,
                guardian_reasoning_effort: terminal.guardian_reasoning_effort,
                had_prior_review_context: terminal.had_prior_review_context,
                review_timeout_ms: GUARDIAN_REVIEW_TIMEOUT.as_millis() as u64,
                // TODO(rhan-oai): plumb nested Guardian review session tool-call counts.
                tool_call_count: None,
                time_to_first_token_ms: terminal.time_to_first_token_ms,
                completion_latency_ms: Some(completion_latency_ms),
                started_at: self.started_at,
                completed_at: Some(terminal.completed_at),
                input_tokens: terminal
                    .token_usage
                    .as_ref()
                    .map(|usage| usage.input_tokens),
                cached_input_tokens: terminal
                    .token_usage
                    .as_ref()
                    .map(|usage| usage.cached_input_tokens),
                output_tokens: terminal
                    .token_usage
                    .as_ref()
                    .map(|usage| usage.output_tokens),
                reasoning_output_tokens: terminal
                    .token_usage
                    .as_ref()
                    .map(|usage| usage.reasoning_output_tokens),
                total_tokens: terminal
                    .token_usage
                    .as_ref()
                    .map(|usage| usage.total_tokens),
            });
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

/// This function always fails closed: timeouts, review-session failures, and
/// parse failures all block execution, but timeouts are still surfaced to the
/// caller as distinct from explicit guardian denials.
async fn run_guardian_review(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    review_id: String,
    request: GuardianApprovalRequest,
    retry_reason: Option<String>,
    approval_request_source: GuardianApprovalRequestSource,
    external_cancel: Option<CancellationToken>,
) -> ReviewDecision {
    let started_at = now_unix_seconds();
    let started_instant = Instant::now();
    let target_item_id = guardian_request_target_item_id(&request).map(str::to_string);
    let assessment_turn_id = guardian_request_turn_id(&request, &turn.sub_id).to_string();
    let action_summary = guardian_assessment_action(&request);
    let analytics_context = GuardianReviewAnalyticsContext {
        thread_id: session.conversation_id.to_string(),
        turn_id: assessment_turn_id.clone(),
        review_id: review_id.clone(),
        target_item_id: target_item_id.clone(),
        approval_request_source,
        reviewed_action: guardian_reviewed_action(&request),
        started_at,
        started_instant,
    };
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
        analytics_context.track(
            session.as_ref(),
            turn.as_ref(),
            GuardianReviewAnalyticsResult {
                decision: GuardianReviewDecision::Aborted,
                terminal_status: GuardianReviewTerminalStatus::Aborted,
                failure_reason: Some(GuardianReviewFailureReason::Cancelled),
                ..GuardianReviewAnalyticsResult::from_session_metadata(None, now_unix_seconds())
            },
        );
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
        return ReviewDecision::Abort;
    }

    let schema = guardian_output_schema();
    let terminal_action = action_summary.clone();
    let outcome = Box::pin(run_guardian_review_session(
        session.clone(),
        turn.clone(),
        request,
        retry_reason.clone(),
        schema,
        external_cancel,
    ))
    .await;

    let completed_at = now_unix_seconds();
    let terminal =
        |metadata| GuardianReviewAnalyticsResult::from_session_metadata(metadata, completed_at);
    let assessment = match outcome {
        GuardianReviewOutcome::Completed(Ok(assessment), metadata) => {
            let approved = matches!(assessment.outcome, GuardianAssessmentOutcome::Allow);
            analytics_context.track(
                session.as_ref(),
                turn.as_ref(),
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
                    ..terminal(metadata)
                },
            );
            assessment
        }
        GuardianReviewOutcome::Completed(Err(err), metadata) => {
            let rationale = format!("Automatic approval review failed: {err}");
            analytics_context.track(
                session.as_ref(),
                turn.as_ref(),
                GuardianReviewAnalyticsResult {
                    decision: GuardianReviewDecision::Denied,
                    terminal_status: GuardianReviewTerminalStatus::FailedClosed,
                    failure_reason: Some(GuardianReviewFailureReason::SessionError),
                    ..terminal(metadata)
                },
            );
            GuardianAssessment {
                risk_level: GuardianRiskLevel::High,
                user_authorization: GuardianUserAuthorization::Unknown,
                outcome: GuardianAssessmentOutcome::Deny,
                rationale,
            }
        }
        GuardianReviewOutcome::Failed(failure, metadata) => {
            let rationale = format!("Automatic approval review failed: {}", failure.error());
            analytics_context.track(
                session.as_ref(),
                turn.as_ref(),
                GuardianReviewAnalyticsResult {
                    decision: GuardianReviewDecision::Denied,
                    terminal_status: GuardianReviewTerminalStatus::FailedClosed,
                    failure_reason: Some(failure.reason()),
                    ..terminal(metadata)
                },
            );
            GuardianAssessment {
                risk_level: GuardianRiskLevel::High,
                user_authorization: GuardianUserAuthorization::Unknown,
                outcome: GuardianAssessmentOutcome::Deny,
                rationale,
            }
        }
        GuardianReviewOutcome::TimedOut(metadata) => {
            let rationale =
                "Automatic approval review timed out while evaluating the requested approval."
                    .to_string();
            analytics_context.track(
                session.as_ref(),
                turn.as_ref(),
                GuardianReviewAnalyticsResult {
                    decision: GuardianReviewDecision::Denied,
                    terminal_status: GuardianReviewTerminalStatus::TimedOut,
                    failure_reason: Some(GuardianReviewFailureReason::Timeout),
                    ..terminal(metadata)
                },
            );
            session
                .send_event(
                    turn.as_ref(),
                    EventMsg::Warning(WarningEvent {
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
                        turn_id: assessment_turn_id,
                        status: GuardianAssessmentStatus::TimedOut,
                        risk_level: None,
                        user_authorization: None,
                        rationale: Some(rationale),
                        decision_source: Some(GuardianAssessmentDecisionSource::Agent),
                        action: terminal_action,
                    }),
                )
                .await;
            return ReviewDecision::TimedOut;
        }
        GuardianReviewOutcome::Aborted(metadata) => {
            analytics_context.track(
                session.as_ref(),
                turn.as_ref(),
                GuardianReviewAnalyticsResult {
                    decision: GuardianReviewDecision::Aborted,
                    terminal_status: GuardianReviewTerminalStatus::Aborted,
                    failure_reason: Some(GuardianReviewFailureReason::Cancelled),
                    ..terminal(metadata)
                },
            );
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
            return ReviewDecision::Abort;
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
    review_id: String,
    request: GuardianApprovalRequest,
    retry_reason: Option<String>,
) -> ReviewDecision {
    // Box the delegated review future so callers do not inline the entire
    // guardian session state machine into their own async stack.
    Box::pin(run_guardian_review(
        Arc::clone(session),
        Arc::clone(turn),
        review_id,
        request,
        retry_reason,
        GuardianApprovalRequestSource::MainTurn,
        /*external_cancel*/ None,
    ))
    .await
}

pub(crate) async fn review_approval_request_with_cancel(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    review_id: String,
    request: GuardianApprovalRequest,
    retry_reason: Option<String>,
    approval_request_source: GuardianApprovalRequestSource,
    cancel_token: CancellationToken,
) -> ReviewDecision {
    run_guardian_review(
        Arc::clone(session),
        Arc::clone(turn),
        review_id,
        request,
        retry_reason,
        approval_request_source,
        Some(cancel_token),
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
    request: GuardianApprovalRequest,
    retry_reason: Option<String>,
    schema: serde_json::Value,
    external_cancel: Option<CancellationToken>,
) -> GuardianReviewOutcome {
    let live_network_config = match session.services.network_proxy.as_ref() {
        Some(network_proxy) => match network_proxy.proxy().current_cfg().await {
            Ok(config) => Some(config),
            Err(err) => {
                return GuardianReviewOutcome::Failed(
                    GuardianReviewFailure::PromptBuild(err),
                    None,
                );
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
            return GuardianReviewOutcome::Failed(GuardianReviewFailure::PromptBuild(err), None);
        }
    };

    let (session_outcome, session_metadata) = Box::pin(session.guardian_review_session.run_review(
        GuardianReviewSessionParams {
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
        },
    ))
    .await;

    match session_outcome {
        GuardianReviewSessionOutcome::Completed(Ok(last_agent_message)) => match last_agent_message
        {
            Some(last_agent_message) => {
                match parse_guardian_assessment(Some(&last_agent_message)) {
                    Ok(assessment) => {
                        GuardianReviewOutcome::Completed(Ok(assessment), session_metadata)
                    }
                    Err(err) => GuardianReviewOutcome::Failed(
                        GuardianReviewFailure::Parse(err),
                        session_metadata,
                    ),
                }
            }
            None => GuardianReviewOutcome::Failed(
                GuardianReviewFailure::Session(anyhow::anyhow!(
                    "guardian review completed without an assessment payload"
                )),
                session_metadata,
            ),
        },
        GuardianReviewSessionOutcome::Completed(Err(err)) => {
            GuardianReviewOutcome::Failed(GuardianReviewFailure::Session(err), session_metadata)
        }
        GuardianReviewSessionOutcome::PromptBuildFailed(err) => {
            GuardianReviewOutcome::Failed(GuardianReviewFailure::PromptBuild(err), session_metadata)
        }
        GuardianReviewSessionOutcome::TimedOut => GuardianReviewOutcome::TimedOut(session_metadata),
        GuardianReviewSessionOutcome::Aborted => GuardianReviewOutcome::Aborted(session_metadata),
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;

    #[test]
    fn guardian_review_failure_reason_distinguishes_failure_kinds() {
        let parse_failure = GuardianReviewFailure::Parse(anyhow::anyhow!("bad guardian JSON"));
        let prompt_failure =
            GuardianReviewFailure::PromptBuild(anyhow::anyhow!("bad prompt/config"));
        let session_failure =
            GuardianReviewFailure::Session(anyhow::anyhow!("guardian runtime failed"));

        assert!(matches!(
            parse_failure.reason(),
            GuardianReviewFailureReason::ParseError
        ));
        assert!(matches!(
            prompt_failure.reason(),
            GuardianReviewFailureReason::PromptBuildError
        ));
        assert!(matches!(
            session_failure.reason(),
            GuardianReviewFailureReason::SessionError
        ));
    }
}
