//! Owns approval routing and the choice between cached evidence and a fresh assessment.
//! Registration does not depend on the async scorer starting successfully.

use super::authorization::ScoreAuthorization;
use super::config::GuardianV2Config;
use super::metrics::TOOL_CALL_LAG_METRIC;
use super::metrics::record_fast_decision;
use super::parent_compaction::select_parent_compaction;
use super::sampler::LunaSampler;
use super::score::GuardianV2ScoreProgress;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::context::GuardianContextMode;
use codex_extension_api::ApprovalDecision;
use codex_extension_api::ApprovalDecisionInput;
use codex_extension_api::ApprovalReviewContributor;
use codex_extension_api::ExtensionFuture;
use codex_protocol::approvals::GuardianReviewReason;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::mcp::is_node_repl_backed_connector;
use codex_protocol::openai_models::GuardianModelPolicy;
use codex_protocol::openai_models::GuardianReviewMode;
use codex_protocol::openai_models::GuardianScope;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::TruncationPolicy;
use std::sync::Weak;

pub(super) struct GuardianApprovalReviewer {
    pub(super) thread_manager: Weak<ThreadManager>,
}

impl ApprovalReviewContributor for GuardianApprovalReviewer {
    fn decide<'a>(
        &'a self,
        input: &'a ApprovalDecisionInput<'_>,
    ) -> ExtensionFuture<'a, Option<ApprovalDecision>> {
        Box::pin(async move {
            // If the scorer is unavailable, the reviewer extension runs its synchronous fallback.
            let manager = self.thread_manager.upgrade()?;
            let Ok(thread) = manager.get_thread(input.thread_id).await else {
                record_fast_decision(input.metrics.as_deref(), "deferred", "scoring_failure");
                return None;
            };
            Some(self.decide_request(&thread, input).await)
        })
    }
}

impl GuardianApprovalReviewer {
    #[tracing::instrument(skip_all, fields(approval_id = input.approval_id))]
    async fn decide_request(
        &self,
        thread: &CodexThread,
        input: &ApprovalDecisionInput<'_>,
    ) -> ApprovalDecision {
        if input.full_access {
            return ApprovalDecision::Allow;
        }
        if !input.require_guardian
            && (input.approvals_reviewer == ApprovalsReviewer::User
                || !matches!(
                    input.approval_policy,
                    AskForApproval::OnRequest | AskForApproval::Granular(_)
                ))
        {
            return ApprovalDecision::AskUser;
        }
        let config = thread.config().await;
        let model = input
            .thread_store
            .get::<codex_protocol::openai_models::ModelInfo>();
        let guardian_config = input
            .thread_store
            .get::<GuardianV2Config>()
            .map(|config| (*config).clone())
            .map_or_else(|| GuardianV2Config::resolve(&config), Ok)
            .ok();
        let mut policy = guardian_config.as_ref().map_or_else(
            || {
                codex_config::GuardianPolicyLoader::new(
                    Some(&codex_features::FeatureToml::Enabled(true)),
                    &codex_config::ConfigRequirements::default(),
                )
                .resolve(model.as_deref())
            },
            |config| config.policy_for_model(model.as_deref()),
        );
        if let Some(model) = model.as_ref() {
            config
                .config_layer_stack
                .requirements()
                .constrain_guardian_policy(&mut policy, &model.slug);
        }
        let mode = policy.review_mode(input.category);
        if mode != GuardianReviewMode::Adaptive {
            record_fast_decision(input.metrics.as_deref(), "deferred", "out_of_scope");
        }
        if mode == GuardianReviewMode::Disabled && !input.require_guardian {
            return ApprovalDecision::AskUser;
        }
        let reason = if mode == GuardianReviewMode::Adaptive && !input.require_fresh_review {
            match guardian_config.as_ref() {
                Some(config) => match cached_evidence(thread, input, config, &policy).await {
                    Ok(()) => return ApprovalDecision::Allow,
                    Err(reason) => reason,
                },
                None => {
                    record_fast_decision(input.metrics.as_deref(), "deferred", "scoring_failure");
                    GuardianReviewReason::ScoringFailure
                }
            }
        } else if input.require_fresh_review {
            GuardianReviewReason::FreshRequired
        } else {
            GuardianReviewReason::Policy
        };
        tracing::debug!(
            decision_source = "synchronous_assessment",
            ?reason,
            "reviewing approval"
        );
        match input.synchronous_reviewer.review(reason).await {
            Some(decision) => ApprovalDecision::Reviewed(decision),
            None => ApprovalDecision::AskUser,
        }
    }
}

