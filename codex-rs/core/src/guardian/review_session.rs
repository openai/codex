//! Host adapter for synchronous Guardian sessions and the existing context builder.
//! The extension owns review policy and pooling; this module binds runtime operations
//! to the captured parent action, environments, authorization and context snapshots.

#[path = "feedback.rs"]
mod feedback;

#[path = "review_session_setup.rs"]
mod setup;
pub use setup::PreparedGuardianContext;
pub use setup::prepare_review_prewarm;
pub(crate) use setup::run_guardian_review_session;

#[path = "review_session_prompt.rs"]
mod prompt_builder;

#[path = "review_session_context.rs"]
mod context_policy;
use codex_guardian_reviewer::ReviewContextPolicy;
use codex_guardian_reviewer::ReviewDeadline;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
#[cfg(test)]
use std::time::Duration;

use codex_analytics::GuardianReviewAnalyticsResult;
use codex_analytics::GuardianReviewSessionAnalyticsParams;
use codex_analytics::GuardianReviewSessionKind;
use codex_extension_api::Instructions;
#[cfg(test)]
use codex_guardian_reviewer::ConversationState;
use codex_guardian_reviewer::GuardianReviewForkSnapshot;
pub use codex_guardian_reviewer::GuardianReviewState;
use codex_guardian_reviewer::GuardianTranscriptSource;
use codex_guardian_reviewer::PendingNodeReplEvidenceAdmission;
use codex_guardian_reviewer::ReviewModel;
use codex_guardian_reviewer::ReviewSessionResult;
use codex_guardian_reviewer::ReviewTurnResult;
use codex_guardian_reviewer::SessionDisposition;
use codex_guardian_reviewer::token_usage_delta;
use codex_history::InitialHistory;
#[cfg(test)]
use codex_history::RolloutItem;
use codex_protocol::config_types::AutoCompactTokenLimitScope;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::GuardianScope;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::Event;
#[cfg(test)]
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;
use futures::future::BoxFuture;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::agents_md_manager::SessionInstructions;
use crate::config::Config;
use crate::config::Constrained;
use crate::config::ManagedFeatures;
use crate::config::Permissions;
use crate::context::ContextualUserFragment;
use crate::context::GuardianContextMode;
use crate::context::GuardianFollowupReviewReminder;
use crate::context::GuardianNodeReplPolicy;
use crate::context_manager::ContextManager;
use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::SessionIo;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_config::types::McpServerConfig;
use codex_features::Feature;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::turn_input::TurnInputMode;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnInputSubmission;
use codex_thread_store::PersistContext;
use codex_utils_path_uri::PathUri;

use super::ApprovalRequestReasons;
use super::GUARDIAN_REVIEWER_NAME;
use super::GuardianApprovalRequest;
use super::GuardianReviewContext;
use super::input_budget::PendingReviewContext;
use super::prompt::GUARDIAN_TRANSCRIPT_START;
use super::prompt::GuardianPromptMode;
#[cfg(test)]
use super::prompt::GuardianTranscriptCursor;
use super::prompt::GuardianTranscriptHistory;
use super::request_budget::ExhaustedReviewBudget;
use super::review::guardian_review_session_config;
pub(crate) use super::reviewer_config::build_guardian_review_session_config;
use codex_guardian_reviewer::run_before_review_deadline;
use codex_guardian_reviewer::wait_for_guardian_review;

pub(crate) use codex_guardian_reviewer::GuardianReviewSessionOutcome;

pub(crate) struct GuardianReviewSessionParams {
    pub(crate) parent_session: Arc<Session>,
    pub(crate) parent_context: GuardianReviewContext,
    // Checkpoint selection and thread-owned prompt evidence must use the same history.
    pub(crate) parent_history: ContextManager,
    pub(crate) spawn_config: Config,
    pub(crate) node_repl_policy: GuardianNodeReplPolicy,
    pub(crate) request: GuardianApprovalRequest,
    pub(crate) category: GuardianScope,
    pub(crate) reasons: ApprovalRequestReasons,
    pub(super) authorization: Option<super::review::ReviewAuthorization>,
    pub(crate) schema: Value,
    pub(crate) review_model: ReviewModel,
    pub(crate) external_cancel: Option<CancellationToken>,
    pub(crate) deadline: ReviewDeadline,
}

