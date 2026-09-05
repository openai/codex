use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Instant;
use std::time::SystemTime;

use codex_analytics::AnalyticsEventsClient;
use codex_analytics::GuardianV2Event;
use codex_analytics::GuardianV2EventKind;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::context::GuardianReviewEvidence;
use codex_core::context::NodeReplReviewEvidence;
use codex_extension_api::ApprovalReviewContributor;
use codex_extension_api::ConversationHistorySnapshot;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionMetrics;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ExtensionWarning;
use codex_extension_api::GuardianV2Enabled;
use codex_extension_api::ResponseItem;
use codex_extension_api::SkillInvocationContributor;
use codex_extension_api::SkillInvocationInput;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadOriginator;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::ToolStartInput;
use codex_features::Feature;
use codex_guardian_context::ContextTarget;
use codex_history::RolloutItem;
use codex_login::AgentIdentityAuthPolicy;
use codex_login::AuthManager;
use codex_model_provider::create_model_provider;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::mcp::is_node_repl_backed_server;
use codex_protocol::openai_models::GuardianReviewMode;
use codex_protocol::openai_models::GuardianScope;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::TruncationPolicy;
use codex_protocol::protocol::has_full_access;
use codex_protocol::security_risk::SecurityRiskScore;

use super::action::GuardianAction;
use super::action::RenderedAction;
use super::authorization::ScoreAuthorization;
use super::config::GuardianV2Config;
use super::coverage::GuardianPolicy;
use super::coverage::UnscoredAction;
use super::metrics::REVIEW_FALLBACK_METRIC;
use super::metrics::TOOL_CALL_LAG_METRIC;
use super::metrics::record_classification;
use super::metrics::record_classification_risk;
use super::metrics::record_fast_decision;
use super::review_evidence::render_review_evidence;
use super::sampler::LunaSampler;
use super::sampler::LunaSamplerConfig;
use super::sampler::LunaSamplerError;
use super::sampler::LunaSamplingRequest;
use super::sampler::MODEL;
use super::truncation::ClassificationTruncations;
use super::trusted_skills::TrustedSkillInvocations;
use super::trusted_skills::TrustedSkillRoots;
use super::trusted_tools::trusted_tool_context;

/// Explains why Guardian v2 requires synchronous approval review.
#[derive(Debug, Eq, PartialEq)]
pub enum StrictReviewReason {
    ElevatedRisk,
    StaleScore,
    IncompatibleCompaction,
}

enum ClassificationOutcome {
    Scored,
    Superseded,
}

#[derive(Default)]
pub(super) struct GuardianV2ScoreProgress {
    pub(super) latest_tool_call: AtomicUsize,
    // Setup and reset calls must not consume the first JS execution allowance.
    pub(super) js_executions: AtomicUsize,
    pub(super) latest_scored_tool_call: AtomicUsize,
    pub(super) latest_failed_tool_call: AtomicUsize,
    // Serialize successful score publication with its authorization metadata.
    pub(super) authorization: Mutex<Option<ScoreAuthorization>>,
    metrics: Option<Arc<dyn ExtensionMetrics>>,
}

#[derive(Clone)]
struct GuardianV2Extension {
    auth_manager: Arc<AuthManager>,
    event_sink: Arc<dyn ExtensionEventSink>,
    thread_manager: Weak<ThreadManager>,
}

impl ThreadLifecycleContributor<Config> for GuardianV2Extension {
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if !input.config.features.enabled(Feature::GuardianApproval) {
                return;
            }

