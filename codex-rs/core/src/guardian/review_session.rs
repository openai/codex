//! Host adapter for synchronous Guardian sessions and the existing context builder.
//! The extension owns review policy and pooling; this module binds runtime operations
//! to the captured parent action, environments, authorization and context snapshots.

#[path = "review_session_factory.rs"]
mod factory;
pub(crate) use factory::prewarm_guardian_review_session;
pub(crate) use factory::run_guardian_review_session;

#[path = "review_session_threads.rs"]
mod managed_threads;

#[path = "review_session_context.rs"]
mod context_policy;
use context_policy::ReviewContextPolicy;

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;

use codex_analytics::GuardianReviewAnalyticsResult;
use codex_analytics::GuardianReviewSessionAnalyticsParams;
use codex_analytics::GuardianReviewSessionKind;
use codex_extension_api::Instructions;
use codex_history::InitialHistory;
use codex_history::RolloutItem;
use codex_protocol::ThreadId;
use codex_protocol::config_types::AutoCompactTokenLimitScope;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::items::TurnItem;
use codex_protocol::mcp::is_node_repl_backed_server;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;
use futures::future::BoxFuture;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::codex_delegate::run_codex_thread_interactive;
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
use crate::image_preparation::ImagePreparationMode;
use crate::image_preparation::ImageResizeNoticeMode;
use crate::image_preparation::prepare_response_items;
use crate::image_preparation::unified_image_budget_enabled;
use crate::session::GitEnrichmentPolicy;
use crate::session::SessionIo;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use codex_config::types::McpServerConfig;
use codex_features::Feature;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::turn_input::TurnInputMode;
use codex_protocol::turn_input::TurnInputRequest;
use codex_protocol::turn_input::TurnInputSubmission;
use codex_protocol::user_input::UserInput;
use codex_thread_store::PersistContext;
use codex_tools::normalize_output_image_detail;
use codex_utils_path_uri::PathUri;

use super::ApprovalRequestReasons;
use super::GUARDIAN_REVIEWER_NAME;
use super::GuardianApprovalRequest;
use super::GuardianReviewContext;
use super::feedback::record_failed_review;
#[cfg(test)]
use super::prompt::BUNDLED_GUARDIAN_POLICY;
use super::prompt::GUARDIAN_TRANSCRIPT_START;
use super::prompt::GuardianPromptMode;
use super::prompt::GuardianTranscriptCursor;
use super::prompt::build_guardian_prompt_items_with_parent_turn;
use super::review::guardian_review_session_config;
pub(crate) use super::reviewer_config::build_guardian_review_session_config;
use codex_guardian_reviewer::run_before_review_deadline;
#[cfg(test)]
use codex_guardian_reviewer::run_before_review_deadline_with_cancel;
use codex_guardian_reviewer::wait_for_guardian_review;

const GUARDIAN_MAX_IMAGE_ITEM_TOKENS: i64 = 10_000;
pub(crate) use codex_guardian_reviewer::GuardianReviewSessionOutcome;

pub(crate) struct GuardianReviewSessionParams {
    pub(crate) parent_session: Arc<Session>,
    pub(crate) parent_context: GuardianReviewContext,
    // Checkpoint selection and thread-owned prompt evidence must use the same history.
    pub(crate) parent_history: ContextManager,
    pub(crate) spawn_config: Config,
    pub(crate) node_repl_policy: GuardianNodeReplPolicy,
    pub(crate) request: GuardianApprovalRequest,
    pub(crate) reasons: ApprovalRequestReasons,
    pub(crate) schema: Value,
    pub(crate) model: String,
    pub(crate) compaction_model_hash: Option<String>,
    pub(crate) reasoning_effort: Option<ReasoningEffortConfig>,
    pub(crate) guardian_default_review_model_id: String,
    pub(crate) guardian_catalog_contains_auto_review: bool,
    pub(crate) guardian_review_model_overridden: bool,
    pub(crate) guardian_review_model_override: Option<String>,
    pub(crate) reasoning_summary: ReasoningSummaryConfig,
    pub(crate) personality: Option<Personality>,
    pub(crate) external_cancel: Option<CancellationToken>,
    pub(crate) deadline: tokio::time::Instant,
}

/// Host capability used to spawn private reviewer runtimes for this parent.
/// The extension owns pooling; this adapter keeps the existing context and runtime paths.
#[derive(Default)]
pub struct GuardianReviewSessionHost {
    managed_threads: Option<managed_threads::ManagedReviewerThreads>,
}

