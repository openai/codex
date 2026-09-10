use super::transcript::ContextInput;
use codex_core::context::ContextualUserFragment;
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
use codex_core::context::GuardianContextMode;
use codex_core::context::GuardianReviewEvidence;
use codex_core::context::NodeReplReviewEvidence;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionMetrics;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ExtensionWarning;
use codex_extension_api::GuardianV2Enabled;
use codex_extension_api::SkillInvocationContributor;
use codex_extension_api::SkillInvocationInput;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::ToolStartInput;
use codex_features::Feature;
use codex_guardian_context::ContextTarget;
use codex_guardian_context::PlannedAction;
use codex_guardian_context::PlannedActionKind;
use codex_history::RolloutItem;
use codex_login::AuthManager;
use codex_model_provider::create_model_provider;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::mcp::is_node_repl_backed_server;
use codex_protocol::openai_models::GuardianScope;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::has_full_access;
use codex_protocol::security_risk::SecurityRiskScore;

use super::action::GuardianAction;
use super::action::RenderedAction;
use super::authorization::ScoreAuthorization;
use super::config::GuardianV2Config;
use super::coverage::UnscoredAction;
use super::metrics::record_classification;
use super::metrics::record_classification_risk;
use super::metrics::sampler_failure_reason;
use super::parent_compaction::ParentCompactionError;
use super::parent_compaction::select_parent_compaction;
use super::sampler::LunaSampler;
use super::sampler::LunaSamplerError;
use super::sampler::LunaSamplingRequest;
use super::truncation::ClassificationTruncations;
use super::trusted_skills::TrustedSkillInvocations;
use super::trusted_skills::TrustedSkillRoots;
use super::trusted_tools::trusted_tool_context;
use super::wrapper_lag::WrapperLag;
use codex_core::context::GuardianReviewEvidenceFragment;
use codex_guardian_context::PreviousReviews;
use codex_guardian_context::ReviewEvidence;
use codex_guardian_context::render_review_evidence;

enum ClassificationOutcome {
    Scored,
    Superseded,
}