pub(crate) type GuardianReviewSessionManager =
    codex_guardian_reviewer::ReviewerPool<GuardianReviewSession>;

/// Opaque host session handle. Its state belongs to the existing context builder.
pub struct GuardianReviewSession {
    session: Arc<Session>,
    io: SessionIo,
    cancel_token: CancellationToken,
    reuse_key: GuardianReviewSessionReuseKey,
    state: Mutex<GuardianReviewState>,
}

fn had_prior_review_context(prompt_mode: &GuardianPromptMode) -> bool {
    matches!(prompt_mode, GuardianPromptMode::Delta { .. })
}

/// Opaque compatibility key derived by the existing context builder.
#[derive(Debug, Clone, PartialEq)]
pub struct GuardianReviewSessionReuseKey {
    // Only include settings that affect spawned-session behavior and parent
    // history rewrites that invalidate existing reviewer context.
    parent_history_version: u64,
    parent_reset_version: u64,
    root_review_version: Option<(crate::codex_thread::GuardianAuthorizationVersion, u64)>,
    node_repl_auto_review_required: bool,
    node_repl_policy: String,
    model: Option<String>,
    model_provider_id: String,
    model_provider: ModelProviderInfo,
    model_context_window: Option<i64>,
    model_auto_compact_token_limit: Option<i64>,
    model_auto_compact_token_limit_scope: AutoCompactTokenLimitScope,
    model_reasoning_effort: Option<ReasoningEffortConfig>,
    model_reasoning_summary: Option<ReasoningSummaryConfig>,
    omit_personality: bool,
    permissions: Permissions,
    developer_instructions: Option<String>,
    base_instructions: Option<String>,
    user_instructions: Option<Instructions>,
    thread_instructions: Option<Instructions>,
    project_instructions: Option<Arc<crate::agents_md::LoadedAgentsMd>>,
    compact_prompt: Option<String>,
    cwd: PathUri,
    mcp_servers: Constrained<HashMap<String, McpServerConfig>>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    features: ManagedFeatures,
    environment_ids: Vec<String>,
}

impl GuardianReviewSessionReuseKey {
    fn from_spawn_config(
        spawn_config: &Config,
        instructions: SessionInstructions,
        parent_history_version: u64,
        context_mode: GuardianContextMode,
    ) -> Self {
        Self {
            root_review_version: None,
            parent_reset_version: 0,
            parent_history_version: match ReviewContextPolicy::for_context(
                context_mode,
                &spawn_config.features,
            ) {
                ReviewContextPolicy::Legacy | ReviewContextPolicy::Independent => 0,
                ReviewContextPolicy::LegacyWithCheckpointReuse
                | ReviewContextPolicy::ThreadOwned => parent_history_version,
            },
            node_repl_auto_review_required: false,
            node_repl_policy: String::new(),
            model: spawn_config.model.clone(),
            model_provider_id: spawn_config.model_provider_id.clone(),
            model_provider: spawn_config.model_provider.clone(),
            model_context_window: spawn_config.model_context_window,
            model_auto_compact_token_limit: spawn_config.model_auto_compact_token_limit,
            model_auto_compact_token_limit_scope: spawn_config.model_auto_compact_token_limit_scope,
            model_reasoning_effort: spawn_config.model_reasoning_effort.clone(),
            model_reasoning_summary: spawn_config.model_reasoning_summary,
            omit_personality: spawn_config.personality == Some(Personality::None),
            permissions: spawn_config.permissions.clone(),
            developer_instructions: spawn_config.developer_instructions.clone(),
            base_instructions: spawn_config.base_instructions.clone(),
            user_instructions: instructions.user,
            thread_instructions: instructions.thread,
            project_instructions: instructions.project_snapshot,
            compact_prompt: spawn_config.compact_prompt.clone(),
            cwd: PathUri::from_abs_path(&spawn_config.cwd),
            mcp_servers: spawn_config.mcp_servers.clone(),
            codex_linux_sandbox_exe: spawn_config.codex_linux_sandbox_exe.clone(),
            features: spawn_config.features.clone(),
            environment_ids: Vec::new(),
        }
    }

