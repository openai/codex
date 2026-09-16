//! Builds context and samples a captured observation in the background.
//! Successful results require current authorization and a newer sample timestamp.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;
use std::time::SystemTime;

use codex_analytics::AnalyticsEventsClient;
use codex_analytics::GuardianV2Event;
use codex_analytics::GuardianV2EventKind;
use codex_core::CodexThread;
use codex_core::GuardianAuthorizationVersion;
use codex_core::GuardianRootSnapshot;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::context::ContextualUserFragment;
use codex_core::context::GuardianContextMode;
use codex_core::context::GuardianReviewEvidenceFragment;
use codex_core::context::GuardianReviewEvidenceRecord;
use codex_extension_api::ConversationHistorySnapshot;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionMetrics;
use codex_extension_api::ExtensionWarning;
use codex_extension_api::McpToolContext;
use codex_extension_api::ResponseItem;
use codex_guardian_context::ContextTarget;
use codex_guardian_context::PlannedAction;
use codex_guardian_context::PlannedActionKind;
use codex_guardian_context::PreviousReviews;
use codex_guardian_context::ReviewEvidence;
use codex_guardian_context::render_review_evidence;
use codex_history::RolloutItem;
use codex_model_provider::create_model_provider;
use codex_prompts::ResolvedModelMessages;
use codex_protocol::models::ContentItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::security_risk::SecurityRiskScore;

use super::authorization::ScoreAuthorization;
use super::config::GuardianV2Config;
use super::metrics::record_classification;
use super::metrics::record_classification_risk;
use super::metrics::sampler_failure_reason;
use super::sampler::LunaSampler;
use super::sampler::LunaSamplerError;
use super::sampler::LunaSamplingRequest;
use super::score::GuardianV2ScoreProgress;
use super::score::record_fail_closed_score;
use super::transcript::ContextInput;
use super::truncation::ClassificationTruncations;
use super::trusted_skills::TrustedSkillInvocations;
use super::trusted_tools::trusted_tool_context;

pub(super) struct Classification {
    pub(super) classification_started_at: Instant,
    pub(super) sampler: Arc<LunaSampler>,
    pub(super) guardian_config: GuardianV2Config,
    pub(super) score_progress: Arc<GuardianV2ScoreProgress>,
    pub(super) parent_model: Option<Arc<ModelInfo>>,
    pub(super) metrics: Option<Arc<dyn ExtensionMetrics>>,
    pub(super) analytics: Option<Arc<AnalyticsEventsClient>>,
    pub(super) sampled_at: SystemTime,
    pub(super) tool_call_index: usize,
    pub(super) event_sink: Arc<dyn ExtensionEventSink>,
    pub(super) thread_id: String,
    pub(super) turn_id: String,
    pub(super) root_turn_id: Option<String>,
    pub(super) parent_response_id: Option<String>,
    pub(super) manager: Arc<ThreadManager>,
    pub(super) thread: Arc<CodexThread>,
    pub(super) config: Arc<Config>,
    pub(super) context_mode: GuardianContextMode,
    pub(super) parent_compaction: Option<ResponseItem>,
    pub(super) parent_compaction_hash: Option<String>,
    pub(super) call_id: String,
    pub(super) mcp_tool: Option<McpToolContext>,
    pub(super) planned_action: String,
    pub(super) review_model_override: Option<String>,
    pub(super) sync_reviews: Vec<Arc<GuardianReviewEvidenceRecord>>,
    pub(super) trusted_user_inputs: Vec<String>,
    pub(super) authorization_version: GuardianAuthorizationVersion,
    pub(super) history: Arc<dyn ConversationHistorySnapshot>,
    pub(super) local_trusted_skill_paths: Vec<String>,
    pub(super) node_repl_images: Vec<ContentItem>,
    pub(super) root_snapshot: Option<GuardianRootSnapshot>,
    pub(super) score_authorization: ScoreAuthorization,
}

enum ClassificationOutcome {
    Scored,
    Superseded,
}

impl Classification {
    pub(super) async fn run(self) {
        let Self {
            classification_started_at,
            sampler,
            guardian_config,
            score_progress,
            parent_model,
            metrics,
            analytics,
            sampled_at,
            tool_call_index,
            event_sink,
            thread_id,
            turn_id,
            root_turn_id,
            parent_response_id,
            manager,
            thread,
            config,
            context_mode,
            parent_compaction,
            parent_compaction_hash,
            call_id,
            mcp_tool,
            planned_action,
            review_model_override,
            sync_reviews,
            trusted_user_inputs,
            authorization_version,
            history,
            local_trusted_skill_paths,
            node_repl_images,
            root_snapshot,
            score_authorization,
        } = self;
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
                record_fail_closed_score(thread.thread_extension_data(), sampled_at);
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
            let review_model = if config.guardian_policy_config.is_none() {
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
                if review_model.used_fallback_model_metadata && review_model_override.is_none() {
                    parent_model.clone()
                } else {
                    Some(Arc::new(review_model))
                }
            } else {
                None
            };
            let model_messages = review_model
                .as_deref()
                .map(ResolvedModelMessages::from_model)
                .unwrap_or_else(ResolvedModelMessages::bundled);
            let policy = config.resolve_guardian_policy(model_messages);
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
                            previous.is_none_or(|previous| previous.sampled_at < score.sampled_at)
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
            record_fail_closed_score(thread.thread_extension_data(), sampled_at);
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
    }
}