#[derive(Default)]
pub(super) struct GuardianV2ScoreProgress {
    pub(super) wrapper_lag: WrapperLag,
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
            let sampler_config = super::startup::sampler_config(
                &input,
                Arc::clone(&self.auth_manager),
                self.thread_manager.upgrade(),
            )
            .await;

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
                    let index = score_progress
                        .latest_tool_call
                        .fetch_add(/*val*/ 1, Ordering::Relaxed)
                        .saturating_add(/*rhs*/ 1);
                    score_progress.wrapper_lag.record(&input, index);
                }
                UnscoredAction::InvalidateScore => {
                    let index = score_progress
                        .latest_tool_call
                        .fetch_add(/*val*/ 1, Ordering::Relaxed)
                        .saturating_add(/*rhs*/ 1);
                    score_progress.wrapper_lag.record(&input, index);
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
        score_progress.wrapper_lag.record(&input, tool_call_index);
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
                    Some("thread_context_error"),
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
                    Some("configuration_error"),
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
        let context_mode = guardian_evidence.context_mode();
        let selected_compaction = match select_parent_compaction(
            context_mode,
            &guardian_config,
            input.conversation_history.as_ref(),
            &sampler,
            parent_model
                .as_ref()
                .and_then(|model| model.comp_hash.as_deref()),
        ) {
            Ok(compaction) => compaction,
            Err(error) => {
                let (outcome, failure_reason) = if error == ParentCompactionError::RequiresSync {
                    score_progress
                        .latest_failed_tool_call
                        .fetch_max(tool_call_index, Ordering::Release);
                    ("skipped", None)
                } else {
                    ("failure", Some("parent_compaction_error"))
                };
                Self::record_fail_closed_score(input.thread_store, sampled_at);
                record_classification(
                    metrics.as_deref(),
                    classification_started_at.elapsed(),
                    outcome,
                    failure_reason,
                );
                return;
            }
        };
        let parent_compaction = selected_compaction.item;
        let parent_compaction_hash = selected_compaction.model_hash;
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
        // Capture root evidence before background metadata resolution or model I/O.
        // Later root changes invalidate this sample through its captured authorization version.
        let root_snapshot = if context_mode == GuardianContextMode::ThreadOwned {
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
            let root_snapshot = if context_mode == GuardianContextMode::ThreadOwned {
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
                        Some("action_serialization_error"),
                    );
                    event_sink.emit_warning(ExtensionWarning {
                        thread_id,
                        turn_id: Some(turn_id),
                        message: format!("Guardian V2 action serialization failed: {error}"),
                    });
                    return;
                }
            };
            let action_section = PlannedAction {
                json: planned_action.clone(),
                tool_descriptions: None,
                kind: PlannedActionKind::Command,
                reason: None,
            };
            let review_fragments = sync_reviews
                .iter()
                .filter(|review| {
                    review.authorization_version == authorization_version
                        && review.root_authorization_version == root_authorization_version
                })
                .map(|review| {
                    let review = render_review_evidence(ReviewEvidence {
                        correlation: &review.correlation,
                        decision: &review.decision,
                        action: &review.action,
                        rationale: review.rationale.as_deref(),
                    });
                    truncations.extend(review.truncations);
                    GuardianReviewEvidenceFragment::new(review.body).render()
                })
                .collect::<Vec<_>>();
            let transcript =
                PreviousReviews::try_from_fragments(review_fragments).and_then(|reviews| {
                    guardian_config.transcript.build_context(ContextInput {
                        target: ContextTarget::Async,
                        history: history.as_ref(),
                        root_conversation: root_conversation.as_deref().unwrap_or_default(),
                        trusted_user_answers: &trusted_user_inputs,
                        planned_action: Some(&action_section),
                        previous_reviews: Some(&reviews),
                        trusted_tool: trusted_tool_context.as_ref(),
                        trusted_skill_paths: &trusted_skill_paths,
                        node_repl_images: Some(&node_repl_images),
                    })
                });
            let mut transcript = match transcript {
                Ok(transcript) => transcript,
                Err(error) => {
                    Self::record_fail_closed_score(thread.thread_extension_data(), sampled_at);
                    record_classification(
                        metrics.as_deref(),
                        classification_started_at.elapsed(),
                        "failure",
                        Some("context_build_error"),
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
            drop(node_repl_images);
            truncations.extend(std::mem::take(&mut transcript.truncations));
            if let Some(metrics) = metrics.as_deref() {
                for (section, cost) in transcript.section_costs() {
                    for (measurement, value) in cost.measurements() {
                        metrics.histogram_with_boundaries(
                            codex_guardian_context::SECTION_COST_METRIC,
                            i64::try_from(value).unwrap_or(i64::MAX),
                            codex_guardian_context::SECTION_COST_BOUNDARIES,
                            &[
                                ("target", "async"),
                                ("section", section),
                                ("measurement", measurement),
                            ],
                        );
                    }
                }
            }
            let classification_input = transcript.into_messages();
            let mut failure_reason = "invalid_output";
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
                        input: classification_input,
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
                    Err(error) => {
                        failure_reason = sampler_failure_reason(&error);
                        return Err(error.to_string());
                    }
                };
                let (action_risk, risk_level) = match output.as_str() {
                    "high" => (1.0, "high"),
                    "low" => (0.0, "low"),
                    _ => return Err("invalid Guardian V2 classification".to_owned()),
                };
                classification_risk = Some(risk_level);
                failure_reason = "action_deserialization_error";
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
            record_classification(
                metrics.as_deref(),
                duration,
                outcome,
                result.is_err().then_some(failure_reason),
            );
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

/// Installs feature-gated Guardian V2 tool classification for each thread.
pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    auth_manager: Arc<AuthManager>,
    thread_manager: Weak<ThreadManager>,
) {
    let extension = Arc::new(GuardianV2Extension {
        auth_manager,
        event_sink: registry.event_sink(),
        thread_manager,
    });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.approval_review_contributor(Arc::new(super::approval::GuardianApprovalReviewer {
        thread_manager: extension.thread_manager.clone(),
    }));
    registry.skill_invocation_contributor(extension.clone());
    registry.tool_lifecycle_contributor(extension);
}

#[cfg(test)]
#[path = "extension_tests.rs"]
mod tests;