    fn with_environments(mut self, environments: &TurnEnvironmentSnapshot) -> Self {
        self.environment_ids = environments
            .captured_environments()
            .into_keys()
            .collect::<Vec<_>>();
        self.environment_ids.sort_unstable();
        self
    }

    fn with_node_repl_policy_eligibility(mut self, required: bool) -> Self {
        self.node_repl_auto_review_required = required;
        self
    }

    fn with_node_repl_policy(mut self, policy: &GuardianNodeReplPolicy) -> Self {
        self.node_repl_policy = policy.body().into_text().unwrap_or_default();
        self
    }
}

pub(super) fn review_turn(
    session: &Session,
    context: &GuardianReviewContext,
    config: &Config,
    review_model: &ReviewModel,
    items: Vec<codex_protocol::user_input::UserInput>,
    schema: Value,
) -> TurnInputRequest {
    let parent_turn_environments = context
        .environments()
        .turn_environments()
        .map(|environment| {
            let mut selection = environment.selection();
            selection.config = codex_protocol::protocol::EnvironmentConfigState::Ready(
                environment.config().clone(),
            );
            selection
        })
        .collect();
    // TODO(anp): Migrate guardian review thread settings to a PathUri fallback cwd so foreign
    // parent environments do not fall back to the host-native config cwd.
    let parent_turn_legacy_fallback_cwd = context
        .environments()
        .primary()
        .and_then(|environment| environment.cwd().to_abs_path().ok())
        .unwrap_or_else(|| context.turn().config.cwd.clone());

    let parent_turn = context.turn();
    codex_guardian_reviewer::ReviewerTurn {
        items,
        environments: codex_protocol::protocol::TurnEnvironmentSelections::new(
            parent_turn_legacy_fallback_cwd,
            parent_turn_environments,
        ),
        permission_profile: config.permissions.permission_profile().clone(),
        reasoning_summary: config.model_reasoning_summary.unwrap_or_default(),
        personality: config.personality,
        model: review_model.model.clone(),
        reasoning_effort: review_model.reasoning_effort.clone(),
        cyber_access_program: parent_turn.cyber_access_program.filter(|_| {
            crate::RequiredCyberAccessPrograms::for_thread(&session.services.thread_extension_data)
                .is_none()
        }),
        parent_response_id: context
            .parent_response
            .as_ref()
            .map(|parent| parent.id.clone()),
        schema,
        parent_turn_id: parent_turn.sub_id.clone(),
        root_turn_id: parent_turn.turn_metadata_state.root_turn_id(),
    }
    .into_request()
}

