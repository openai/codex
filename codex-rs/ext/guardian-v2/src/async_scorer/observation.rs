//! Selects tool observations for classification and captures their evidence before spawning.
//! Keep snapshots on their existing side of the background task boundary.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;
use std::time::SystemTime;

use codex_analytics::AnalyticsEventsClient;
use codex_core::context::GuardianContextMode;
use codex_core::context::GuardianReviewEvidence;
use codex_core::context::NodeReplReviewEvidence;
use codex_extension_api::ExtensionWarning;
use codex_extension_api::GuardianV2Enabled;
use codex_extension_api::ToolCallSource;
use codex_extension_api::ToolPayload;
use codex_extension_api::ToolStartInput;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::mcp::is_node_repl_backed_server;
use codex_protocol::openai_models::GuardianReviewMode;
use codex_protocol::openai_models::GuardianScope;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::security_risk::SecurityRiskScore;

use super::action::ActionRenderError;
use super::action::GuardianAction;
use super::authorization::ScoreAuthorization;
use super::classification::Classification;
use super::config::GuardianV2Config;
use super::coverage::scores_tool;
use super::extension::GuardianV2Extension;
use super::metrics::record_classification;
use super::parent_compaction::ParentCompactionError;
use super::parent_compaction::select_parent_compaction;
use super::sampler::LunaSampler;
use super::score::GuardianV2ScoreProgress;
use super::score::record_fail_closed_score;
use codex_protocol::openai_models::GuardianUnscoredAction as UnscoredAction;

impl GuardianV2Extension {
    pub(super) async fn score_tool(&self, input: ToolStartInput<'_>) {
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
        // Model policies review nested actions; the Code Mode wrapper leaves their scores alone.
        // The legacy all-tools policy still scores wrappers through `other_tools`.
        if scope.is_none()
            && input.tool_name.is_default_namespace()
            && input.tool_name.name == "exec"
            && matches!(input.payload, ToolPayload::Custom { .. })
            && matches!(input.source, ToolCallSource::Direct)
            && policy.other_tools == GuardianReviewMode::Disabled
        {
            return;
        }
        if !scores_tool(&policy, input.tool_name, input.payload, scope) {
            match policy.unscored_action {
                UnscoredAction::Ignore => {}
                UnscoredAction::AgeScore => {
                    let index = score_progress
                        .latest_tool_call
                        .fetch_add(/*val*/ 1, Ordering::Relaxed)
                        .saturating_add(/*rhs*/ 1);
                    score_progress.wrapper_lag.record(&input, index);
                    // Unscored permission widening must not reuse an earlier approval score.
                    if input.tool_name.is_default_namespace()
                        && input.tool_name.name == "exec_command"
                        && let ToolPayload::Function { arguments } = input.payload
                        && let Ok(arguments) = serde_json::from_str::<serde_json::Value>(arguments)
                        && arguments
                            .get("sandbox_permissions")
                            .and_then(serde_json::Value::as_str)
                            == Some("with_additional_permissions")
                    {
                        score_progress
                            .latest_failed_tool_call
                            .fetch_max(index, Ordering::Release);
                    }
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
        if !(scope == Some(GuardianScope::ComputerUse) && policy.allows_initial_cua_call())
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
                record_fail_closed_score(input.thread_store, sampled_at);
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
        let context_mode = GuardianContextMode::from_history(input.conversation_history.as_ref());
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
                record_fail_closed_score(input.thread_store, sampled_at);
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
        let planned_action = match action.render(guardian_config.max_action_tokens) {
            Ok(text) => text,
            Err(ActionRenderError::TooLarge { .. }) => {
                score_progress
                    .oversized_tool_calls
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(input.call_id.to_owned());
                score_progress
                    .latest_failed_tool_call
                    .fetch_max(tool_call_index, Ordering::Release);
                record_fail_closed_score(input.thread_store, sampled_at);
                record_classification(
                    metrics.as_deref(),
                    classification_started_at.elapsed(),
                    "failure",
                    Some("input_too_large"),
                );
                return;
            }
            Err(error) => {
                score_progress
                    .latest_failed_tool_call
                    .fetch_max(tool_call_index, Ordering::Release);
                record_fail_closed_score(input.thread_store, sampled_at);
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
        let classification = Classification {
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
        };
        tokio::spawn(classification.run());
    }
}
