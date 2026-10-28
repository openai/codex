//! Supplies host review preparation and context-dependent configuration.
//! Guardian's extension owns routing, execution, reporting and denial accounting.

#[path = "review_request.rs"]
mod request;
pub(super) use request::ReviewAuthorization;

use codex_analytics::GuardianApprovalRequestSource;
use codex_analytics::GuardianReviewAnalyticsResult;
use codex_core_plugins::PluginCommandAttribution;
use codex_features::Feature;
use codex_guardian_reviewer::GuardianAssessment;
use codex_guardian_reviewer::GuardianReviewError;
use codex_guardian_reviewer::GuardianReviewOutcome;
#[cfg(test)]
use codex_guardian_reviewer::GuardianReviewSessionLimits;
use codex_guardian_reviewer::ReviewDeadline;
use codex_guardian_reviewer::ReviewModel;
use codex_guardian_reviewer::run_before_review_deadline;
#[cfg(test)]
use codex_guardian_reviewer::run_with_retry;
use codex_prompts::ResolvedModelMessages;
use codex_protocol::openai_models::GuardianScope;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ReviewDecision;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::context::GuardianNodeReplPolicy;
use crate::context::GuardianReviewEvidence;
use crate::context_manager::ContextManager;
use crate::session::session::Session;

use super::ApprovalRequestReasons;
#[cfg(test)]
use super::GUARDIAN_REVIEW_TIMEOUT;
use super::GuardianApprovalRequest;
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
        _ => None,
    }
}

pub(crate) fn new_guardian_review_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(crate) use codex_guardian_reviewer::routes_approval_policy_to_guardian;

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
    review_model: ReviewModel,
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
    let (review_model, guardian_model_info) =
        super::reviewer_config::resolve_review_model(session, context).await;
    let reviewer_config = session
        .services
        .thread_extension_data
        .get::<codex_guardian_reviewer::ReviewerConfig<crate::config::Config>>()
        .ok_or_else(|| anyhow::anyhow!("Guardian reviewer configuration is not installed"))?;
    let model_messages = ResolvedModelMessages::from_model(&guardian_model_info);
    let mut spawn_config = build_guardian_review_session_config(
        (reviewer_config.0)(turn.config.as_ref())?,
        live_network_config,
        review_model.model.as_str(),
        review_model.reasoning_effort.clone(),
        context.reasoning_summary,
        context.personality,
        model_messages,
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
    if review_model.model != context.model_info.slug {
        spawn_config.model_context_window = None;
        spawn_config.model_auto_compact_token_limit = None;
    }
    Ok(GuardianReviewSessionConfig {
        spawn_config,
        node_repl_policy: GuardianNodeReplPolicy::from_messages(model_messages),
        review_model,
    })
}

struct ReviewAttemptOptions {
    authorization: Option<ReviewAuthorization>,
    external_cancel: Option<CancellationToken>,
    deadline: ReviewDeadline,
}

/// A prepared action and the parent transcript to use as evidence for it.
pub(super) struct ReviewInput {
    pub(super) session: Arc<Session>,
    pub(super) context: GuardianReviewContext,
    pub(super) history: ContextManager,
    pub(super) request: GuardianApprovalRequest,
    pub(super) reasons: ApprovalRequestReasons,
}

impl ReviewInput {
    fn into_params(
        self,
        config: GuardianReviewSessionConfig,
        category: GuardianScope,
        options: ReviewAttemptOptions,
    ) -> GuardianReviewSessionParams {
        GuardianReviewSessionParams {
            parent_session: self.session,
            parent_history: self.history,
            reasoning_summary: self.context.reasoning_summary,
            personality: self.context.personality,
            parent_context: self.context,
            spawn_config: config.spawn_config,
            node_repl_policy: config.node_repl_policy,
            request: self.request,
            category,
            reasons: self.reasons,
            authorization: options.authorization,
            schema: guardian_output_schema(),
            review_model: config.review_model,
            external_cancel: options.external_cancel,
            deadline: options.deadline,
        }
    }