async fn run_review_on_session(
    review_session: &GuardianReviewSession,
    params: &GuardianReviewSessionParams,
    guardian_session_kind: GuardianReviewSessionKind,
    deadline: ReviewDeadline,
) -> ReviewSessionResult {
    let review_model = &params.review_model;
    let actor_turn = params.parent_context.turn();
    let inherited_turn = actor_turn
        .config
        .guardian_model_provider
        .is_none()
        .then_some(actor_turn.as_ref());
    let models_manager = if inherited_turn.is_some() {
        &params.parent_session.services.models_manager
    } else {
        &review_session.session.services.models_manager
    };
    let model_info = models_manager
        .get_model_info(
            review_model.model.as_str(),
            &params.spawn_config.to_models_manager_config(),
        )
        .await;
    let (prior_review_count, had_prior_context) = {
        let state = review_session.state.lock().await;
        (
            state.conversation.completed_review_count(),
            state.conversation.cursor().is_some(),
        )
    };
    let mut analytics_result = review_session.analytics(
        review_model,
        model_info.default_reasoning_level.as_ref(),
        &params.spawn_config.model_provider_id,
        guardian_session_kind,
        had_prior_context,
    );
    if prior_review_count > 0 {
        ensure_guardian_followup_reminder(review_session).await;
    }

    match run_before_review_deadline(
        deadline,
        params.external_cancel.as_ref(),
        Box::pin(prepare_verdict_session(
            review_session,
            &params.parent_context,
            params.category,
            &params.node_repl_policy,
        )),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            return ReviewSessionResult {
                outcome: GuardianReviewSessionOutcome::SessionFailed {
                    error,
                    error_info: None,
                    retry_at: None,
                },
                disposition: SessionDisposition::Discard,
                analytics: analytics_result,
            };
        }
        Err(outcome) => {
            return ReviewSessionResult {
                outcome,
                disposition: SessionDisposition::Discard,
                analytics: analytics_result,
            };
        }
    }

    let needs_compaction = params.spawn_config.features.enabled(Feature::TokenBudget)
        && crate::session::context_window::context_window_token_status_for_model(
            review_session.session.as_ref(),
            &params.spawn_config,
            inherited_turn,
            &model_info,
        )
        .await
        .token_limit_reached;
    let should_compact = if needs_compaction {
        match super::input_budget::should_compact(&review_session.session) {
            Ok(should_compact) => should_compact,
            Err(error) => {
                return ReviewSessionResult {
                    outcome: GuardianReviewSessionOutcome::PromptBuildFailed(error.into()),
                    disposition: SessionDisposition::Discard,
                    analytics: analytics_result,
                };
            }
        }
    } else {
        false
    };
    if should_compact {
        let mut execution_attempt =
            crate::turn_timing::review_execution::ReviewExecutionObservation::install(
                Arc::clone(&review_session.session),
                params.parent_context.review_execution.clone(),
            );
        let compact_submission = run_before_review_deadline(
            deadline,
            params.external_cancel.as_ref(),
            review_session.io.submit(Op::Compact),
        )
        .await;
        let compact_turn_id = match compact_submission {
            Ok(Ok(turn_id)) => turn_id,
            Ok(Err(error)) => {
                return ReviewSessionResult {
                    outcome: GuardianReviewSessionOutcome::SessionFailed {
                        error: error.into(),
                        error_info: None,
                        retry_at: None,
                    },
                    disposition: SessionDisposition::Discard,
                    analytics: analytics_result,
                };
            }
            Err(outcome) => {
                return ReviewSessionResult {
                    outcome,
                    disposition: SessionDisposition::Discard,
                    analytics: analytics_result,
                };
            }
        };
        let result = wait_for_guardian_review(
            review_session,
            &compact_turn_id,
            deadline,
            params.external_cancel.as_ref(),
            &mut analytics_result,
        )
        .await;
        execution_attempt.finish(result.turn_completed);
        if !matches!(
            result.outcome,
            GuardianReviewSessionOutcome::Completed(Ok(_))
        ) {
            return ReviewSessionResult {
                outcome: result.outcome,
                disposition: result.disposition,
                analytics: analytics_result,
            };
        }

        if prior_review_count > 0 {
            ensure_guardian_followup_reminder(review_session).await;
        }
    }

    let reviewer_history = review_session.session.clone_history().await;
    let reviewer_history_version = reviewer_history.history_version();
    let reviewer_has_full_transcript = reviewer_history.raw_items().any(|item| {
        matches!(item, ResponseItem::Message { role, content, .. }
        if role == "user" && content.iter().any(|content| {
            matches!(content, ContentItem::InputText { text }
                if text == GUARDIAN_TRANSCRIPT_START)
        }))
    });
    let (prompt_mode, last_admitted_node_repl_response_sequence, previous_transcript_source) = {
        let mut state = review_session.state.lock().await;
        state.pending_node_repl_evidence_admission = None;
        if state.transcript_history_version != reviewer_history_version
            && let Some(recovery) = review_session
                .session
                .services
                .thread_extension_data
                .get::<super::input_budget::CheckpointRecovery>()
        {
            recovery.requested.store(/*val*/ true, Ordering::Release);
            return ReviewSessionResult {
                outcome: GuardianReviewSessionOutcome::PromptBuildFailed(anyhow::anyhow!(
                    "Guardian history changed; restart from the parent checkpoint"
                )),
                disposition: SessionDisposition::Discard,
                analytics: analytics_result,
            };
        }
        if !reviewer_has_full_transcript {
            state.conversation.reset_transcript();
            state.transcript_source = None;
            state.last_admitted_node_repl_response_sequence = 0;
        }

        let prompt_mode = state
            .conversation
            .cursor()
            .map_or(GuardianPromptMode::Full, |cursor| {
                GuardianPromptMode::Delta { cursor }
            });
        (
            prompt_mode,
            state.last_admitted_node_repl_response_sequence,
            state.transcript_source,
        )
    };
    let prompt_items = run_before_review_deadline(
        deadline,
        params.external_cancel.as_ref(),
        Box::pin(async {
            params
                .parent_session
                .services
                .network_approval
                .sync_session_approved_hosts_to(&review_session.session.services.network_approval)
                .await;

            let parent_history = params.parent_history.conversation_history_snapshot();
            let policy = ReviewContextPolicy::for_context(
                GuardianContextMode::from_history(parent_history.as_ref()),
                &params.spawn_config.features,
            );
            let loaded_parent_checkpoint = policy
                .parent_compaction(params.parent_history.annotated_items())?
                .as_ref()
                .zip(codex_history::CompactionCheckpoint::latest(
                    reviewer_history.annotated_items(),
                ))
                .is_some_and(|(parent, loaded)| parent == loaded.item);
            // Keep the action-time snapshot: a later parent compaction must not
            // replace evidence with a checkpoint this reviewer never received.
            let (history, transcript_source) = if loaded_parent_checkpoint {
                (
                    GuardianTranscriptHistory::LoadedParentCheckpoint(parent_history.as_ref()),
                    GuardianTranscriptSource::LoadedParentCheckpoint,
                )
            } else {
                (
                    GuardianTranscriptHistory::Retained(parent_history.as_ref()),
                    GuardianTranscriptSource::Retained,
                )
            };
            let prompt_mode = if previous_transcript_source == Some(transcript_source) {
                prompt_mode
            } else {
                GuardianPromptMode::Full
            };
            analytics_result.had_prior_review_context =
                Some(had_prior_review_context(&prompt_mode));
            let prompt_items = prompt_builder::build_guardian_prompt_items(
                params,
                review_session,
                &model_info,
                &reviewer_history,
                history,
                prompt_mode,
                last_admitted_node_repl_response_sequence,
            )
            .await?;

            let items = match prompt_items.context.clone().into_user_inputs() {
                Ok(items) => items,
                Err(codex_guardian_context::SectionError::UnsupportedDelivery {
                    section: "conversation_transcript" | "planned_action",
                }) => {
                    // Final admission replaces this turn-start marker with the complete,
                    // budgeted context, including native encrypted messages and tool calls.
                    vec![codex_protocol::user_input::UserInput::Text {
                        text: super::prompt::GUARDIAN_TRANSCRIPT_START.to_owned(),
                        text_elements: Vec::new(),
                    }]
                }
                Err(error) => return Err(error.into()),
            };
            Ok::<_, anyhow::Error>((prompt_items, items, transcript_source))
        }),
    )
    .await;
    // Release the shared snapshot before recording input so it does not force
    // a copy of the accumulated reviewer history during inference.
    drop(reviewer_history);
    let prompt_items = match prompt_items {
        Ok(prompt_items) => prompt_items,
        Err(outcome) => {
            return ReviewSessionResult {
                outcome,
                disposition: SessionDisposition::Discard,
                analytics: analytics_result,
            };
        }
    };
    let (prompt_items, items, transcript_source) = match prompt_items {
        Ok(prompt_items) => prompt_items,
        Err(err) => {
            return ReviewSessionResult {
                outcome: GuardianReviewSessionOutcome::PromptBuildFailed(err),
                disposition: SessionDisposition::Discard,
                analytics: analytics_result,
            };
        }
    };
    let transcript_cursor = prompt_items.transcript_cursor;
    let node_repl_evidence_admission = (prompt_items.node_repl_evidence_sequence
        > last_admitted_node_repl_response_sequence)
        .then_some(prompt_items.node_repl_evidence_sequence);
    let token_usage_at_review_start = review_session.token_usage_baseline().await;
    let request = review_turn(
        &params.parent_session,
        &params.parent_context,
        &params.spawn_config,
        review_model,
        items,
        params.schema.clone(),
    );
    let result = review_session
        .run_sync_verdict(
            &params.parent_context,
            PreparedVerdictTurn {
                request,
                evidence: PendingReviewContext {
                    context: prompt_items.context,
                    elastic: params.spawn_config.guardian_sync_elastic_budget.clone(),
                },
                node_repl_evidence_sequence: node_repl_evidence_admission,
            },
            deadline,
            params.external_cancel.as_ref(),
            &mut analytics_result,
        )
        .await;
    let (turn_result, transcript_admitted) = match result {
        Ok(completed) => completed,
        Err(outcome) => {
            return ReviewSessionResult {
                outcome,
                disposition: SessionDisposition::Discard,
                analytics: analytics_result,
            };
        }
    };
    let completed_review = matches!(
        turn_result.outcome,
        GuardianReviewSessionOutcome::Completed(_)
    );
    review_session
        .record_verdict_usage(&token_usage_at_review_start, &mut analytics_result)
        .await;
    if completed_review
        || (transcript_admitted
            && turn_result.turn_completed
            && turn_result.disposition == SessionDisposition::Reusable)
    {
        let mut state = review_session.state.lock().await;
        if completed_review {
            state.conversation.complete_review(transcript_cursor);
        } else {
            // A failed inference still retains its input. Keep admission progress
            // in sync without counting the failure as a completed review.
            state.conversation.admit_transcript(transcript_cursor);
        }
        state.transcript_history_version = reviewer_history_version;
        state.transcript_source = Some(transcript_source);
    }
    ReviewSessionResult {
        outcome: turn_result.outcome,
        disposition: turn_result.disposition,
        analytics: analytics_result,
    }
}