async fn cached_evidence(
    thread: &CodexThread,
    input: &ApprovalDecisionInput<'_>,
    config: &GuardianV2Config,
    policy: &GuardianModelPolicy,
) -> Result<(), GuardianReviewReason> {
    let store = input.thread_store;
    let metrics = input.metrics.as_deref();
    let Some(progress) = store.get::<GuardianV2ScoreProgress>() else {
        record_fast_decision(metrics, "deferred", "missing_score");
        return Err(GuardianReviewReason::MissingScore);
    };
    // Elicitations and intercepted execs can expand beyond the original scored action.
    let action = codex_guardian_context::action_for_review(input.action.clone());
    let max_action_bytes = TruncationPolicy::Tokens(config.max_action_tokens).byte_budget();
    let action_fits = serde_json::to_string_pretty(&action)
        .is_ok_and(|action| action.len().saturating_add(1) <= max_action_bytes);
    let history = thread.conversation_history_snapshot().await;
    let cached = progress.inspect(input.tool_call_id);
    if !action_fits || cached.oversized {
        record_fast_decision(metrics, "deferred", "scoring_failure");
        return Err(GuardianReviewReason::ScoringFailure);
    }
    let context_mode = GuardianContextMode::from_history(history.as_ref());
    if context_mode == GuardianContextMode::ThreadOwned {
        let sampler = store
            .get::<LunaSampler>()
            .ok_or(GuardianReviewReason::MissingScore)?;
        if select_parent_compaction(
            context_mode,
            config,
            history.as_ref(),
            &sampler,
            /*legacy_model_hash*/ None,
        )
        .is_err()
        {
            record_fast_decision(metrics, "deferred", "incompatible_compaction");
            return Err(GuardianReviewReason::IncompatibleCompaction);
        }
    }
    let action = input.action;
    if input.category == GuardianScope::ComputerUse
        && policy.allows_initial_cua_call()
        && matches!(
            action.get("tool_name").and_then(serde_json::Value::as_str),
            Some("js" | "browser_js")
        )
        && is_node_repl_backed_connector(
            action
                .get("server")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default(),
            action
                .get("connector_id")
                .and_then(serde_json::Value::as_str),
        )
        && cached.js_executions == 1
    {
        record_fast_decision(metrics, "approved", "initial_cua_call");
        return Ok(());
    }
    let Some(permissions) = input.permissions else {
        record_fast_decision(metrics, "deferred", "permission_resolution_error");
        return Err(GuardianReviewReason::AuthorizationChanged);
    };
    let current = ScoreAuthorization::current(thread, permissions).await;
    // Classification may publish or fail while authorization is collected.
    let cached = progress.inspect(input.tool_call_id);
    if cached.oversized {
        record_fast_decision(metrics, "deferred", "scoring_failure");
        return Err(GuardianReviewReason::ScoringFailure);
    }
    if !current.local.retained_context_complete
        || current
            .root
            .is_some_and(|root| !root.retained_context_complete)
    {
        record_fast_decision(metrics, "deferred", "incomplete_authorization");
        return Err(GuardianReviewReason::Policy);
    }
    let lag = cached.lag;
    if let Some(metrics) = metrics {
        metrics.histogram(
            TOOL_CALL_LAG_METRIC,
            i64::try_from(lag).unwrap_or(i64::MAX),
            &[],
        );
    }
    // Reuse the latest thread score within the lag limit, even across categories
    // and while the current action's async score is still in flight.
    let (reason, label) = match cached.action_risk {
        _ if lag > config.max_tool_call_lag => {
            if let Some(metrics) = metrics {
                metrics.counter(
                    super::metrics::REVIEW_FALLBACK_METRIC,
                    /*inc*/ 1,
                    &[("fallback_reason", "score_lag")],
                );
            }
            (GuardianReviewReason::StaleScore, "stale_score")
        }
        _ if cached.has_unscored_failure => {
            (GuardianReviewReason::ScoringFailure, "scoring_failure")
        }
        None => (GuardianReviewReason::MissingScore, "missing_score"),
        Some(score) if score < config.review_threshold => {
            if cached.authorization.as_ref() != Some(&current) {
                (
                    GuardianReviewReason::AuthorizationChanged,
                    "authorization_changed",
                )
            } else {
                record_fast_decision(metrics, "approved", "low_risk");
                return Ok(());
            }
        }
        Some(score) if score >= config.review_threshold => {
            (GuardianReviewReason::ElevatedRisk, "elevated_risk")
        }
        Some(_) => (GuardianReviewReason::InvalidScore, "invalid_score"),
    };
    tracing::debug!(
        approval_id = input.approval_id,
        fallback_reason = label,
        "requesting synchronous review"
    );
    record_fast_decision(metrics, "deferred", label);
    Err(reason)
}
