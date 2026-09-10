//! Supplies host review preparation, interruption and runtime configuration.
//! Guardian's extension owns the synchronous review loop and pool.

#[path = "review_request.rs"]
mod request;

use crate::context::GuardianContextMode;
use codex_analytics::GuardianApprovalRequestSource;
use codex_analytics::GuardianReviewAnalyticsResult;
use codex_core_plugins::PluginCommandAttribution;
use codex_extension_api::ThreadIdleCause;
use codex_features::Feature;
use codex_guardian_reviewer::GuardianReviewError;
use codex_guardian_reviewer::GuardianReviewOutcome;
#[cfg(test)]
use codex_guardian_reviewer::GuardianReviewSessionLimits;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::WarningEvent;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::context::GuardianNodeReplPolicy;
use crate::context::GuardianReviewEvidence;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::turn_timing::now_unix_timestamp_ms;

use super::ApprovalRequestReasons;
#[cfg(test)]
use super::GUARDIAN_REVIEW_TIMEOUT;
use super::GUARDIAN_REVIEWER_NAME;
use super::GuardianApprovalRequest;
use super::GuardianAssessmentOutcome;
use super::GuardianReviewContext;
use super::approval_request::format_guardian_action_pretty;
use super::approval_request::guardian_assessment_action;
use super::approval_request::guardian_request_target_item_id;
use super::approval_request::guardian_request_turn_id;
use super::approval_request::guardian_reviewed_action;
use super::review_session::GuardianReviewSessionParams;
use super::review_session::build_guardian_review_session_config;
use codex_guardian_reviewer::guardian_output_schema;

const GUARDIAN_PLUGIN_ATTRIBUTION_TIMEOUT: Duration = Duration::from_secs(5);