pub(super) async fn ensure_guardian_followup_reminder(review_session: &GuardianReviewSession) {
    let followup_reminder = GuardianFollowupReviewReminder
        .body()
        .into_text()
        .unwrap_or_default();
    let already_injected = review_session
        .session
        .clone_history()
        .await
        .raw_items()
        .any(|item| {
            matches!(item, ResponseItem::Message { role, content, .. }
            if role == "developer"
                && content.iter().any(|content| {
                    matches!(content, ContentItem::InputText { text }
                        if text == &followup_reminder)
                }))
        });
    if already_injected {
        return;
    }

    let reminder: ResponseItem = ContextualUserFragment::into(GuardianFollowupReviewReminder);
    review_session
        .session
        .inject_no_new_turn(vec![reminder], /*current_turn_context*/ None)
        .await;
}

pub(super) async fn prepare_verdict_session(
    review_session: &GuardianReviewSession,
    context: &GuardianReviewContext,
    category: GuardianScope,
    policy: &GuardianNodeReplPolicy,
) -> anyhow::Result<()> {
    review_session
        .session
        .update_settings(crate::session::session::SessionSettingsUpdate {
            step_settings: crate::session::step_settings::StepSettingsUpdate {
                request_country: Some(context.request_country.clone()),
                ..Default::default()
            },
            ..Default::default()
        })
        .await?;
    if !context.turn().model_info().computer_use_review_required()
        || category != GuardianScope::ComputerUse
    {
        return Ok(());
    }
    let policy_body = policy.body().into_text().unwrap_or_default();
    if policy_body.is_empty() {
        return Ok(());
    }
    let already_injected = review_session
        .session
        .clone_history()
        .await
        .raw_items()
        .any(|item| {
            matches!(item, ResponseItem::Message { role, content, .. }
            if role == "developer"
                && content.iter().any(|content| {
                    matches!(content, ContentItem::InputText { text } if text == &policy_body)
                }))
        });
    if already_injected {
        return Ok(());
    }

    let turn_context = review_session.session.new_default_turn().await;
    if review_session
        .session
        .reference_context_item()
        .await
        .is_none()
    {
        let initialize_context: BoxFuture<'_, anyhow::Result<()>> = Box::pin(async {
            let step_context = review_session
                .session
                .capture_step_context(Arc::clone(&turn_context), &review_session.cancel_token)
                .await?;
            review_session
                .session
                .record_context_updates_and_set_reference_context_item(step_context.as_ref())
                .await?;
            Ok(())
        });
        initialize_context.await?;
    }

    let item: ResponseItem = ContextualUserFragment::into(policy.clone());
    review_session
        .session
        .inject_client_response_items(vec![item], turn_context.as_ref())
        .await;

    Ok(())
}