impl GuardianReviewSessionHost {
    pub fn with_thread_manager(manager: std::sync::Weak<crate::ThreadManager>) -> Self {
        Self {
            managed_threads: Some(managed_threads::ManagedReviewerThreads::new(manager)),
        }
    }

    pub fn mark_ready(&self) {
        if let Some(threads) = &self.managed_threads {
            threads.mark_ready();
        }
    }
}

pub(crate) type GuardianReviewSessionManager =
    codex_guardian_reviewer::ReviewerPool<GuardianReviewSession>;

/// Opaque host session handle. Its state belongs to the existing context builder.
pub(crate) struct GuardianReviewSession {
    session: Arc<Session>,
    io: SessionIo,
    cancel_token: CancellationToken,
    reuse_key: GuardianReviewSessionReuseKey,
    state: Mutex<GuardianReviewState>,
}

struct GuardianReviewState {
    prior_review_count: usize,
    last_reviewed_transcript_cursor: Option<GuardianTranscriptCursor>,
    last_admitted_node_repl_response_sequence: u64,
    pending_node_repl_evidence_admission: Option<PendingNodeReplEvidenceAdmission>,
    last_committed_fork_snapshot: Option<GuardianReviewForkSnapshot>,
}

struct PendingNodeReplEvidenceAdmission {
    turn_id: String,
    response_sequence: u64,
}

fn had_prior_review_context(prompt_mode: &GuardianPromptMode) -> bool {
    matches!(prompt_mode, GuardianPromptMode::Delta { .. })
}

fn token_usage_delta(start: &TokenUsage, end: &TokenUsage) -> TokenUsage {
    TokenUsage {
        input_tokens: (end.input_tokens - start.input_tokens).max(0),
        cached_input_tokens: (end.cached_input_tokens - start.cached_input_tokens).max(0),
        cache_write_input_tokens: (end.cache_write_input_tokens - start.cache_write_input_tokens)
            .max(0),
        output_tokens: (end.output_tokens - start.output_tokens).max(0),
        reasoning_output_tokens: (end.reasoning_output_tokens - start.reasoning_output_tokens)
            .max(0),
        total_tokens: (end.total_tokens - start.total_tokens).max(0),
        codex_rollout_budget_units: None,
    }
}

/// Committed context used to seed a private reviewer fork.
#[derive(Clone)]
pub struct GuardianReviewForkSnapshot {
    initial_history: InitialHistory,
    prior_review_count: usize,
    last_reviewed_transcript_cursor: Option<GuardianTranscriptCursor>,
    last_admitted_node_repl_response_sequence: u64,
}

/// Opaque compatibility key derived by the existing context builder.
#[derive(Debug, Clone, PartialEq)]
pub struct GuardianReviewSessionReuseKey {
    // Only include settings that affect spawned-session behavior and parent
    // history rewrites that invalidate existing reviewer context.
    parent_history_version: u64,
    root_authorization_version: Option<crate::codex_thread::GuardianAuthorizationVersion>,
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
    personality: Option<Personality>,
    permissions: Permissions,
    developer_instructions: Option<String>,
    base_instructions: Option<String>,
    user_instructions: Option<Instructions>,
    compact_prompt: Option<String>,
    cwd: PathUri,
    mcp_servers: Constrained<HashMap<String, McpServerConfig>>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    main_execve_wrapper_exe: Option<PathBuf>,
    zsh_path: Option<PathBuf>,
    features: ManagedFeatures,
    environment_ids: Vec<String>,
}