    /// Assess the supplied request in a fresh reviewer without publishing a live approval.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "consumed by the recorded-call stack layer")
    )]
    pub(super) async fn assess_isolated(
        self,
        deadline: ReviewDeadline,
        cancellation: &CancellationToken,
    ) -> Result<GuardianAssessment, GuardianReviewError> {
        let review = async {
            let pool = self.session.guardian_review_session().ok_or_else(|| {
                GuardianReviewError::session(anyhow::anyhow!("Guardian reviewer is unavailable"))
            })?;
            let config = guardian_review_session_config(&self.session, &self.context)
                .await
                .map_err(GuardianReviewError::prompt_build)?;
            let category = self.request.guardian_scope();
            let params = self.into_params(
                config,
                category,
                ReviewAttemptOptions {
                    authorization: None,
                    external_cancel: Some(cancellation.clone()),
                    deadline,
                },
            );
            let prepared = super::review_session::prepare_review(params)
                .await
                .map_err(GuardianReviewError::prompt_build)?;
            Ok::<_, GuardianReviewError>(pool.review_isolated(&prepared).await.0)
        };
        let outcome = match run_before_review_deadline(
            deadline,
            Some(cancellation),
            Box::pin(review),
        )
        .await
        {
            Ok(result) => result?,
            Err(interruption) => interruption,
        };
        outcome.into_assessment()
    }
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
    category: GuardianScope,
    reasons: ApprovalRequestReasons,
    options: ReviewAttemptOptions,
) -> (GuardianReviewOutcome, GuardianReviewAnalyticsResult) {
    let Some(pool) = session.guardian_review_session() else {
        return (
            GuardianReviewOutcome::Error(GuardianReviewError::prompt_build(anyhow::anyhow!(
                "Guardian extension is not installed for this thread"
            ))),
            GuardianReviewAnalyticsResult::without_session(),
        );
    };
    let session_config = match run_before_review_deadline(
        options.deadline,
        options.external_cancel.as_ref(),
        guardian_review_session_config(session.as_ref(), &context),
    )
    .await
    {
        Ok(Ok(session_config)) => session_config,
        Ok(Err(err)) => {
            return (
                GuardianReviewOutcome::Error(GuardianReviewError::prompt_build(err)),
                GuardianReviewAnalyticsResult::without_session(),
            );
        }
        Err(outcome) => {
            return (
                outcome.into(),
                GuardianReviewAnalyticsResult::without_session(),
            );
        }
    };
    let input = ReviewInput {
        history: session.clone_history().await,
        session,
        context,
        request,
        reasons,
    };
    let params = input.into_params(session_config, category, options);
    let (session_outcome, session_analytics_result) = Box::pin(
        super::review_session::run_guardian_review_session(pool, params),
    )
    .await;

    (session_outcome.into(), session_analytics_result)
}

#[cfg(test)]
pub(super) async fn run_guardian_review_session_with_retry(
    session: Arc<Session>,
    context: impl Into<GuardianReviewContext>,
    request: GuardianApprovalRequest,
    reasons: ApprovalRequestReasons,
    external_cancel: Option<CancellationToken>,
    max_attempts: i64,
) -> (GuardianReviewOutcome, GuardianReviewAnalyticsResult) {
    run_guardian_review_session_with_retry_before_deadline(
        session,
        context,
        request,
        reasons,
        external_cancel,
        GuardianReviewSessionLimits {
            max_attempts,
            deadline: ReviewDeadline::from_timeout(Some(GUARDIAN_REVIEW_TIMEOUT)),
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
    external_cancel: Option<CancellationToken>,
    limits: GuardianReviewSessionLimits,
) -> (GuardianReviewOutcome, GuardianReviewAnalyticsResult) {
    let context = context.into();
    let (outcome, analytics, _) = run_with_retry(
        limits,
        external_cancel.as_ref(),
        /*telemetry*/ None,
        |deadline| {
            let attempt = run_guardian_review_session_before_deadline(
                Arc::clone(&session),
                context.clone(),
                request.clone(),
                request.guardian_scope(),
                reasons.clone(),
                ReviewAttemptOptions {
                    authorization: None,
                    external_cancel: external_cancel.clone(),
                    deadline,
                },
            );
            async move {
                let (outcome, analytics) = attempt.await;
                (outcome, analytics, None::<()>)
            }
        },
    )
    .await;
    (outcome, analytics)
}