fn finish_budgeted_verdict(
    mut turn: ReviewTurnResult,
    budget: Option<&ExhaustedReviewBudget>,
) -> ReviewTurnResult {
    let Some(budget) = budget else { return turn };
    turn.disposition = SessionDisposition::Discard;
    if matches!(budget, ExhaustedReviewBudget::Detected)
        && matches!(
            turn.outcome,
            GuardianReviewSessionOutcome::SessionFailed {
                error_info: Some(CodexErrorInfo::ContextWindowExceeded),
                ..
            }
        )
    {
        turn.outcome = GuardianReviewSessionOutcome::InputBudgetExceeded;
    }
    turn
}

/// Input for one verdict, with the evidence that replaces the prompt marker at admission.
pub(super) struct PreparedVerdictTurn {
    pub(super) request: TurnInputRequest,
    pub(super) evidence: PendingReviewContext,
    pub(super) node_repl_evidence_sequence: Option<u64>,
}

impl GuardianReviewSession {
    /// Returns the turn result and whether prompt finalization consumed its evidence.
    pub(super) async fn run_sync_verdict(
        &self,
        context: &GuardianReviewContext,
        turn: PreparedVerdictTurn,
        deadline: ReviewDeadline,
        cancel: Option<&CancellationToken>,
        analytics: &mut GuardianReviewAnalyticsResult,
    ) -> Result<(ReviewTurnResult, bool), GuardianReviewSessionOutcome> {
        let store = &self.session.services.thread_extension_data;
        if self
            .reuse_key
            .features
            .enabled(Feature::GuardianDenialReconsideration)
        {
            store.insert(codex_guardian_reviewer::DenialResample::default());
        }
        store.insert(turn.evidence);
        store.insert(super::PendingReviewRouting(context.parent_response.clone()));
        let mut observation =
            crate::turn_timing::review_execution::ReviewExecutionObservation::install(
                Arc::clone(&self.session),
                context.review_execution.clone(),
            );
        let result = async {
            let id =
                codex_guardian_reviewer::start_review_turn(self, turn.request, deadline, cancel)
                    .await?;
            if let Some(response_sequence) = turn.node_repl_evidence_sequence {
                self.state.lock().await.pending_node_repl_evidence_admission =
                    Some(PendingNodeReplEvidenceAdmission {
                        turn_id: id.clone(),
                        response_sequence,
                    });
            }
            let result = wait_for_guardian_review(self, &id, deadline, cancel, analytics).await;
            observation.finish(result.turn_completed);
            let budget = store.remove::<ExhaustedReviewBudget>();
            Ok(finish_budgeted_verdict(result, budget.as_deref()))
        }
        .await;
        store.remove::<super::PendingReviewRouting>();
        let admitted = store.remove::<PendingReviewContext>().is_none();
        let resample = store.remove::<codex_guardian_reviewer::DenialResample>();
        result.map(|turn| {
            let turn = match resample {
                Some(resample) => resample.finish(turn, &context.turn().session_telemetry),
                None => turn,
            };
            (turn, admitted)
        })
    }