            let model = input.thread_store.get::<ModelInfo>();
            let thread_id = input.thread_store.level_id().to_string();
            let guardian_config = match GuardianV2Config::resolve(input.config) {
                Ok(config) => config,
                Err(error) => {
                    self.event_sink.emit_warning(ExtensionWarning {
                        thread_id,
                        turn_id: None,
                        message: error,
                    });
                    return;
                }
            };
            let mut policy = guardian_config.policy_for_model(model.as_deref());
            if model.as_ref().is_some_and(|model| {
                input
                    .config
                    .config_layer_stack
                    .requirements()
                    .auto_review_required_for_model(&model.slug)
            }) {
                policy.enforce_required_model();
            }
            let scoring_enabled = policy.scoring_enabled();
            let luna_compaction_hash = if let Some(thread_manager) = self.thread_manager.upgrade() {
                thread_manager
                    .get_models_manager()
                    .get_model_info(MODEL, &input.config.to_models_manager_config())
                    .await
                    .comp_hash
            } else {
                None
            };
            let sampler_config = LunaSamplerConfig {
                provider: create_model_provider(
                    input.config.model_provider.clone(),
                    Some(Arc::clone(&self.auth_manager)),
                ),
                http_client_factory: input.config.http_client_factory(),
                agent_identity_policy: if input.config.features.enabled(Feature::UseAgentIdentity) {
                    AgentIdentityAuthPolicy::ChatGptAuth
                } else {
                    AgentIdentityAuthPolicy::JwtOnly
                },
                session_source: input.session_source.clone(),
                session_id: input.session_store.level_id().to_string(),
                thread_id: thread_id.clone(),
                originator: input
                    .thread_store
                    .get::<ThreadOriginator>()
                    .map(|originator| originator.0.clone()),
                free_guardian: input.config.free_guardian_enabled(),
                service_tier: input.config.service_tier.clone(),
                luna_compaction_hash,
                metrics: input.extension_metrics.clone(),
            };

            if scoring_enabled && guardian_config.transcript.include_images {
                input
                    .thread_store
                    .get_or_init(NodeReplReviewEvidence::default)
                    .enable_image_capture();
            }
            input.thread_store.remove::<LunaSampler>();
            let sampler = input
                .thread_store
                .get_or_init(|| LunaSampler::new(sampler_config));
            input.thread_store.insert(guardian_config);
            input.thread_store.insert(GuardianV2ScoreProgress {
                metrics: input.extension_metrics.clone(),
                ..Default::default()
            });
            // Preserve the answer path selected by the host for this thread.
            input
                .thread_store
                .get_or_init(GuardianReviewEvidence::default);
            input
                .thread_store
                .insert(TrustedSkillRoots::from_config(input.config));
            if scoring_enabled {
                input.thread_store.insert(GuardianV2Enabled);
            }

            // Keep the sampler available for later automatic review, but do not
            // prewarm while User approval mode or Full Access is selected.
            if scoring_enabled
                && input.config.approvals_reviewer == ApprovalsReviewer::AutoReview
                && !has_full_access(
                    input.config.permissions.approval_policy.value(),
                    &input.config.permissions.effective_permission_profile(),
                    input
                        .environments
                        .iter()
                        .map(|environment| &environment.config),
                )
            {
                tokio::spawn(async move {
                    sampler.prewarm().await;
                });
            }
        })
    }
}

impl SkillInvocationContributor for GuardianV2Extension {
    fn requires_host_skill_discovery(&self) -> bool {
        false
    }

    fn on_skill_invocation<'a>(
        &'a self,
        input: SkillInvocationInput<'a>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let Some(roots) = input.thread_store.get::<TrustedSkillRoots>() else {
                return;
            };
            let Some(skill_path) = roots.trusted_skill_path(input.skill_resource) else {
                return;
            };
            let Some(evidence) = input.thread_store.get::<GuardianReviewEvidence>() else {
                return;
            };
            evidence.record_trusted_skill(input.turn_id, skill_path);
        })
    }
}