async fn plugin_attribution_for_guardian_request(
    context: &GuardianReviewContext,
    request: &GuardianApprovalRequest,
) -> Option<PluginCommandAttribution> {
    let turn = context.turn();
    match request {
        GuardianApprovalRequest::ExecCommand {
            environment_id,
            command,
            cwd,
            ..
        } => {
            let turn_environment =
                context
                    .environments()
                    .turn_environments()
                    .find(|environment| {
                        environment.selection.environment_id.as_str() == environment_id
                    })?;
            if turn_environment.environment.is_remote() {
                let file_system = turn_environment.environment.get_filesystem();
                turn.plugin_attribution_for_executor_command(command, cwd, file_system.as_ref())
                    .await
            } else {
                cwd.to_abs_path()
                    .ok()
                    .and_then(|cwd| turn.plugin_attribution_for_command(command, &cwd))
            }
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

pub(crate) fn is_basic_session_source(session_source: &SessionSource) -> bool {
    match session_source {
        SessionSource::SubAgent(SubAgentSource::Other(label)) => label == GUARDIAN_REVIEWER_NAME,
        SessionSource::Internal(InternalSessionSource::Guardian) => true,
        _ => false,
    }
}

pub(super) async fn record_guardian_non_denial(session: &Arc<Session>, turn_id: &str) {
    codex_guardian_reviewer::ReviewDenials::for_thread(&session.services.thread_extension_data)
        .record_non_denial(turn_id)
        .await;
}

async fn record_guardian_denial(session: &Arc<Session>, turn: &Arc<TurnContext>, turn_id: &str) {
    let Some(message) =
        codex_guardian_reviewer::ReviewDenials::for_thread(&session.services.thread_extension_data)
            .record_denial(turn_id, turn.model_info())
            .await
    else {
        return;
    };

    if session.turn_context_for_sub_id(turn_id).await.is_none() {
        return;
    }

    session
        .send_event(
            turn.as_ref(),
            EventMsg::GuardianWarning(WarningEvent { message }),
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

#[derive(Clone)]
pub(crate) struct GuardianReviewOptions {
    /// Requires Guardian rather than a manual approval; cached evidence may still satisfy it.
    pub(crate) require_guardian: bool,
    pub(crate) plugin_attribution_override: Option<PluginCommandAttribution>,
    pub(crate) approval_request_source: GuardianApprovalRequestSource,
    pub(crate) external_cancel: Option<CancellationToken>,
    /// Escalate from extension fast approval to the synchronous Guardian reviewer.
    pub(crate) require_synchronous_review: bool,
}

pub(super) struct GuardianReviewSessionConfig {
    pub(super) spawn_config: crate::config::Config,
    pub(super) node_repl_policy: GuardianNodeReplPolicy,
    pub(super) compaction_model_hash: Option<String>,
    model: String,
    reasoning_effort: Option<codex_protocol::openai_models::ReasoningEffort>,
    default_review_model_id: String,
    catalog_contains_auto_review: bool,
    model_overridden: bool,
    model_override: Option<String>,
}

pub(super) async fn guardian_review_session_config(
    session: &Session,
    context: &GuardianReviewContext,
) -> anyhow::Result<GuardianReviewSessionConfig> {
    let turn = context.turn();
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
    let codex_guardian_reviewer::ReviewModel {
        model: guardian_model,
        reasoning_effort: guardian_reasoning_effort,
        default_review_model_id,
        catalog_contains_auto_review: guardian_catalog_contains_auto_review,
        model_overridden: guardian_review_model_overridden,
        model_override: guardian_review_model_override,
    } = codex_guardian_reviewer::select_review_model(
        &context.model_info,
        context.reasoning_effort.as_ref(),
        default_review_model_id,
        &available_models,
    );

    // Resolve a separate reviewer against the current catalog on every attempt.
    // Parent fallback must retain the action's metadata even after a catalog refresh.
    let guardian_model_info =
        if !guardian_catalog_contains_auto_review && !guardian_review_model_overridden {
            Arc::clone(&context.model_info)
        } else {
            Arc::new(
                session
                    .services
                    .models_manager
                    .get_model_info(
                        guardian_model.as_str(),
                        &turn.config.to_models_manager_config(),
                    )
                    .await,
            )
        };
    let mut spawn_config = build_guardian_review_session_config(
        turn.config.as_ref(),
        live_network_config,
        guardian_model.as_str(),
        guardian_reasoning_effort.clone(),
        context.reasoning_summary,
        context.personality,
        guardian_model_info.model_messages.as_ref(),
    )?;
    if context.model_info.computer_use_review_required() {
        spawn_config
            .features
            .enable(Feature::RetainClientDeveloperMessages)
            .map_err(|error| {
                anyhow::anyhow!(
                    "guardian review session could not preserve REPL developer policy: {error}"
                )
            })?;
    }
    if guardian_model != context.model_info.slug {
        spawn_config.model_context_window = None;
        spawn_config.model_auto_compact_token_limit = None;
    }
    Ok(GuardianReviewSessionConfig {
        spawn_config,
        compaction_model_hash: guardian_model_info.comp_hash.clone(),
        node_repl_policy: GuardianNodeReplPolicy::from_model_messages(
            guardian_model_info.model_messages.as_ref(),
        ),
        model: guardian_model,
        reasoning_effort: guardian_reasoning_effort,
        default_review_model_id,
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
    let session_config = match guardian_review_session_config(session.as_ref(), &context).await {
        Ok(session_config) => session_config,
        Err(err) => {
            return (
                GuardianReviewOutcome::Error(GuardianReviewError::prompt_build(err)),
                GuardianReviewAnalyticsResult::without_session(),
            );
        }
    };
    let (session_outcome, session_analytics_result) =
        Box::pin(super::review_session::run_guardian_review_session(
            session.guardian_review_session(),
            GuardianReviewSessionParams {
                parent_session: Arc::clone(&session),
                parent_context: context.clone(),
                parent_history: session.clone_history().await,
                spawn_config: session_config.spawn_config,
                node_repl_policy: session_config.node_repl_policy,
                request,
                reasons,
                schema,
                model: session_config.model,
                compaction_model_hash: session_config.compaction_model_hash,
                reasoning_effort: session_config.reasoning_effort,
                guardian_default_review_model_id: session_config.default_review_model_id,
                guardian_catalog_contains_auto_review: session_config.catalog_contains_auto_review,
                guardian_review_model_overridden: session_config.model_overridden,
                guardian_review_model_override: session_config.model_override,
                reasoning_summary: context.reasoning_summary,
                personality: context.personality,
                external_cancel,
                deadline,
            },
        ))
        .await;

    (session_outcome.into(), session_analytics_result)
}

#[cfg(test)]
pub(super) async fn run_guardian_review_session_with_retry(
    session: Arc<Session>,
    context: impl Into<GuardianReviewContext>,
    request: GuardianApprovalRequest,
    reasons: ApprovalRequestReasons,
    schema: serde_json::Value,
    external_cancel: Option<CancellationToken>,
    max_attempts: i64,
) -> (GuardianReviewOutcome, GuardianReviewAnalyticsResult) {
    run_guardian_review_session_with_retry_before_deadline(
        session,
        context,
        request,
        reasons,
        schema,
        external_cancel,
        GuardianReviewSessionLimits {
            max_attempts,
            deadline: Instant::now() + GUARDIAN_REVIEW_TIMEOUT,
        },
    )
    .await
}

#[cfg(test)]
async fn run_guardian_review_session_with_retry_before_deadline(
    session: Arc<Session>,
    context: impl Into<GuardianReviewContext>,
    request: GuardianApprovalRequest,
    reasons: ApprovalRequestReasons,
    schema: serde_json::Value,
    external_cancel: Option<CancellationToken>,
    limits: GuardianReviewSessionLimits,
) -> (GuardianReviewOutcome, GuardianReviewAnalyticsResult) {
    let context = context.into();
    codex_guardian_reviewer::run_with_retry(limits, external_cancel.as_ref(), |deadline| {
        run_guardian_review_session_before_deadline(
            Arc::clone(&session),
            context.clone(),
            request.clone(),
            reasons.clone(),
            schema.clone(),
            external_cancel.clone(),
            deadline,
        )
    })
    .await
}