    pub(super) fn analytics(
        &self,
        review_model: &ReviewModel,
        default_effort: Option<&ReasoningEffortConfig>,
        provider_id: &str,
        guardian_session_kind: GuardianReviewSessionKind,
        had_prior_context: bool,
    ) -> GuardianReviewAnalyticsResult {
        GuardianReviewAnalyticsResult::from_session(GuardianReviewSessionAnalyticsParams {
            guardian_thread_id: self.session.thread_id().to_string(),
            guardian_session_kind,
            guardian_model: review_model.model.clone(),
            guardian_reasoning_effort: review_model
                .reasoning_effort
                .as_ref()
                .or(default_effort)
                .map(ToString::to_string),
            guardian_default_review_model_id: review_model.default_review_model_id.clone(),
            guardian_catalog_contains_auto_review: review_model.catalog_contains_auto_review,
            guardian_review_model_overridden: review_model.model_overridden,
            guardian_review_model_override: review_model.model_override.clone(),
            guardian_model_provider_id: provider_id.to_owned(),
            had_prior_review_context: had_prior_context,
        })
    }

    pub(super) async fn token_usage_baseline(&self) -> TokenUsage {
        self.session.total_token_usage().await.unwrap_or_default()
    }

    /// Count only usage reported on the verdict thread since this attempt started.
    /// A failed attempt may still have completed one or more model requests.
    pub(super) async fn record_verdict_usage(
        &self,
        start: &TokenUsage,
        analytics: &mut GuardianReviewAnalyticsResult,
    ) {
        if let Some(end) = self.session.total_token_usage().await {
            analytics.token_usage = Some(token_usage_delta(start, &end));
        }
    }
}