impl ApprovalReviewContributor for GuardianV2Extension {
    fn fast_decision<'a>(
        &'a self,
        _session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
        prompt: &'a str,
        extension_metrics: Option<Arc<dyn ExtensionMetrics>>,
    ) -> ExtensionFuture<'a, Option<ReviewDecision>> {
        Box::pin(async move {
            let model = thread_store.get::<ModelInfo>();
            let guardian_config = thread_store.get::<GuardianV2Config>()?;
            let policy = guardian_config.policy_for_model(model.as_deref());
            let action = serde_json::from_str::<serde_json::Value>(prompt).unwrap_or_default();
            let scope = GuardianPolicy::review_scope(&action);
            if scope.map_or(policy.other_tools, |scope| policy.mode(scope))
                != GuardianReviewMode::Adaptive
            {
                record_fast_decision(extension_metrics.as_deref(), "deferred", "out_of_scope");
                return None;
            }
            let guardian_evidence = thread_store.get_or_init(GuardianReviewEvidence::default);
            let thread_context_enabled = guardian_evidence.uses_thread_owned_context();
            let mut initial_cua_call = false;
            if scope == Some(GuardianScope::ComputerUse) && policy.initial_cua_call {
                // Thread-owned context checks checkpoint compatibility before the first CUA allowance.
                initial_cua_call = action.get("tool_name").and_then(serde_json::Value::as_str)
                    == Some("js")
                    && action
                        .get("connector_id")
                        .and_then(serde_json::Value::as_str)
                        == Some("node_repl")
                    && thread_store
                        .get::<GuardianV2ScoreProgress>()?
                        .js_executions
                        .load(Ordering::Acquire)
                        == 1;
                if initial_cua_call && !thread_context_enabled {
                    record_fast_decision(
                        extension_metrics.as_deref(),
                        "approved",
                        "initial_cua_call",
                    );
                    return Some(ReviewDecision::Approved);
                }
            } else if thread_store.get::<ModelInfo>().is_some() {
                let manager = self.thread_manager.upgrade()?;
                let thread_id = ThreadId::from_string(thread_store.level_id()).ok()?;
                let Ok(thread) = manager.get_thread(thread_id).await else {
                    record_fast_decision(
                        extension_metrics.as_deref(),
                        "deferred",
                        "scoring_failure",
                    );
                    return None;
                };
                let config = thread.config().await;
                let model = thread_store.get::<ModelInfo>()?;
                if config
                    .config_layer_stack
                    .requirements()
                    .auto_review_required_for_model(&model.slug)
                {
                    record_fast_decision(
                        extension_metrics.as_deref(),
                        "deferred",
                        "required_model",
                    );
                    return None;
                }
            }
            let Some(score_progress) = thread_store.get::<GuardianV2ScoreProgress>() else {
                record_fast_decision(extension_metrics.as_deref(), "deferred", "missing_score");
                return None;
            };
            let manager = self.thread_manager.upgrade()?;
            let thread_id = ThreadId::from_string(thread_store.level_id()).ok()?;
            let Ok(thread) = manager.get_thread(thread_id).await else {
                record_fast_decision(extension_metrics.as_deref(), "deferred", "scoring_failure");
                return None;
            };
            let history = thread.conversation_history_snapshot().await;
            if thread_context_enabled {
                let sampler = thread_store.get::<LunaSampler>()?;
                if requires_sync_for_compaction(&guardian_config, history.as_ref(), &sampler)
                    || encrypted_parent_compaction(
                        history.items(),
                        guardian_config.max_parent_compaction_tokens,
                    )
                    .is_err()
                {
                    thread_store.insert(StrictReviewReason::IncompatibleCompaction);
                    record_fast_decision(
                        extension_metrics.as_deref(),
                        "deferred",
                        "incompatible_compaction",
                    );
                    return None;
                }
            }
            if initial_cua_call {
                record_fast_decision(extension_metrics.as_deref(), "approved", "initial_cua_call");
                return Some(ReviewDecision::Approved);
            }
            let current_authorization = ScoreAuthorization::current(&thread).await;
            if !current_authorization.local.retained_context_complete
                || current_authorization
                    .root
                    .is_some_and(|root| !root.retained_context_complete)
            {
                record_fast_decision(
                    extension_metrics.as_deref(),
                    "deferred",
                    "incomplete_authorization",
                );
                return None;
            }
            let scored_authorization = score_progress
                .authorization
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let latest_scored_tool_call = score_progress
                .latest_scored_tool_call
                .load(Ordering::Acquire);
            let tool_call_lag = score_progress
                .latest_tool_call
                .load(Ordering::Acquire)
                .saturating_sub(latest_scored_tool_call);
            if let Some(metrics) = &extension_metrics {
                metrics.histogram(
                    TOOL_CALL_LAG_METRIC,
                    i64::try_from(tool_call_lag).unwrap_or(i64::MAX),
                    &[],
                );
            }
            if tool_call_lag > guardian_config.max_tool_call_lag {
                thread_store.insert(StrictReviewReason::StaleScore);
                if let Some(metrics) = &extension_metrics {
                    metrics.counter(
                        REVIEW_FALLBACK_METRIC,
                        /*inc*/ 1,
                        &[("fallback_reason", "score_lag")],
                    );
                }
                record_fast_decision(extension_metrics.as_deref(), "deferred", "stale_score");
                return None;
            }
            if score_progress
                .latest_failed_tool_call
                .load(Ordering::Acquire)
                > latest_scored_tool_call
            {
                thread_store.insert(StrictReviewReason::ElevatedRisk);
                record_fast_decision(extension_metrics.as_deref(), "deferred", "scoring_failure");
                return None;
            }

            let Some(score) = thread_store
                .get::<SecurityRiskScore>()
                .and_then(|score| score.scores.get("action_risk").copied())
            else {
                record_fast_decision(extension_metrics.as_deref(), "deferred", "missing_score");
                return None;
            };
            if score < guardian_config.review_threshold {
                if scored_authorization.as_ref() != Some(&current_authorization) {
                    thread_store.insert(StrictReviewReason::StaleScore);
                    record_fast_decision(
                        extension_metrics.as_deref(),
                        "deferred",
                        "authorization_changed",
                    );
                    return None;
                }
                record_fast_decision(extension_metrics.as_deref(), "approved", "low_risk");
                return Some(ReviewDecision::Approved);
            }
            if score >= guardian_config.review_threshold {
                thread_store.insert(StrictReviewReason::ElevatedRisk);
                record_fast_decision(extension_metrics.as_deref(), "deferred", "elevated_risk");
            } else {
                record_fast_decision(extension_metrics.as_deref(), "deferred", "invalid_score");
            }
            None
        })
    }
}