impl GuardianReviewSessionReuseKey {
    fn from_spawn_config(
        spawn_config: &Config,
        user_instructions: Option<Instructions>,
        parent_history_version: u64,
        context_mode: GuardianContextMode,
    ) -> Self {
        Self {
            root_authorization_version: None,
            parent_history_version: match ReviewContextPolicy::for_context(
                context_mode,
                &spawn_config.features,
            ) {
                ReviewContextPolicy::Legacy => 0,
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
            personality: spawn_config.personality,
            permissions: spawn_config.permissions.clone(),
            developer_instructions: spawn_config.developer_instructions.clone(),
            base_instructions: spawn_config.base_instructions.clone(),
            user_instructions,
            compact_prompt: spawn_config.compact_prompt.clone(),
            cwd: PathUri::from_abs_path(&spawn_config.cwd),
            mcp_servers: spawn_config.mcp_servers.clone(),
            codex_linux_sandbox_exe: spawn_config.codex_linux_sandbox_exe.clone(),
            main_execve_wrapper_exe: spawn_config.main_execve_wrapper_exe.clone(),
            zsh_path: spawn_config.zsh_path.clone(),
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
        self.node_repl_policy = policy.body();
        self
    }
}

pub(crate) fn prompt_cache_key_override_for_review_session(
    session_source: &SessionSource,
    parent_thread_id: Option<ThreadId>,
) -> Option<String> {
    let SessionSource::SubAgent(SubAgentSource::Other(name)) = session_source else {
        return None;
    };
    if name != GUARDIAN_REVIEWER_NAME {
        return None;
    }
    let parent_thread_id = parent_thread_id?;
    Some(format!("guardian:{parent_thread_id}"))
}

impl GuardianReviewSession {
    async fn admit_node_repl_evidence(&self, event: &Event) {
        let EventMsg::ItemCompleted(completed) = &event.msg else {
            return;
        };
        let TurnItem::UserMessage(_) = &completed.item else {
            return;
        };

        let mut state = self.state.lock().await;
        let Some(pending) = state.pending_node_repl_evidence_admission.as_ref() else {
            return;
        };
        if completed.thread_id == self.session.thread_id()
            && event.id == pending.turn_id
            && completed.turn_id == pending.turn_id
        {
            state.last_admitted_node_repl_response_sequence = state
                .last_admitted_node_repl_response_sequence
                .max(pending.response_sequence);
            state.pending_node_repl_evidence_admission = None;
        }
    }
}

async fn run_review_on_session(
    review_session: &GuardianReviewSession,
    params: &GuardianReviewSessionParams,
    guardian_session_kind: GuardianReviewSessionKind,
    deadline: tokio::time::Instant,
) -> (
    GuardianReviewSessionOutcome,
    bool,
    GuardianReviewAnalyticsResult,
) {
    let model_info = params
        .parent_session
        .services
        .models_manager
        .get_model_info(
            params.model.as_str(),
            &params.spawn_config.to_models_manager_config(),
        )
        .await;
    let guardian_reasoning_effort = params
        .reasoning_effort
        .clone()
        .or_else(|| model_info.default_reasoning_level.clone());
    let (prior_review_count, had_prior_context) = {
        let state = review_session.state.lock().await;
        (
            state.prior_review_count,
            state.last_reviewed_transcript_cursor.is_some(),
        )
    };
    let mut analytics_result =
        GuardianReviewAnalyticsResult::from_session(GuardianReviewSessionAnalyticsParams {
            guardian_thread_id: review_session.session.thread_id().to_string(),
            guardian_session_kind,
            guardian_model: params.model.clone(),
            guardian_reasoning_effort: guardian_reasoning_effort.map(|effort| effort.to_string()),
            guardian_default_review_model_id: params.guardian_default_review_model_id.clone(),
            guardian_catalog_contains_auto_review: params.guardian_catalog_contains_auto_review,
            guardian_review_model_overridden: params.guardian_review_model_overridden,
            guardian_review_model_override: params.guardian_review_model_override.clone(),
            guardian_model_provider_id: params.spawn_config.model_provider_id.clone(),
            had_prior_review_context: had_prior_context,
        });
    if prior_review_count > 0 {
        ensure_guardian_followup_reminder(review_session).await;
    }

    match run_before_review_deadline(
        deadline,
        params.external_cancel.as_ref(),
        Box::pin(ensure_guardian_node_repl_policy(review_session, params)),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            return (
                GuardianReviewSessionOutcome::SessionFailed {
                    error,
                    error_info: None,
                    retry_at: None,
                },
                false,
                analytics_result,
            );
        }
        Err(outcome) => return (outcome, false, analytics_result),
    }

    if params.spawn_config.features.enabled(Feature::TokenBudget)
        && crate::session::context_window::context_window_token_status_for_model(
            review_session.session.as_ref(),
            &params.spawn_config,
            params.parent_context.turn(),
            &model_info,
        )
        .await
        .token_limit_reached
    {
        let compact_submission = run_before_review_deadline(
            deadline,
            params.external_cancel.as_ref(),
            review_session.io.submit(Op::Compact),
        )
        .await;
        let compact_turn_id = match compact_submission {
            Ok(Ok(turn_id)) => turn_id,
            Ok(Err(error)) => {
                return (
                    GuardianReviewSessionOutcome::SessionFailed {
                        error: error.into(),
                        error_info: None,
                        retry_at: None,
                    },
                    false,
                    analytics_result,
                );
            }
            Err(outcome) => return (outcome, false, analytics_result),
        };
        let (outcome, keep_review_session, _) = wait_for_guardian_review(
            review_session,
            &compact_turn_id,
            deadline,
            params.external_cancel.as_ref(),
            &mut analytics_result,
        )
        .await;
        if !matches!(outcome, GuardianReviewSessionOutcome::Completed(Ok(_))) {
            return (outcome, keep_review_session, analytics_result);
        }

        if prior_review_count > 0 {
            ensure_guardian_followup_reminder(review_session).await;
        }
    }

    let reviewer_has_full_transcript = review_session
        .session
        .clone_history()
        .await
        .raw_items()
        .any(|item| {
            matches!(item, ResponseItem::Message { role, content, .. }
            if role == "user" && content.iter().any(|content| {
                matches!(content, ContentItem::InputText { text }
                    if text == GUARDIAN_TRANSCRIPT_START)
            }))
        });
    let (prompt_mode, last_admitted_node_repl_response_sequence) = {
        let mut state = review_session.state.lock().await;
        state.pending_node_repl_evidence_admission = None;
        if !reviewer_has_full_transcript {
            state.last_reviewed_transcript_cursor = None;
            state.last_admitted_node_repl_response_sequence = 0;
        }

        let prompt_mode = state
            .last_reviewed_transcript_cursor
            .map_or(GuardianPromptMode::Full, |cursor| {
                GuardianPromptMode::Delta { cursor }
            });
        (prompt_mode, state.last_admitted_node_repl_response_sequence)
    };
    analytics_result.had_prior_review_context = Some(had_prior_review_context(&prompt_mode));

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

            let history = if params.parent_session.guardian_context_mode
                == GuardianContextMode::ThreadOwned
            {
                params.parent_history.conversation_history_snapshot()
            } else {
                params.parent_session.conversation_history_snapshot().await
            };
            let mut prompt_items = build_guardian_prompt_items_with_parent_turn(
                params.parent_session.as_ref(),
                history.as_ref(),
                Some(&params.parent_context),
                params.reasons.clone(),
                params.request.clone(),
                prompt_mode,
                last_admitted_node_repl_response_sequence,
            )
            .await?;

            if prompt_items
                .context
                .section_costs()
                .any(|(_, cost)| cost.image_count > 0)
            {
                let reviewer_history = review_session.session.clone_history().await;
                let reviewer_image_urls = reviewer_history
                    .raw_items()
                    .flat_map(|item| match item {
                        ResponseItem::Message { content, .. } => content.as_slice(),
                        _ => &[],
                    })
                    .filter_map(|item| match item {
                        ContentItem::InputImage { image_url, .. } => Some(image_url.as_str()),
                        _ => None,
                    })
                    .collect::<HashSet<_>>();
                let context_window = model_info.resolved_context_window().map(|supported| {
                    params
                        .spawn_config
                        .model_context_window
                        .unwrap_or(supported)
                        .min(supported)
                        .saturating_mul(model_info.effective_context_window_percent.clamp(0, 100))
                        / 100
                });
                let admit_images = if let Some(context_window) = context_window.filter(|limit| {
                    *limit > 0
                        && !model_info.used_fallback_model_metadata
                        && model_info.input_modalities.contains(&InputModality::Image)
                }) {
                    let features = &params.spawn_config.features;
                    let mode = if unified_image_budget_enabled(features, &model_info) {
                        ImagePreparationMode::UnifiedBudget
                    } else {
                        ImagePreparationMode::DetailBased
                    };
                    prompt_items.context.retain_images(|image_url, detail| {
                        *detail = match normalize_output_image_detail(&model_info, *detail) {
                            _ if mode == ImagePreparationMode::UnifiedBudget => {
                                Some(ImageDetail::Original)
                            }
                            Some(ImageDetail::Low) => Some(ImageDetail::High),
                            detail => detail,
                        };
                        let mut prepared = vec![
                            ResponseInputItem::from(vec![UserInput::Image {
                                image_url: image_url.to_owned(),
                                detail: *detail,
                            }])
                            .into(),
                        ];
                        prepare_response_items(
                            &mut prepared,
                            mode,
                            ImageResizeNoticeMode::Disabled,
                        );
                        let Some(ResponseItem::Message { content, .. }) = prepared.first() else {
                            return false;
                        };
                        content.iter().any(|item| {
                            matches!(item, ContentItem::InputImage { image_url, .. }
                                if !reviewer_image_urls.contains(image_url.as_str()))
                        })
                    });
                    let prompt: ResponseItem =
                        ResponseInputItem::from(prompt_items.context.clone().into_user_inputs()?)
                            .into();
                    let prompt_tokens = crate::context_manager::estimate_item_token_count(&prompt);
                    let base_instructions = review_session.session.get_base_instructions().await;
                    let history_tokens = reviewer_history
                        .estimate_token_count_with_base_instructions(&base_instructions)
                        .unwrap_or(i64::MAX)
                        .max(review_session.session.get_total_token_usage().await);
                    prompt_tokens <= GUARDIAN_MAX_IMAGE_ITEM_TOKENS
                        && prompt_tokens.saturating_add(history_tokens) <= context_window
                } else {
                    false
                };
                if !admit_images {
                    prompt_items.context.retain_images(|_, _| false);
                }
            }

            let items = prompt_items.context.clone().into_user_inputs()?;
            Ok::<_, anyhow::Error>((prompt_items, items))
        }),
    )
    .await;
    let prompt_items = match prompt_items {
        Ok(prompt_items) => prompt_items,
        Err(outcome) => return (outcome, false, analytics_result),
    };
    let (prompt_items, items) = match prompt_items {
        Ok(prompt_items) => prompt_items,
        Err(err) => {
            return (
                GuardianReviewSessionOutcome::PromptBuildFailed(err),
                false,
                analytics_result,
            );
        }
    };
    let transcript_cursor = prompt_items.transcript_cursor;
    let node_repl_evidence_admission = (prompt_items.node_repl_evidence_sequence
        > last_admitted_node_repl_response_sequence)
        .then_some(prompt_items.node_repl_evidence_sequence);
    let token_usage_at_review_start = review_session
        .session
        .total_token_usage()
        .await
        .unwrap_or_default();
    let parent_turn_environments = params
        .parent_context
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
    let parent_turn_legacy_fallback_cwd = params
        .parent_context
        .environments()
        .primary()
        .and_then(|environment| environment.cwd().to_abs_path().ok())
        .unwrap_or_else(|| params.parent_context.turn().config.cwd.clone());

    let parent_turn = params.parent_context.turn();
    review_session
        .session
        .services
        .thread_extension_data
        .insert(super::input_budget::PendingReviewContext(
            prompt_items.context,
        ));
    let request = codex_guardian_reviewer::ReviewerTurn {
        items,
        environments: codex_protocol::protocol::TurnEnvironmentSelections::new(
            parent_turn_legacy_fallback_cwd,
            parent_turn_environments,
        ),
        permission_profile: params.spawn_config.permissions.permission_profile().clone(),
        reasoning_summary: params.reasoning_summary,
        personality: params.personality,
        model: params.model.clone(),
        reasoning_effort: params.reasoning_effort.clone(),
        parent_response_id: params.parent_context.parent_response_id.clone(),
        schema: params.schema.clone(),
        parent_turn_id: parent_turn.sub_id.clone(),
        root_turn_id: parent_turn.turn_metadata_state.root_turn_id(),
    }
    .into_request();
    let child_turn_id = match codex_guardian_reviewer::start_review_turn(
        review_session,
        request,
        deadline,
        params.external_cancel.as_ref(),
    )
    .await
    {
        Ok(turn_id) => turn_id,
        Err(outcome) => {
            review_session
                .session
                .services
                .thread_extension_data
                .remove::<super::input_budget::PendingReviewContext>();
            return (outcome, false, analytics_result);
        }
    };
    if let Some(response_sequence) = node_repl_evidence_admission {
        let mut state = review_session.state.lock().await;
        state.pending_node_repl_evidence_admission = Some(PendingNodeReplEvidenceAdmission {
            turn_id: child_turn_id.clone(),
            response_sequence,
        });
    }

    let outcome = wait_for_guardian_review(
        review_session,
        child_turn_id.as_str(),
        deadline,
        params.external_cancel.as_ref(),
        &mut analytics_result,
    )
    .await;
    review_session
        .session
        .services
        .thread_extension_data
        .remove::<super::input_budget::PendingReviewContext>();
    if matches!(outcome.0, GuardianReviewSessionOutcome::Completed(_)) {
        if outcome.2
            && let Some(total_token_usage) = review_session.session.total_token_usage().await
        {
            analytics_result.token_usage = Some(token_usage_delta(
                &token_usage_at_review_start,
                &total_token_usage,
            ));
        }
        let mut state = review_session.state.lock().await;
        state.prior_review_count = state.prior_review_count.saturating_add(1);
        state.last_reviewed_transcript_cursor = Some(transcript_cursor);
    }
    let budget_exhausted = review_session
        .session
        .services
        .thread_extension_data
        .remove::<super::request_budget::ExhaustedReviewBudget>();
    let result = match outcome.0 {
        GuardianReviewSessionOutcome::SessionFailed {
            error_info: Some(CodexErrorInfo::ContextWindowExceeded),
            ..
        } if matches!(
            budget_exhausted.as_deref(),
            Some(super::request_budget::ExhaustedReviewBudget::Detected)
        ) =>
        {
            GuardianReviewSessionOutcome::InputBudgetExceeded
        }
        result => result,
    };
    (
        result,
        outcome.1 && budget_exhausted.is_none(),
        analytics_result,
    )
}