impl codex_guardian_reviewer::ReviewerRuntime for GuardianReviewSession {
    async fn submit_turn(&self, request: TurnInputRequest) -> anyhow::Result<TurnInputSubmission> {
        Ok(self
            .io
            .submit_turn_input(
                crate::codex_thread::resolve_client_turn_input(request).await?,
                TurnInputMode::StartIfIdle,
            )
            .await?)
    }

    async fn next_event(&self) -> anyhow::Result<Event> {
        Ok(self.io.next_event().await?)
    }

    async fn admit_context(&self, event: &Event) {
        codex_guardian_reviewer::admit_node_repl_evidence(&self.state, event).await;
    }

    async fn interrupt(&self) -> anyhow::Result<()> {
        self.io.submit(Op::Interrupt).await?;
        Ok(())
    }

    fn retry_at(&self, turn_id: &str) -> Option<tokio::time::Instant> {
        self.session
            .services
            .thread_extension_data
            .get::<crate::responses_retry::ExhaustedResponseRetry>()
            .filter(|advice| advice.turn_id == turn_id)
            .and_then(|advice| advice.retry_at)
    }
}

#[cfg(test)]
#[path = "review_session_tests.rs"]
mod tests;

impl codex_guardian_reviewer::ReviewerSession for GuardianReviewSession {
    type Setup = PreparedGuardianContext;
    type Context = GuardianReviewSessionReuseKey;
    type Snapshot = GuardianReviewForkSnapshot;

    fn context(&self) -> &Self::Context {
        &self.reuse_key
    }
    async fn snapshot(&self) -> Option<GuardianReviewForkSnapshot> {
        self.state.lock().await.conversation.snapshot().cloned()
    }
}

impl GuardianReviewSession {
    pub(crate) async fn rollout_path(&self) -> Option<PathBuf> {
        self.session
            .ensure_rollout_materialized(PersistContext::Standard)
            .await;
        match self.session.current_rollout_path().await {
            Ok(path) => path,
            Err(error) => {
                warn!("failed to resolve guardian trunk rollout path: {error}");
                None
            }
        }
    }
}

#[cfg(test)]
impl GuardianReviewSession {
    pub(crate) async fn committed_fork_rollout_items_for_test(&self) -> Option<Vec<RolloutItem>> {
        let state = self.state.lock().await;
        let snapshot = state.conversation.snapshot()?;
        match &snapshot.history().initial_history {
            InitialHistory::Forked(items) => Some(items.clone()),
            InitialHistory::New | InitialHistory::Cleared | InitialHistory::Resumed(_) => None,
        }
    }

    pub(crate) async fn send_trunk_event_raw_for_test(&self, event: Event) {
        self.session.send_event_raw(event).await;
    }
}