impl ToolLifecycleContributor for GuardianV2Extension {
    fn on_tool_start<'a>(&'a self, input: ToolStartInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(self.score_tool(input))
    }
}

impl GuardianV2Extension {
    fn record_fail_closed_score(thread_store: &ExtensionData, sampled_at: SystemTime) {
        let score = SecurityRiskScore {
            scores: BTreeMap::from([("action_risk".to_owned(), 1.0)]),
            call_id: None,
            action: None,
            sampled_at: Some(sampled_at.into()),
        };
        thread_store.insert_if(score.clone(), |previous| {
            previous.is_none_or(|previous| previous.sampled_at <= score.sampled_at)
        });
    }

    async fn score_tool(&self, input: ToolStartInput<'_>) {
        // Polling a code cell does not introduce another action or age its score.
        if input.tool_name.is_default_namespace() && input.tool_name.name == "wait" {
            return;
        }
        let classification_started_at = Instant::now();
        let Some(sampler) = input.thread_store.get::<LunaSampler>() else {
            return;
        };
        let Some(guardian_config) = input.thread_store.get::<GuardianV2Config>() else {
            return;
        };
        let Some(score_progress) = input.thread_store.get::<GuardianV2ScoreProgress>() else {
            return;
        };
        let parent_model = input.thread_store.get::<ModelInfo>();
        let policy = guardian_config.policy_for_model(parent_model.as_deref());
        if !policy.scoring_enabled() {
            input.thread_store.remove::<GuardianV2Enabled>();
        }
        let mcp_server = input
            .mcp_tool
            .map(|tool| tool.tool_info().server_name.as_str());
        let scope = mcp_server
            .map(GuardianScope::for_mcp_server)
            .or_else(|| GuardianScope::for_tool(input.tool_name));
        if !policy.scores_tool(input.tool_name, input.payload, scope) {
            match policy.unscored_action {
                UnscoredAction::Ignore => {}
                UnscoredAction::AgeScore => {
                    score_progress
                        .latest_tool_call
                        .fetch_add(/*val*/ 1, Ordering::Relaxed);
                }
                UnscoredAction::InvalidateScore => {
                    let index = score_progress
                        .latest_tool_call
                        .fetch_add(/*val*/ 1, Ordering::Relaxed)
                        .saturating_add(/*rhs*/ 1);
                    score_progress
                        .latest_failed_tool_call
                        .fetch_max(index, Ordering::Release);
                }
            }
            return;
        }
        if input.mcp_tool.is_some_and(|tool| {
            let info = tool.tool_info();
            is_node_repl_backed_server(&info.server_name) && info.tool.name == "js"
        }) {
            score_progress
                .js_executions
                .fetch_add(/*val*/ 1, Ordering::Relaxed);
        }
        let metrics = score_progress.metrics.clone();
        let analytics = input.session_store.get::<AnalyticsEventsClient>();
        let sampled_at = SystemTime::now();
        let tool_call_index = score_progress
            .latest_tool_call
            .fetch_add(/*val*/ 1, Ordering::Relaxed)
            .saturating_add(/*rhs*/ 1);
        let event_sink = Arc::clone(&self.event_sink);
        let thread_id = input.thread_store.level_id().to_owned();
        let turn_id = input.turn_id.to_owned();
        let root_turn_id = input.root_turn_id.map(str::to_owned);
        let parent_response_id = input
            .turn_store
            .get::<codex_api::ResponseId>()
            .map(|id| id.0.clone());
        let thread_context: Result<_, String> = async {
            let parsed_thread_id =
                ThreadId::from_string(&thread_id).map_err(|error| error.to_string())?;
            let manager = self
                .thread_manager
                .upgrade()
                .ok_or_else(|| "thread manager is unavailable".to_string())?;
            let thread = manager
                .get_thread(parsed_thread_id)
                .await
                .map_err(|error| error.to_string())?;
            let config = thread.config().await;
            Ok((manager, thread, config))
        }
        .await;
        let (manager, thread, config) = match thread_context {
            Ok(context) => context,
            Err(error) => {
                score_progress
                    .latest_failed_tool_call
                    .fetch_max(tool_call_index, Ordering::Release);
                record_classification(
                    metrics.as_deref(),
                    classification_started_at.elapsed(),
                    "failure",
                );
                event_sink.emit_warning(ExtensionWarning {
                    thread_id,
                    turn_id: Some(turn_id),
                    message: format!("Guardian V2 risk scoring failed: {error}"),
                });
                return;
            }
        };
        // Use the live reviewer, not the startup config or per-app reviewer overrides.
        let snapshot = thread.config_snapshot().await;
        if snapshot.full_access
            || thread.approvals_reviewer_for_turn(input.turn_id).await == ApprovalsReviewer::User
        {
            // A skipped call invalidates older scores, including ones still in flight.
            score_progress
                .latest_failed_tool_call
                .fetch_max(tool_call_index, Ordering::Release);
            return;
        }
        // A required model keeps synchronous review outside its CUA allowance.
        if !(scope == Some(GuardianScope::ComputerUse) && policy.initial_cua_call)
            && parent_model.as_ref().is_some_and(|model| {
                config
                    .config_layer_stack
                    .requirements()
                    .auto_review_required_for_model(&model.slug)
            })
        {
            input.thread_store.remove::<SecurityRiskScore>();
            return;
        }
        input.thread_store.insert(GuardianV2Enabled);
        let model_defaults = parent_model
            .as_ref()
            .and_then(|model| model.model_messages.as_ref())
            .and_then(|messages| messages.guardian_v2.as_ref());
        let guardian_config = match guardian_config.with_model_defaults(model_defaults) {
            Ok(config) => config,
            Err(error) => {
                Self::record_fail_closed_score(input.thread_store, sampled_at);
                record_classification(
                    metrics.as_deref(),
                    classification_started_at.elapsed(),
                    "failure",
                );
                self.event_sink.emit_warning(ExtensionWarning {
                    thread_id: input.thread_store.level_id().to_owned(),
                    turn_id: Some(input.turn_id.to_owned()),
                    message: error,
                });
                return;
            }
        };
        if guardian_config.transcript.include_images {
            input
                .thread_store
                .get_or_init(NodeReplReviewEvidence::default)
                .enable_image_capture();
        }
        input.thread_store.insert(guardian_config.clone());
        let guardian_evidence = input
            .thread_store
            .get_or_init(GuardianReviewEvidence::default);
        let thread_context_enabled = guardian_evidence.uses_thread_owned_context();
        if thread_context_enabled
            && requires_sync_for_compaction(
                &guardian_config,
                input.conversation_history.as_ref(),
                &sampler,
            )
        {
            score_progress
                .latest_failed_tool_call
                .fetch_max(tool_call_index, Ordering::Release);
            Self::record_fail_closed_score(input.thread_store, sampled_at);
            record_classification(
                metrics.as_deref(),
                classification_started_at.elapsed(),
                "skipped",
            );
            return;
        }
        let parent_compaction_hash = if thread_context_enabled {
            input
                .conversation_history
                .latest_compaction_model_hash()
                .map(str::to_owned)
        } else {
            parent_model
                .as_ref()
                .and_then(|model| model.comp_hash.clone())
        };
        let parent_compaction = if guardian_config.reuse_parent_compaction {
            match encrypted_parent_compaction(
                input.conversation_history.items(),
                guardian_config.max_parent_compaction_tokens,
            ) {
                Ok(compaction) => compaction,
                Err(ParentCompactionError::Unusable) if !thread_context_enabled => None,
                Err(_) => {
                    Self::record_fail_closed_score(input.thread_store, sampled_at);
                    record_classification(
                        metrics.as_deref(),
                        classification_started_at.elapsed(),
                        "failure",
                    );
                    return;
                }
            }
        } else {
            None
        };
        // Legacy requests may omit an incompatible checkpoint because their raw
        // review transcript is still retained. The sampler rejects supplied
        // incompatible checkpoints, so preserve that legacy omission here.
        let parent_compaction = parent_compaction.filter(|_| {
            thread_context_enabled
                || sampler.supports_parent_compaction(parent_compaction_hash.as_deref())
        });
        let call_id = input.call_id.to_owned();
        let mcp_tool = input.mcp_tool.cloned();
        let action = GuardianAction {
            tool_name: input.tool_name.clone(),
            payload: input.payload.clone(),
        };
        let review_model_override = parent_model
            .as_ref()
            .and_then(|model| model.auto_review_model_override.clone());
        // Snapshot before spawning so a delayed sample cannot see later reviews.
        let sync_reviews = guardian_evidence.snapshot();
        let codex_core::context::GuardianUserInputSnapshot {
            fragments: trusted_user_inputs,
            authorization_version,
        } = guardian_evidence.user_input_snapshot(input.conversation_history.as_ref());
        let history = Arc::clone(&input.conversation_history);
        let local_trusted_skill_paths = guardian_evidence.trusted_skill_paths(input.turn_id);
        let node_repl_images = if guardian_config.transcript.include_images {
            input
                .thread_store
                .get::<NodeReplReviewEvidence>()
                .map(|evidence| evidence.images())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let rendered_images = guardian_config
            .transcript
            .images(input.conversation_history.review_items(), node_repl_images);
        // Capture root evidence before background metadata resolution or model I/O.
        // Later root changes invalidate this sample through its captured authorization version.
        let root_snapshot = if thread_context_enabled {
            thread.guardian_root_snapshot().await
        } else {
            None
        };

        let score_authorization = ScoreAuthorization::current(&thread).await;
        tokio::spawn(async move {
            let mut truncations = ClassificationTruncations::default();
            let trusted_tool_context = match mcp_tool.as_ref() {
                Some(tool) => {
                    trusted_tool_context(tool.tool_info(), tool.source(), &manager, &config).await
                }
                None => None,
            };
            let root_snapshot = if thread_context_enabled {
                root_snapshot
            } else {
                thread.guardian_root_snapshot().await
            };
            let mut trusted_skills = TrustedSkillInvocations::default();
            for path in local_trusted_skill_paths.iter().chain(
                root_snapshot
                    .as_ref()
                    .into_iter()
                    .flat_map(|snapshot| snapshot.trusted_skill_paths.iter()),
            ) {
                trusted_skills.record(path.clone());
            }
            let trusted_skill_paths = trusted_skills.into_paths();
            let root_authorization_version = root_snapshot
                .as_ref()
                .map(|snapshot| snapshot.authorization_version);
            let root_conversation = root_snapshot.map(|snapshot| snapshot.messages);
            let score_authorization = ScoreAuthorization {
                local: authorization_version,
                root: root_authorization_version,
                model: parent_model.clone(),
                ..score_authorization
            };
            let transcript = match guardian_config.transcript.build_context(
                ContextTarget::Async,
                history.as_ref(),
                root_conversation.as_deref().unwrap_or_default(),
                &trusted_user_inputs,
            ) {
                Ok(transcript) => transcript,
                Err(error) => {
                    Self::record_fail_closed_score(thread.thread_extension_data(), sampled_at);
                    record_classification(
                        metrics.as_deref(),
                        classification_started_at.elapsed(),
                        "failure",
                    );
                    event_sink.emit_warning(ExtensionWarning {
                        thread_id,
                        turn_id: Some(turn_id),
                        message: format!("Guardian V2 context collection failed: {error}"),
                    });
                    return;
                }
            };
            drop(history);
            truncations.extend(transcript.truncations);
            truncations.record(
                "transcript_image",
                rendered_images.omitted_bytes,
                /*retained_bytes*/ 0,
            );
            let images = rendered_images.images;
            let planned_action = match action.render(guardian_config.max_action_tokens) {
                Ok(RenderedAction {
                    text,
                    original_bytes,
                }) => {
                    truncations.record("action", original_bytes, text.len());
                    text
                }
                Err(error) => {
                    Self::record_fail_closed_score(thread.thread_extension_data(), sampled_at);
                    record_classification(
                        metrics.as_deref(),
                        classification_started_at.elapsed(),
                        "failure",
                    );
                    event_sink.emit_warning(ExtensionWarning {
                        thread_id,
                        turn_id: Some(turn_id),
                        message: format!("Guardian V2 action serialization failed: {error}"),
                    });
                    return;
                }
            };
            let mut classification_input = transcript.authorization;
            classification_input.push(">>> TRANSCRIPT START\n".to_owned());
            classification_input.extend(transcript.entries);
            classification_input.push(">>> TRANSCRIPT END\n\n".to_owned());
            let trusted_review_evidence = sync_reviews
                .iter()
                .filter(|review| {
                    review.authorization_version == authorization_version
                        && review.root_authorization_version == root_authorization_version
                })
                .map(|review| {
                    let review = render_review_evidence(review);
                    truncations.extend(review.truncations);
                    review.text
                })
                .collect();
            classification_input.extend([
                "The Codex agent has requested the following action:\n".to_owned(),
                ">>> APPROVAL REQUEST START\n".to_owned(),
                "Planned action JSON:\n".to_owned(),
                format!("{planned_action}\n"),
                ">>> APPROVAL REQUEST END\n".to_owned(),
            ]);
            let mut classification_risk = None;
            let mut classification_finished_at = None;
            let result: Result<ClassificationOutcome, String> = async {
                let review_model_messages = if config.guardian_policy_config.is_none() {
                    let review_model_id = review_model_override.as_deref().unwrap_or_else(|| {
                        create_model_provider(
                            config.model_provider.clone(),
                            Some(manager.auth_manager()),
                        )
                        .approval_review_preferred_model()
                    });
                    let review_model = manager
                        .get_models_manager()
                        .get_model_info(review_model_id, &config.to_models_manager_config())
                        .await;
                    if review_model.used_fallback_model_metadata && review_model_override.is_none()
                    {
                        parent_model
                            .as_ref()
                            .and_then(|model| model.model_messages.clone())
                    } else {
                        review_model.model_messages
                    }
                } else {
                    None
                };
                let policy = config.resolve_guardian_policy(review_model_messages.as_ref());
                let instructions = guardian_config.render_classifier_instructions(policy);
                let output = match sampler
                    .sample(LunaSamplingRequest {
                        parent_response_id,
                        instructions,
                        trusted_review_evidence,
                        trusted_tool_context,
                        trusted_skill_paths,
                        input: classification_input,
                        images,
                        parent_compaction,
                        parent_compaction_hash,
                        reasoning_effort: guardian_config.reasoning_effort.clone(),
                        parent_turn_id: turn_id.clone(),
                        root_turn_id,
                    })
                    .await
                {
                    Ok(output) => output,
                    Err(LunaSamplerError::Superseded) => {
                        return Ok(ClassificationOutcome::Superseded);
                    }
                    Err(error) => return Err(error.to_string()),
                };
                let (action_risk, risk_level) = match output.as_str() {
                    "high" => (1.0, "high"),
                    "low" => (0.0, "low"),
                    _ => return Err("invalid Guardian V2 classification".to_owned()),
                };
                classification_risk = Some(risk_level);
                let score = SecurityRiskScore {
                    scores: BTreeMap::from([("action_risk".to_owned(), action_risk)]),
                    call_id: Some(call_id.clone()),
                    action: Some(
                        serde_json::from_str(&planned_action).map_err(|error| error.to_string())?,
                    ),
                    sampled_at: Some(sampled_at.into()),
                };
                if score_authorization != ScoreAuthorization::current(&thread).await {
                    return Ok(ClassificationOutcome::Superseded);
                }
                let accepted = {
                    let mut scored_authorization = score_progress
                        .authorization
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    let accepted =
                        thread
                            .thread_extension_data()
                            .insert_if(score.clone(), |previous| {
                                previous
                                    .is_none_or(|previous| previous.sampled_at < score.sampled_at)
                            });
                    if accepted {
                        *scored_authorization = Some(score_authorization);
                    }
                    accepted
                };
                tracing::info!(
                    %thread_id,
                    %turn_id,
                    %call_id,
                    tool_call_index,
                    action_risk = score.scores.get("action_risk").copied(),
                    review_threshold = guardian_config.review_threshold,
                    sampled_at = ?score.sampled_at,
                    accepted,
                    "Guardian V2 classification result"
                );
                if !accepted {
                    return Ok(ClassificationOutcome::Superseded);
                }
                score_progress
                    .latest_scored_tool_call
                    .fetch_max(tool_call_index, Ordering::Release);
                classification_finished_at = Some(Instant::now());
                record_classification_risk(metrics.as_deref(), output.as_str());
                if guardian_config.persist_scores
                    && !config.ephemeral
                    && let Err(error) = thread
                        .append_rollout_items(&[RolloutItem::SecurityRiskScore(score)])
                        .await
                {
                    tracing::warn!(
                        %thread_id,
                        %turn_id,
                        %call_id,
                        %error,
                        "failed to persist Guardian V2 classification result"
                    );
                }
                Ok(ClassificationOutcome::Scored)
            }
            .await;
            if result.is_err() {
                Self::record_fail_closed_score(thread.thread_extension_data(), sampled_at);
            }
            let duration = classification_finished_at
                .map(|finished_at: Instant| finished_at.duration_since(classification_started_at))
                .unwrap_or_else(|| classification_started_at.elapsed());
            let outcome = match &result {
                Ok(ClassificationOutcome::Scored) => "success",
                Ok(ClassificationOutcome::Superseded) => "superseded",
                Err(_) => "failure",
            };
            record_classification(metrics.as_deref(), duration, outcome);
            if let Some(analytics) = analytics {
                analytics.track_guardian_v2_event(GuardianV2Event {
                    thread_id: thread_id.clone(),
                    turn_id: turn_id.clone(),
                    item_id: Some(call_id),
                    model: parent_model.as_ref().map(|model| model.slug.clone()),
                    occurred_at_ms: codex_analytics::now_unix_millis(),
                    kind: GuardianV2EventKind::Classification {
                        outcome,
                        risk_level: classification_risk,
                        duration_ms: u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
                    },
                });
            }
            if matches!(result, Ok(ClassificationOutcome::Scored)) {
                truncations.emit(metrics.as_deref());
            }
            if let Err(error) = result {
                event_sink.emit_warning(ExtensionWarning {
                    thread_id,
                    turn_id: Some(turn_id),
                    message: format!("Guardian V2 risk scoring failed: {error}"),
                });
            }
        });
    }
}

#[derive(Debug, Eq, PartialEq)]
enum ParentCompactionError {
    Serialization,
    Oversized,
    Unusable,
}

// Sampling and fast approval must apply the same checkpoint eligibility policy.
pub(super) fn requires_sync_for_compaction(
    config: &GuardianV2Config,
    history: &dyn ConversationHistorySnapshot,
    sampler: &LunaSampler,
) -> bool {
    history.items().any(|item| {
        matches!(
            item,
            ResponseItem::Compaction { .. } | ResponseItem::ContextCompaction { .. }
        )
    }) && (!config.reuse_parent_compaction
        || !sampler.supports_parent_compaction(history.latest_compaction_model_hash()))
}

// An unusable latest compaction must never fall back to an older one. Missing
// encrypted content is rejected here; only legacy callers may omit that checkpoint.
fn encrypted_parent_compaction<'a>(
    items: impl Iterator<Item = &'a ResponseItem>,
    max_parent_compaction_tokens: usize,
) -> Result<Option<ResponseItem>, ParentCompactionError> {
    let max_compaction_bytes = TruncationPolicy::Tokens(max_parent_compaction_tokens).byte_budget();
    let Some(item) = items
        .filter(|item| {
            matches!(
                item,
                ResponseItem::Compaction { .. } | ResponseItem::ContextCompaction { .. }
            )
        })
        .last()
    else {
        return Ok(None);
    };

    let encrypted_content = match item {
        ResponseItem::Compaction {
            id: Some(_),
            encrypted_content,
            ..
        }
        | ResponseItem::ContextCompaction {
            id: Some(_),
            encrypted_content: Some(encrypted_content),
            ..
        } => encrypted_content,
        _ => return Err(ParentCompactionError::Unusable),
    };
    if encrypted_content.is_empty() {
        return Err(ParentCompactionError::Unusable);
    }
    let serialized = serde_json::to_vec(item).map_err(|_| ParentCompactionError::Serialization)?;
    if serialized.len() > max_compaction_bytes {
        return Err(ParentCompactionError::Oversized);
    }

    Ok(Some(item.clone()))
}

/// Installs feature-gated Guardian V2 tool classification for each thread.
pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    auth_manager: Arc<AuthManager>,
    thread_manager: Weak<ThreadManager>,
) {
    let extension = Arc::new(GuardianV2Extension {
        auth_manager,
        event_sink: registry.event_sink(),
        thread_manager: thread_manager.clone(),
    });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.approval_review_contributor(Arc::new(super::approval::GuardianApprovalReviewer {
        thread_manager,
    }));
    registry.approval_review_contributor(extension.clone());
    registry.skill_invocation_contributor(extension.clone());
    registry.tool_lifecycle_contributor(extension);
}

#[cfg(test)]
#[path = "extension_tests.rs"]
mod tests;