async fn ensure_guardian_followup_reminder(review_session: &GuardianReviewSession) {
    let followup_reminder = GuardianFollowupReviewReminder.body();
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

async fn ensure_guardian_node_repl_policy(
    review_session: &GuardianReviewSession,
    params: &GuardianReviewSessionParams,
) -> anyhow::Result<()> {
    if !params
        .parent_context
        .turn()
        .model_info()
        .computer_use_review_required()
        || !matches!(
            &params.request,
            GuardianApprovalRequest::McpToolCall { server, tool_name, .. }
                if is_node_repl_backed_server(server) && tool_name == "js"
        )
    {
        return Ok(());
    }

    let policy = &params.node_repl_policy;
    let policy_body = policy.body();
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

async fn load_rollout_items_for_fork(
    session: &Session,
) -> anyhow::Result<Option<Vec<RolloutItem>>> {
    session
        .try_ensure_rollout_materialized(PersistContext::Standard)
        .await?;
    session.flush_rollout().await?;
    let live_thread = session.live_thread_for_persistence("guardian review fork")?;
    let history = live_thread.load_history(/*include_archived*/ true).await?;
    Ok(Some(history.items))
}

impl codex_guardian_reviewer::ReviewerRuntime for GuardianReviewSession {
    async fn submit_turn(&self, request: TurnInputRequest) -> anyhow::Result<TurnInputSubmission> {
        Ok(self
            .io
            .submit_turn_input(request, TurnInputMode::StartIfIdle)
            .await?)
    }

    async fn next_event(&self) -> anyhow::Result<Event> {
        Ok(self.io.next_event().await?)
    }

    async fn admit_context(&self, event: &Event) {
        self.admit_node_repl_evidence(event).await;
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
    type Context = GuardianReviewSessionReuseKey;
    type Snapshot = GuardianReviewForkSnapshot;

    fn context(&self) -> &Self::Context {
        &self.reuse_key
    }
    fn cancel(&self) {
        self.cancel_token.cancel();
    }

    async fn shutdown(&self) {
        self.cancel_token.cancel();
        let _ = self.io.shutdown_and_wait().await;
    }

    async fn snapshot(&self) -> Option<GuardianReviewForkSnapshot> {
        self.state.lock().await.last_committed_fork_snapshot.clone()
    }

    async fn commit_snapshot(&self) {
        match load_rollout_items_for_fork(&self.session).await {
            Ok(Some(items)) if !items.is_empty() => {
                let mut state = self.state.lock().await;
                let prior_review_count = state.prior_review_count;
                let last_reviewed_transcript_cursor = state.last_reviewed_transcript_cursor;
                let last_admitted_node_repl_response_sequence =
                    state.last_admitted_node_repl_response_sequence;
                state.last_committed_fork_snapshot = Some(GuardianReviewForkSnapshot {
                    initial_history: InitialHistory::Forked(items),
                    prior_review_count,
                    last_reviewed_transcript_cursor,
                    last_admitted_node_repl_response_sequence,
                });
            }
            Ok(Some(_)) => {}
            Ok(None) => {}
            Err(err) => {
                warn!("failed to refresh guardian trunk rollout snapshot: {err}");
            }
        }
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
        let snapshot = state.last_committed_fork_snapshot.as_ref()?;
        match &snapshot.initial_history {
            InitialHistory::Forked(items) => Some(items.clone()),
            InitialHistory::New | InitialHistory::Cleared | InitialHistory::Resumed(_) => None,
        }
    }

    pub(crate) async fn send_trunk_event_raw_for_test(&self, event: Event) {
        self.session.send_event_raw(event).await;
    }
}
