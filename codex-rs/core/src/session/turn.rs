use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::SkillInjections;
use crate::build_skill_injections;
use crate::client::ModelClientSession;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::collect_explicit_skill_mentions;
use crate::compact::InitialContextInjection;
use crate::compact::run_inline_auto_compact_task;
use crate::compact::should_use_remote_compact_task;
use crate::compact_remote::run_inline_remote_auto_compact_task;
use crate::compact_remote_v2::run_inline_remote_auto_compact_task as run_inline_remote_auto_compact_task_v2;
use crate::connectors;
use crate::context::ContextualUserFragment;
use crate::feedback_tags;
use crate::goals::GoalRuntimeEvent;
use crate::hook_runtime::inspect_pending_input;
use crate::hook_runtime::record_additional_contexts;
use crate::hook_runtime::record_pending_input;
use crate::hook_runtime::run_legacy_after_agent_hook;
use crate::hook_runtime::run_pending_session_start_hooks;
use crate::hook_runtime::run_turn_stop_hooks;
use crate::injection::ToolMentionKind;
use crate::injection::app_id_from_path;
use crate::injection::tool_kind_for_path;
use crate::mcp_skill_dependencies::maybe_prompt_and_install_mcp_dependencies;
use crate::mcp_tool_exposure::build_mcp_tool_exposure;
use crate::mentions::build_connector_slug_counts;
use crate::mentions::build_skill_name_counts;
use crate::mentions::collect_explicit_app_ids;
use crate::mentions::collect_explicit_plugin_mentions;
use crate::mentions::collect_tool_mentions_from_messages;
use crate::plugins::build_plugin_injections;
use crate::responses_retry::ResponsesStreamRequest;
use crate::responses_retry::handle_retryable_response_stream_error;
use crate::session::PreviousTurnSettings;
use crate::session::TurnInput;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::stream_events_utils::HandleOutputCtx;
use crate::stream_events_utils::TurnItemContributorPolicy;
use crate::stream_events_utils::finalize_non_tool_response_item;
use crate::stream_events_utils::handle_non_tool_response_item;
use crate::stream_events_utils::handle_output_item_done;
use crate::stream_events_utils::last_assistant_message_from_item;
use crate::stream_events_utils::mark_thread_memory_mode_polluted_if_external_context;
use crate::stream_events_utils::raw_assistant_output_text_from_item;
use crate::stream_events_utils::record_completed_response_item_with_finalized_facts;
use crate::tasks::emit_compact_metric;
use crate::tools::ToolRouter;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::parallel::ToolCallRuntime;
use crate::tools::registry::ToolArgumentDiffConsumer;
use crate::tools::router::ToolRouterParams;
use crate::tools::router::extension_tool_executors;
use crate::tools::spec_plan::search_tool_enabled;
use crate::tools::spec_plan::tool_suggest_enabled;
use crate::turn_diff_tracker::TurnDiffTracker;
use crate::turn_timing::record_turn_ttft_metric;
use crate::util::error_or_panic;
use codex_analytics::AppInvocation;
use codex_analytics::CompactionPhase;
use codex_analytics::CompactionReason;
use codex_analytics::InvocationType;
use codex_analytics::TurnResolvedConfigFact;
use codex_analytics::build_track_events_context;
use codex_async_utils::OrCancelExt;
use codex_core_skills::injection::InjectedHostSkillPrompts;
use codex_extension_api::TurnInputContext;
use codex_extension_api::TurnInputEnvironment;
use codex_features::Feature;
use codex_git_utils::get_git_repo_root;
use codex_git_utils::get_git_repo_root_with_fs;
use codex_protocol::config_types::AutoCompactTokenLimitScope;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::items::PlanItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::build_hook_prompt_message;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentMessageContentDeltaEvent;
use codex_protocol::protocol::AgentReasoningSectionBreakEvent;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::PlanDeltaEvent;
use codex_protocol::protocol::ReasoningContentDeltaEvent;
use codex_protocol::protocol::ReasoningRawContentDeltaEvent;
use codex_protocol::protocol::TurnDiffEvent;
use codex_protocol::protocol::WarningEvent;
use codex_protocol::user_input::UserInput;
use codex_tools::ToolName;
use codex_tools::filter_request_plugin_install_discoverable_tools_for_client;
use codex_utils_stream_parser::AssistantTextChunk;
use codex_utils_stream_parser::AssistantTextStreamParser;
use codex_utils_stream_parser::ProposedPlanSegment;
use codex_utils_stream_parser::extract_proposed_plan_text;
use codex_utils_stream_parser::strip_citations;
use futures::future::BoxFuture;
use futures::prelude::*;
use futures::stream::FuturesOrdered;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use tracing::error;
use tracing::field;
use tracing::info;
use tracing::instrument;
use tracing::trace;
use tracing::trace_span;
use tracing::warn;

/// Takes initial turn input and runs a loop where, at each sampling request,
/// the model replies with either:
///
/// - requested function calls
/// - an assistant message
///
/// While it is possible for the model to return multiple of these items in a
/// single sampling request, in practice, we generally one item per sampling request:
///
/// - If the model requests a function call, we execute it and send the output
///   back to the model in the next sampling request.
/// - If the model sends only an assistant message, we record it in the
///   conversation history and consider the turn complete.
///
pub(crate) async fn run_turn(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    turn_extension_data: Arc<codex_extension_api::ExtensionData>,
    input: Vec<TurnInput>,
    prewarmed_client_session: Option<ModelClientSession>,
    cancellation_token: CancellationToken,
) -> Option<String> {
    let mut client_session =
        prewarmed_client_session.unwrap_or_else(|| sess.services.model_client.new_session());
    // TODO(ccunningham): Pre-turn compaction runs before context updates and the
    // new user message are recorded. Estimate pending incoming items (context
    // diffs/full reinjection + user input) and trigger compaction preemptively
    // when they would push the thread over the compaction threshold.
    if let Err(err) = run_pre_sampling_compact(&sess, &turn_context, &mut client_session).await {
        let error = err.to_codex_protocol_error();
        sess.emit_turn_error_lifecycle(turn_context.as_ref(), error.clone())
            .await;
        if error == CodexErrorInfo::UsageLimitExceeded
            && let Err(err) = sess
                .goal_runtime_apply(GoalRuntimeEvent::UsageLimitReached {
                    turn_context: turn_context.as_ref(),
                })
                .await
        {
            warn!("failed to usage-limit active goal after usage-limit error: {err}");
        }
        error!("Failed to run pre-sampling compact");
        return None;
    }

    sess.record_context_updates_and_set_reference_context_item(turn_context.as_ref())
        .await;

    let (injection_items, explicitly_enabled_connectors) =
        build_skills_and_plugins(&sess, turn_context.as_ref(), &input, &cancellation_token).await?;

    if run_pending_session_start_hooks(&sess, &turn_context).await {
        return None;
    }
    let mut can_drain_pending_input = input.is_empty();
    if run_hooks_and_record_inputs(&sess, &turn_context, &input).await {
        return None;
    }

    sess.merge_connector_selection(explicitly_enabled_connectors.clone())
        .await;
    sess.set_previous_turn_settings(Some(PreviousTurnSettings {
        model: turn_context.model_info.slug.clone(),
        realtime_active: Some(turn_context.realtime_active),
    }))
    .await;
    for response_item in injection_items {
        sess.record_conversation_items(&turn_context, std::slice::from_ref(&response_item))
            .await;
    }

    track_turn_resolved_config_analytics(&sess, &turn_context, &input).await;

    let mut last_agent_message: Option<String> = None;
    let mut stop_hook_active = false;
    // Although from the perspective of codex.rs, TurnDiffTracker has the lifecycle of a Task which contains
    // many turns, from the perspective of the user, it is a single turn.
    #[allow(deprecated)]
    let display_root = match turn_context.environments.primary() {
        Some(turn_environment) => get_git_repo_root_with_fs(
            turn_environment.environment.get_filesystem().as_ref(),
            &turn_environment.cwd,
        )
        .await
        .unwrap_or_else(|| turn_environment.cwd.clone())
        .into_path_buf(),
        None => get_git_repo_root(turn_context.cwd.as_path())
            .unwrap_or_else(|| turn_context.cwd.clone().into_path_buf()),
    };
    let turn_diff_tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::with_display_root(
        display_root,
    )));

    // `ModelClientSession` is turn-scoped and caches WebSocket + sticky routing state, so we reuse
    // one instance across retries within this turn.
    // Pending input is drained into history before building the next model request.
    // However, we defer that drain until after sampling in two cases:
    // 1. At the start of a turn, so the fresh turn input in `input` gets sampled first.
    // 2. After auto-compact, when model/tool continuation needs to resume before any steer.

    loop {
        // Note that pending_input would be something like a message the user
        // submitted through the UI while the model was running. Though the UI
        // may support this, the model might not.
        let pending_input = if can_drain_pending_input {
            sess.input_queue.get_pending_input(&sess.active_turn).await
        } else {
            Vec::new()
        };

        if run_hooks_and_record_inputs(&sess, &turn_context, &pending_input).await {
            break;
        }

        // Construct the input that we will send to the model.
        let sampling_request_input: Vec<ResponseItem> = {
            sess.clone_history()
                .await
                .for_prompt(&turn_context.model_info.input_modalities)
        };

        let window_id = sess.services.model_client.current_window_id();
        let turn_metadata_header = turn_context
            .turn_metadata_state
            .current_header_value_for_model_request(&window_id);
        match run_sampling_request(
            Arc::clone(&sess),
            Arc::clone(&turn_context),
            Arc::clone(&turn_extension_data),
            Arc::clone(&turn_diff_tracker),
            &mut client_session,
            turn_metadata_header.as_deref(),
            sampling_request_input.clone(),
            cancellation_token.child_token(),
        )
        .await
        {
            Ok(sampling_request_output) => {
                let SamplingRequestResult {
                    needs_follow_up: model_needs_follow_up,
                    last_agent_message: sampling_request_last_agent_message,
                } = sampling_request_output;
                can_drain_pending_input = true;
                let has_pending_input = sess.input_queue.has_pending_input(&sess.active_turn).await;
                let needs_follow_up = model_needs_follow_up || has_pending_input;
                let token_status =
                    auto_compact_token_status(sess.as_ref(), turn_context.as_ref()).await;
                let token_limit_reached = token_status.token_limit_reached;

                let estimated_token_count =
                    sess.get_estimated_token_count(turn_context.as_ref()).await;

                trace!(
                    turn_id = %turn_context.sub_id,
                    total_usage_tokens = token_status.active_context_tokens,
                    auto_compact_scope_tokens = token_status.auto_compact_scope_tokens,
                    estimated_token_count = ?estimated_token_count,
                    auto_compact_scope_limit = token_status.auto_compact_scope_limit,
                    auto_compact_limit_scope = ?turn_context.config.model_auto_compact_token_limit_scope,
                    auto_compact_window_ordinal = ?token_status.auto_compact_window_ordinal,
                    auto_compact_window_prefill_tokens = ?token_status.auto_compact_window_prefill_tokens,
                    full_context_window_limit = ?token_status.full_context_window_limit,
                    full_context_window_limit_reached = token_status.full_context_window_limit_reached,
                    token_limit_reached,
                    model_needs_follow_up,
                    has_pending_input,
                    needs_follow_up,
                    "post sampling token usage"
                );

                // as long as compaction works well in getting us way below the token limit, we shouldn't worry about being in an infinite loop.
                if token_limit_reached && needs_follow_up {
                    if let Err(err) = run_auto_compact(
                        &sess,
                        &turn_context,
                        &mut client_session,
                        InitialContextInjection::BeforeLastUserMessage,
                        CompactionReason::ContextLimit,
                        CompactionPhase::MidTurn,
                    )
                    .await
                    {
                        let error = err.to_codex_protocol_error();
                        sess.emit_turn_error_lifecycle(turn_context.as_ref(), error.clone())
                            .await;
                        if error == CodexErrorInfo::UsageLimitExceeded
                            && let Err(err) = sess
                                .goal_runtime_apply(GoalRuntimeEvent::UsageLimitReached {
                                    turn_context: turn_context.as_ref(),
                                })
                                .await
                        {
                            warn!(
                                "failed to usage-limit active goal after usage-limit error: {err}"
                            );
                        }
                        return None;
                    }
                    can_drain_pending_input = !model_needs_follow_up;
                    continue;
                }

                if !needs_follow_up {
                    last_agent_message = sampling_request_last_agent_message;
                    let stop_outcome = run_turn_stop_hooks(
                        &sess,
                        &turn_context,
                        stop_hook_active,
                        last_agent_message.clone(),
                    )
                    .await;
                    if stop_outcome.should_block {
                        if let Some(hook_prompt_message) =
                            build_hook_prompt_message(&stop_outcome.continuation_fragments)
                        {
                            sess.record_conversation_items(
                                &turn_context,
                                std::slice::from_ref(&hook_prompt_message),
                            )
                            .await;
                            stop_hook_active = true;
                            continue;
                        } else {
                            sess.send_event(
                                &turn_context,
                                EventMsg::Warning(WarningEvent {
                                    message: "Stop hook requested continuation without a prompt; ignoring the block.".to_string(),
                                }),
                            )
                            .await;
                        }
                    }
                    if stop_outcome.should_stop {
                        break;
                    }
                    if run_legacy_after_agent_hook(
                        &sess,
                        &turn_context,
                        &sampling_request_input,
                        last_agent_message.clone(),
                    )
                    .await
                    {
                        return None;
                    }
                    break;
                }
                continue;
            }
            Err(CodexErr::TurnAborted) => {
                // Aborted turn is reported via a different event.
                break;
            }
            Err(codex_error @ CodexErr::InvalidImageRequest()) => {
                {
                    let mut state = sess.state.lock().await;
                    error_or_panic(
                        "Invalid image detected; sanitizing tool output to prevent poisoning",
                    );
                    if state.history.replace_last_turn_images("Invalid image") {
                        continue;
                    }
                }

                sess.track_turn_codex_error(turn_context.as_ref(), &codex_error);
                let error = CodexErrorInfo::BadRequest;
                sess.emit_turn_error_lifecycle(turn_context.as_ref(), error.clone())
                    .await;
                let event = EventMsg::Error(ErrorEvent {
                    message: "Invalid image in your last message. Please remove it and try again."
                        .to_string(),
                    codex_error_info: Some(error),
                });
                sess.send_event(&turn_context, event).await;
                break;
            }
            Err(e) => {
                info!("Turn error: {e:#}");
                let error = e.to_codex_protocol_error();
                sess.emit_turn_error_lifecycle(turn_context.as_ref(), error.clone())
                    .await;
                if error == CodexErrorInfo::UsageLimitExceeded
                    && let Err(err) = sess
                        .goal_runtime_apply(GoalRuntimeEvent::UsageLimitReached {
                            turn_context: turn_context.as_ref(),
                        })
                        .await
                {
                    warn!("failed to usage-limit active goal after usage-limit error: {err}");
                }
                sess.track_turn_codex_error(turn_context.as_ref(), &e);
                let event = EventMsg::Error(e.to_error_event(/*message_prefix*/ None));
                sess.send_event(&turn_context, event).await;
                // let the user continue the conversation
                break;
            }
        }
    }

    last_agent_message
}

async fn run_hooks_and_record_inputs(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    input: &[TurnInput],
) -> bool {
    let mut blocked_input = false;
    let mut accepted_user_input = false;
    for input_item in input {
        let hook_outcome = inspect_pending_input(sess, turn_context, input_item).await;
        if hook_outcome.should_stop {
            blocked_input = true;
            record_additional_contexts(sess, turn_context, hook_outcome.additional_contexts).await;
        } else {
            if matches!(input_item, TurnInput::UserInput { content, .. } if !content.is_empty()) {
                accepted_user_input = true;
            }
            record_pending_input(
                sess,
                turn_context,
                input_item.clone(),
                hook_outcome.additional_contexts,
            )
            .await;
        }
    }
    blocked_input && !accepted_user_input
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "MCP tool listing borrows the read guard across cancellation-aware await"
)]
async fn build_skills_and_plugins(
    sess: &Arc<Session>,
    turn_context: &TurnContext,
    input: &[TurnInput],
    cancellation_token: &CancellationToken,
) -> Option<(Vec<ResponseItem>, HashSet<String>)> {
    let user_input = input
        .iter()
        .filter_map(|item| match item {
            TurnInput::UserInput { content, .. } => Some(content.as_slice()),
            TurnInput::ResponseItem(_) => None,
        })
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    let tracking = build_track_events_context(
        turn_context.model_info.slug.clone(),
        sess.thread_id.to_string(),
        turn_context.sub_id.clone(),
    );
    let loaded_plugins = sess
        .services
        .plugins_manager
        .plugins_for_config(&turn_context.config.plugins_config_input())
        .await;
    // Structured plugin:// mentions are resolved from the current session's
    // enabled plugins, then converted into turn-scoped guidance below.
    let mentioned_plugins =
        collect_explicit_plugin_mentions(&user_input, loaded_plugins.capability_summaries());
    let mcp_tools = if turn_context.apps_enabled() || !mentioned_plugins.is_empty() {
        // Plugin mentions need raw MCP/app inventory even when app tools
        // are normally hidden so we can describe the plugin's currently
        // usable capabilities for this turn.
        match sess
            .services
            .mcp_connection_manager
            .read()
            .await
            .list_all_tools()
            .or_cancel(cancellation_token)
            .await
        {
            Ok(mcp_tools) => mcp_tools,
            Err(_) if turn_context.apps_enabled() => return None,
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };
    let available_connectors = if turn_context.apps_enabled() {
        let connectors = codex_connectors::merge::merge_plugin_connectors_with_accessible(
            loaded_plugins
                .effective_apps()
                .into_iter()
                .map(|connector_id| connector_id.0),
            connectors::accessible_connectors_from_mcp_tools(&mcp_tools),
        );
        connectors::with_app_enabled_state(connectors, &turn_context.config)
    } else {
        Vec::new()
    };
    let skills_outcome = turn_context.turn_skills.outcome.as_ref();
    let connector_slug_counts = build_connector_slug_counts(&available_connectors);
    let extension_injection_items =
        build_extension_turn_input_items(sess, turn_context, &user_input, cancellation_token)
            .await?;
    let skill_name_counts_lower =
        build_skill_name_counts(&skills_outcome.skills, &skills_outcome.disabled_paths).1;
    let mentioned_skills = collect_explicit_skill_mentions(
        &user_input,
        &skills_outcome.skills,
        &skills_outcome.disabled_paths,
        &connector_slug_counts,
    );
    maybe_prompt_and_install_mcp_dependencies(
        sess,
        turn_context,
        cancellation_token,
        &mentioned_skills,
        Some(sess.mcp_elicitation_reviewer()),
    )
    .await;

    let injected_host_skill_prompts = turn_context
        .extension_data
        .get::<InjectedHostSkillPrompts>();
    let SkillInjections {
        items: skill_injections,
        warnings: skill_warnings,
    } = build_skill_injections(
        &mentioned_skills,
        Some(skills_outcome),
        Some(&turn_context.session_telemetry),
        &sess.services.analytics_events_client,
        tracking.clone(),
    )
    .await;

    for message in skill_warnings {
        sess.send_event(turn_context, EventMsg::Warning(WarningEvent { message }))
            .await;
    }

    let skill_items: Vec<ResponseItem> = skill_injections
        .iter()
        .map(|skill| ContextualUserFragment::into(crate::context::SkillInstructions::from(skill)))
        .collect();
    let skill_connector_ids = collect_explicit_app_ids_from_skill_items(
        &skill_items,
        &available_connectors,
        &skill_name_counts_lower,
    );
    let plugin_items =
        build_plugin_injections(&mentioned_plugins, &mcp_tools, &available_connectors);
    let mut explicitly_enabled_connectors = collect_explicit_app_ids(&user_input);
    explicitly_enabled_connectors.extend(skill_connector_ids);
    let connector_names_by_id = available_connectors
        .iter()
        .map(|connector| (connector.id.as_str(), connector.name.as_str()))
        .collect::<HashMap<&str, &str>>();
    let mentioned_app_invocations = explicitly_enabled_connectors
        .iter()
        .map(|connector_id| AppInvocation {
            connector_id: Some(connector_id.clone()),
            app_name: connector_names_by_id
                .get(connector_id.as_str())
                .map(|name| (*name).to_string()),
            invocation_type: Some(InvocationType::Explicit),
        })
        .collect::<Vec<_>>();
    sess.services
        .analytics_events_client
        .track_app_mentioned(tracking.clone(), mentioned_app_invocations);
    for plugin in mentioned_plugins
        .iter()
        .filter_map(crate::plugins::PluginCapabilitySummary::telemetry_metadata)
    {
        sess.services
            .analytics_events_client
            .track_plugin_used(tracking.clone(), plugin);
    }

    let mut injection_items: Vec<ResponseItem> = match injected_host_skill_prompts {
        Some(injected_host_skill_prompts) => skill_injections
            .iter()
            .filter(|skill| !injected_host_skill_prompts.contains_path(&skill.path))
            .map(|skill| {
                ContextualUserFragment::into(crate::context::SkillInstructions::from(skill))
            })
            .collect(),
        None => skill_items,
    };
    injection_items.extend(plugin_items);
    injection_items.extend(extension_injection_items);
    Some((injection_items, explicitly_enabled_connectors))
}

async fn build_extension_turn_input_items(
    sess: &Arc<Session>,
    turn_context: &TurnContext,
    user_input: &[UserInput],
    cancellation_token: &CancellationToken,
) -> Option<Vec<ResponseItem>> {
    let contributors = sess.services.extensions.turn_input_contributors().to_vec();
    if contributors.is_empty() {
        return Some(Vec::new());
    }

    let environments = turn_context
        .environments
        .turn_environments
        .iter()
        .enumerate()
        .map(|(index, environment)| TurnInputEnvironment {
            environment_id: environment.environment_id.clone(),
            cwd: environment.cwd.as_path().to_path_buf(),
            is_primary: index == 0,
        })
        .collect::<Vec<_>>();

    let input = TurnInputContext {
        turn_id: turn_context.sub_id.to_string(),
        user_input: user_input.to_vec(),
        environments,
    };

    let mut items = Vec::new();
    for contributor in contributors {
        let contributed_fragments = contributor
            .contribute(
                input.clone(),
                &sess.services.session_extension_data,
                &sess.services.thread_extension_data,
                turn_context.extension_data.as_ref(),
            )
            .or_cancel(cancellation_token)
            .await
            .ok()?;
        items.extend(
            contributed_fragments
                .into_iter()
                .map(ContextualUserFragment::into_boxed_response_item),
        );
    }

    Some(items)
}

async fn track_turn_resolved_config_analytics(
    sess: &Session,
    turn_context: &TurnContext,
    input: &[TurnInput],
) {
    let thread_config = {
        let state = sess.state.lock().await;
        state.session_configuration.thread_config_snapshot()
    };
    let is_first_turn = {
        let mut state = sess.state.lock().await;
        state.take_next_turn_is_first()
    };
    sess.services
        .analytics_events_client
        .track_turn_resolved_config(TurnResolvedConfigFact {
            turn_id: turn_context.sub_id.clone(),
            thread_id: sess.thread_id.to_string(),
            num_input_images: input
                .iter()
                .filter_map(|item| match item {
                    TurnInput::UserInput { content, .. } => Some(content.as_slice()),
                    TurnInput::ResponseItem(_) => None,
                })
                .flatten()
                .filter(|item| {
                    matches!(item, UserInput::Image { .. } | UserInput::LocalImage { .. })
                })
                .count(),
            submission_type: None,
            ephemeral: thread_config.ephemeral,
            session_source: thread_config.session_source,
            model: turn_context.model_info.slug.clone(),
            model_provider: turn_context.config.model_provider_id.clone(),
            permission_profile: turn_context.permission_profile(),
            #[allow(deprecated)]
            permission_profile_cwd: turn_context.cwd.to_path_buf(),
            reasoning_effort: turn_context.reasoning_effort.clone(),
            reasoning_summary: Some(turn_context.reasoning_summary),
            service_tier: turn_context
                .config
                .service_tier
                .as_deref()
                .and_then(ServiceTier::from_request_value),
            approval_policy: turn_context.approval_policy.value(),
            approvals_reviewer: turn_context.config.approvals_reviewer,
            sandbox_network_access: turn_context.network_sandbox_policy().is_enabled(),
            collaboration_mode: turn_context.collaboration_mode.mode,
            personality: turn_context.personality,
            workspace_kind: turn_context.turn_metadata_state.workspace_kind(),
            is_first_turn,
        });
}

#[derive(Debug)]
struct AutoCompactTokenStatus {
    // Full active context usage, independent of the configured auto-compact scope.
    active_context_tokens: i64,
    // Usage counted against `model_auto_compact_token_limit` for the current scope.
    auto_compact_scope_tokens: i64,
    auto_compact_scope_limit: i64,
    full_context_window_limit: Option<i64>,
    auto_compact_window_ordinal: Option<u64>,
    auto_compact_window_prefill_tokens: Option<i64>,
    full_context_window_limit_reached: bool,
    token_limit_reached: bool,
}

async fn auto_compact_token_status(
    sess: &Session,
    turn_context: &TurnContext,
) -> AutoCompactTokenStatus {
    let active_context_tokens = sess.get_total_token_usage().await;
    let mut auto_compact_window_ordinal = None;
    let mut auto_compact_window_prefill_tokens = None;
    let (auto_compact_scope_tokens, auto_compact_scope_limit, full_context_window_limit) =
        match turn_context.config.model_auto_compact_token_limit_scope {
            AutoCompactTokenLimitScope::Total => (
                active_context_tokens,
                turn_context
                    .model_info
                    .auto_compact_token_limit()
                    .unwrap_or(i64::MAX),
                None,
            ),
            AutoCompactTokenLimitScope::BodyAfterPrefix => {
                let window = sess.auto_compact_window_snapshot().await;
                auto_compact_window_ordinal = Some(window.ordinal);
                auto_compact_window_prefill_tokens = window.prefill_input_tokens;
                let baseline = window.prefill_input_tokens.unwrap_or(active_context_tokens);
                (
                    active_context_tokens.saturating_sub(baseline),
                    turn_context
                        .config
                        .model_auto_compact_token_limit
                        .or_else(|| turn_context.model_info.auto_compact_token_limit())
                        .unwrap_or(i64::MAX),
                    turn_context.model_context_window(),
                )
            }
        };
    let full_context_window_limit_reached =
        full_context_window_limit.is_some_and(|full_context_window_limit| {
            active_context_tokens >= full_context_window_limit
        });
    let token_limit_reached =
        auto_compact_scope_tokens >= auto_compact_scope_limit || full_context_window_limit_reached;

    AutoCompactTokenStatus {
        active_context_tokens,
        auto_compact_scope_tokens,
        auto_compact_scope_limit,
        full_context_window_limit,
        auto_compact_window_ordinal,
        auto_compact_window_prefill_tokens,
        full_context_window_limit_reached,
        token_limit_reached,
    }
}

async fn run_pre_sampling_compact(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    client_session: &mut ModelClientSession,
) -> CodexResult<()> {
    maybe_run_previous_model_inline_compact(sess, turn_context, client_session).await?;
    let token_status = auto_compact_token_status(sess.as_ref(), turn_context.as_ref()).await;
    // Compact if the configured auto-compaction budget or usable context window is exhausted.
    if token_status.token_limit_reached {
        run_auto_compact(
            sess,
            turn_context,
            client_session,
            InitialContextInjection::DoNotInject,
            CompactionReason::ContextLimit,
            CompactionPhase::PreTurn,
        )
        .await?;
    }
    Ok(())
}

/// Runs pre-sampling compaction against the previous model when switching to a smaller
/// context-window model.
///
/// Returns `Err(_)` only when compaction was attempted and failed.
async fn maybe_run_previous_model_inline_compact(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    client_session: &mut ModelClientSession,
) -> CodexResult<()> {
    let Some(previous_turn_settings) = sess.previous_turn_settings().await else {
        return Ok(());
    };
    let previous_model_turn_context = Arc::new(
        turn_context
            .with_model(previous_turn_settings.model, &sess.services.models_manager)
            .await,
    );

    let Some(old_context_window) = previous_model_turn_context.model_context_window() else {
        return Ok(());
    };
    let Some(new_context_window) = turn_context.model_context_window() else {
        return Ok(());
    };
    let active_context_tokens = sess.get_total_token_usage().await;
    let previous_model_limit_reached = match turn_context
        .config
        .model_auto_compact_token_limit_scope
    {
        AutoCompactTokenLimitScope::Total => {
            let new_auto_compact_limit = turn_context
                .model_info
                .auto_compact_token_limit()
                .unwrap_or(i64::MAX);
            active_context_tokens > new_auto_compact_limit
                || active_context_tokens >= new_context_window
        }
        AutoCompactTokenLimitScope::BodyAfterPrefix => active_context_tokens >= new_context_window,
    };
    let should_run = previous_model_limit_reached
        && previous_model_turn_context.model_info.slug != turn_context.model_info.slug
        && old_context_window > new_context_window;
    if should_run {
        run_auto_compact(
            sess,
            &previous_model_turn_context,
            client_session,
            InitialContextInjection::DoNotInject,
            CompactionReason::ModelDownshift,
            CompactionPhase::PreTurn,
        )
        .await?;
    }
    Ok(())
}

async fn run_auto_compact(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    client_session: &mut ModelClientSession,
    initial_context_injection: InitialContextInjection,
    reason: CompactionReason,
    phase: CompactionPhase,
) -> CodexResult<()> {
    if should_use_remote_compact_task(turn_context.provider.info()) {
        if turn_context.features.enabled(Feature::RemoteCompactionV2) {
            emit_compact_metric(
                &sess.services.session_telemetry,
                "remote_v2",
                /*manual*/ false,
            );
            run_inline_remote_auto_compact_task_v2(
                Arc::clone(sess),
                Arc::clone(turn_context),
                client_session,
                initial_context_injection,
                reason,
                phase,
            )
            .await?;
            return Ok(());
        }
        emit_compact_metric(
            &sess.services.session_telemetry,
            "remote",
            /*manual*/ false,
        );
        run_inline_remote_auto_compact_task(
            Arc::clone(sess),
            Arc::clone(turn_context),
            initial_context_injection,
            reason,
            phase,
        )
        .await?;
    } else {
        emit_compact_metric(
            &sess.services.session_telemetry,
            "local",
            /*manual*/ false,
        );
        run_inline_auto_compact_task(
            Arc::clone(sess),
            Arc::clone(turn_context),
            initial_context_injection,
            reason,
            phase,
        )
        .await?;
    }
    Ok(())
}

pub(super) fn collect_explicit_app_ids_from_skill_items(
    skill_items: &[ResponseItem],
    connectors: &[connectors::AppInfo],
    skill_name_counts_lower: &HashMap<String, usize>,
) -> HashSet<String> {
    if skill_items.is_empty() || connectors.is_empty() {
        return HashSet::new();
    }

    let skill_messages = skill_items
        .iter()
        .filter_map(|item| match item {
            ResponseItem::Message { content, .. } => {
                content.iter().find_map(|content_item| match content_item {
                    ContentItem::InputText { text } => Some(text.clone()),
                    _ => None,
                })
            }
            _ => None,
        })
        .collect::<Vec<String>>();
    if skill_messages.is_empty() {
        return HashSet::new();
    }

    let mentions = collect_tool_mentions_from_messages(&skill_messages);
    let mention_names_lower = mentions
        .plain_names
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect::<HashSet<String>>();
    let mut connector_ids = mentions
        .paths
        .iter()
        .filter(|path| tool_kind_for_path(path) == ToolMentionKind::App)
        .filter_map(|path| app_id_from_path(path).map(str::to_string))
        .collect::<HashSet<String>>();

    let connector_slug_counts = build_connector_slug_counts(connectors);
    for connector in connectors {
        let slug = codex_connectors::metadata::connector_mention_slug(connector);
        let connector_count = connector_slug_counts.get(&slug).copied().unwrap_or(0);
        let skill_count = skill_name_counts_lower.get(&slug).copied().unwrap_or(0);
        if connector_count == 1 && skill_count == 0 && mention_names_lower.contains(&slug) {
            connector_ids.insert(connector.id.clone());
        }
    }

    connector_ids
}

pub(crate) fn build_prompt(
    input: Vec<ResponseItem>,
    router: &ToolRouter,
    turn_context: &TurnContext,
    base_instructions: BaseInstructions,
) -> Prompt {
    Prompt {
        input,
        tools: router.model_visible_specs(),
        parallel_tool_calls: turn_context.model_info.supports_parallel_tool_calls,
        base_instructions,
        personality: turn_context.personality,
        output_schema: turn_context.final_output_json_schema.clone(),
        output_schema_strict: !crate::guardian::is_guardian_reviewer_source(
            &turn_context.session_source,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(deprecated)]
#[instrument(level = "trace",
    skip_all,
    fields(
        turn_id = %turn_context.sub_id,
        model = %turn_context.model_info.slug,
        cwd = %turn_context.cwd.display()
    )
)]
async fn run_sampling_request(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    turn_store: Arc<codex_extension_api::ExtensionData>,
    turn_diff_tracker: SharedTurnDiffTracker,
    client_session: &mut ModelClientSession,
    turn_metadata_header: Option<&str>,
    input: Vec<ResponseItem>,
    cancellation_token: CancellationToken,
) -> CodexResult<SamplingRequestResult> {
    let router = built_tools(sess.as_ref(), turn_context.as_ref(), &cancellation_token).await?;

    let base_instructions = sess.get_base_instructions().await;

    let tool_runtime = ToolCallRuntime::new(
        Arc::clone(&router),
        Arc::clone(&sess),
        Arc::clone(&turn_context),
        Arc::clone(&turn_diff_tracker),
    );
    let _code_mode_worker = sess.services.code_mode_service.start_turn_worker(
        &sess,
        &turn_context,
        Arc::clone(&router),
        Arc::clone(&turn_diff_tracker),
    );
    let max_retries = turn_context.provider.info().stream_max_retries();
    let mut retries = 0;
    let mut initial_input = Some(input);
    loop {
        let prompt_input = if let Some(input) = initial_input.take() {
            input
        } else {
            sess.clone_history()
                .await
                .for_prompt(&turn_context.model_info.input_modalities)
        };
        let prompt = build_prompt(
            prompt_input,
            router.as_ref(),
            turn_context.as_ref(),
            base_instructions.clone(),
        );
        let err = match try_run_sampling_request(
            tool_runtime.clone(),
            Arc::clone(&sess),
            Arc::clone(&turn_context),
            Arc::clone(&turn_store),
            client_session,
            turn_metadata_header,
            Arc::clone(&turn_diff_tracker),
            &prompt,
            cancellation_token.child_token(),
        )
        .await
        {
            Ok(output) => {
                return Ok(output);
            }
            Err(CodexErr::ContextWindowExceeded) => {
                sess.set_total_tokens_full(&turn_context).await;
                return Err(CodexErr::ContextWindowExceeded);
            }
            Err(CodexErr::UsageLimitReached(e)) => {
                let rate_limits = e.rate_limits.clone();
                if let Some(rate_limits) = rate_limits {
                    sess.update_rate_limits(&turn_context, *rate_limits).await;
                }
                return Err(CodexErr::UsageLimitReached(e));
            }
            Err(err) => err,
        };

        if !err.is_retryable() {
            return Err(err);
        }

        handle_retryable_response_stream_error(
            &mut retries,
            max_retries,
            err,
            client_session,
            &sess,
            &turn_context,
            ResponsesStreamRequest::Sampling,
        )
        .await?;
    }
}

#[expect(
    clippy::await_holding_invalid_type,
    reason = "tool router construction reads through the session-owned manager guard"
)]
#[instrument(level = "trace",
    skip_all,
    fields(
        turn_id = %turn_context.sub_id,
        model = %turn_context.model_info.slug,
        apps_enabled = turn_context.apps_enabled()
    )
)]
pub(crate) async fn built_tools(
    sess: &Session,
    turn_context: &TurnContext,
    cancellation_token: &CancellationToken,
) -> CodexResult<Arc<ToolRouter>> {
    let mcp_connection_manager = sess
        .services
        .mcp_connection_manager
        .read()
        .instrument(trace_span!("read_mcp_connection_manager"))
        .await;
    let has_mcp_servers = mcp_connection_manager.has_servers();
    let all_mcp_tools = mcp_connection_manager
        .list_all_tools()
        .or_cancel(cancellation_token)
        .await?;
    drop(mcp_connection_manager);
    let loaded_plugins = sess
        .services
        .plugins_manager
        .plugins_for_config(&turn_context.config.plugins_config_input())
        .await;

    let apps_enabled = turn_context.apps_enabled();
    let accessible_connectors =
        apps_enabled.then(|| connectors::accessible_connectors_from_mcp_tools(&all_mcp_tools));
    let accessible_connectors_with_enabled_state =
        accessible_connectors.as_ref().map(|connectors| {
            connectors::with_app_enabled_state(connectors.clone(), &turn_context.config)
        });
    let connectors = if apps_enabled {
        let connectors = codex_connectors::merge::merge_plugin_connectors_with_accessible(
            loaded_plugins
                .effective_apps()
                .into_iter()
                .map(|connector_id| connector_id.0),
            accessible_connectors.clone().unwrap_or_default(),
        );
        Some(connectors::with_app_enabled_state(
            connectors,
            &turn_context.config,
        ))
    } else {
        None
    };
    let auth = sess.services.auth_manager.auth().await;
    let loaded_plugin_app_connector_ids = loaded_plugins
        .effective_apps()
        .into_iter()
        .map(|connector_id| connector_id.0)
        .collect::<Vec<_>>();
    let discoverable_tools = if apps_enabled && tool_suggest_enabled(turn_context) {
        if let Some(accessible_connectors) = accessible_connectors_with_enabled_state.as_ref() {
            match connectors::list_tool_suggest_discoverable_tools_with_auth(
                &turn_context.config,
                sess.services.plugins_manager.as_ref(),
                auth.as_ref(),
                accessible_connectors.as_slice(),
                &loaded_plugin_app_connector_ids,
            )
            .await
            .map(|discoverable_tools| {
                filter_request_plugin_install_discoverable_tools_for_client(
                    discoverable_tools,
                    turn_context.app_server_client_name.as_deref(),
                )
            }) {
                Ok(discoverable_tools) if discoverable_tools.is_empty() => None,
                Ok(discoverable_tools) => Some(discoverable_tools),
                Err(err) => {
                    warn!("failed to load discoverable tool suggestions: {err:#}");
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let mcp_tool_exposure = build_mcp_tool_exposure(
        &all_mcp_tools,
        connectors.as_deref(),
        &turn_context.config,
        search_tool_enabled(turn_context),
    );
    let mcp_tools = has_mcp_servers.then_some(mcp_tool_exposure.direct_tools);
    let deferred_mcp_tools = mcp_tool_exposure.deferred_tools;
    Ok(Arc::new(ToolRouter::from_turn_context(
        turn_context,
        ToolRouterParams {
            mcp_tools,
            deferred_mcp_tools,
            discoverable_tools,
            extension_tool_executors: extension_tool_executors(sess),
            dynamic_tools: turn_context.dynamic_tools.as_slice(),
        },
    )))
}

#[derive(Debug)]
struct SamplingRequestResult {
    needs_follow_up: bool,
    last_agent_message: Option<String>,
}

/// Ephemeral per-response state for streaming a single proposed plan.
/// This is intentionally not persisted or stored in session/state since it
/// only exists while a response is actively streaming. The final plan text
/// is extracted from the completed assistant message.
/// Tracks a single proposed plan item across a streaming response.
struct ProposedPlanItemState {
    item_id: String,
    started: bool,
    completed: bool,
}

/// Aggregated state used only while streaming a plan-mode response.
/// Includes per-item parsers, deferred agent message bookkeeping, and the plan item lifecycle.
struct PlanModeStreamState {
    /// Agent message items started by the model but deferred until we see non-plan text.
    pending_agent_message_items: HashMap<String, TurnItem>,
    /// Agent message items whose start notification has been emitted.
    started_agent_message_items: HashSet<String>,
    /// Leading whitespace buffered until we see non-whitespace text for an item.
    leading_whitespace_by_item: HashMap<String, String>,
    /// Tracks plan item lifecycle while streaming plan output.
    plan_item_state: ProposedPlanItemState,
}

impl PlanModeStreamState {
    fn new(turn_id: &str) -> Self {
        Self {
            pending_agent_message_items: HashMap::new(),
            started_agent_message_items: HashSet::new(),
            leading_whitespace_by_item: HashMap::new(),
            plan_item_state: ProposedPlanItemState::new(turn_id),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct AssistantMessageStreamParsers {
    plan_mode: bool,
    parsers_by_item: HashMap<String, AssistantTextStreamParser>,
}

type ParsedAssistantTextDelta = AssistantTextChunk;

impl AssistantMessageStreamParsers {
    pub(super) fn new(plan_mode: bool) -> Self {
        Self {
            plan_mode,
            parsers_by_item: HashMap::new(),
        }
    }

    fn parser_mut(&mut self, item_id: &str) -> &mut AssistantTextStreamParser {
        let plan_mode = self.plan_mode;
        self.parsers_by_item
            .entry(item_id.to_string())
            .or_insert_with(|| AssistantTextStreamParser::new(plan_mode))
    }

    pub(super) fn seed_item_text(&mut self, item_id: &str, text: &str) -> ParsedAssistantTextDelta {
        if text.is_empty() {
            return ParsedAssistantTextDelta::default();
        }
        self.parser_mut(item_id).push_str(text)
    }

    pub(super) fn parse_delta(&mut self, item_id: &str, delta: &str) -> ParsedAssistantTextDelta {
        self.parser_mut(item_id).push_str(delta)
    }

    pub(super) fn finish_item(&mut self, item_id: &str) -> ParsedAssistantTextDelta {
        let Some(mut parser) = self.parsers_by_item.remove(item_id) else {
            return ParsedAssistantTextDelta::default();
        };
        parser.finish()
    }

    fn drain_finished(&mut self) -> Vec<(String, ParsedAssistantTextDelta)> {
        let parsers_by_item = std::mem::take(&mut self.parsers_by_item);
        parsers_by_item
            .into_iter()
            .map(|(item_id, mut parser)| (item_id, parser.finish()))
            .collect()
    }
}

impl ProposedPlanItemState {
    fn new(turn_id: &str) -> Self {
        Self {
            item_id: format!("{turn_id}-plan"),
            started: false,
            completed: false,
        }
    }

    async fn start(&mut self, sess: &Session, turn_context: &TurnContext) {
        if self.started || self.completed {
            return;
        }
        self.started = true;
        let item = TurnItem::Plan(PlanItem {
            id: self.item_id.clone(),
            text: String::new(),
        });
        sess.emit_turn_item_started(turn_context, &item).await;
    }

    async fn push_delta(&mut self, sess: &Session, turn_context: &TurnContext, delta: &str) {
        if self.completed {
            return;
        }
        if delta.is_empty() {
            return;
        }
        let event = PlanDeltaEvent {
            thread_id: sess.thread_id.to_string(),
            turn_id: turn_context.sub_id.clone(),
            item_id: self.item_id.clone(),
            delta: delta.to_string(),
        };
        sess.send_event(turn_context, EventMsg::PlanDelta(event))
            .await;
    }

    async fn complete_with_text(
        &mut self,
        sess: &Session,
        turn_context: &TurnContext,
        text: String,
    ) {
        if self.completed || !self.started {
            return;
        }
        self.completed = true;
        let item = TurnItem::Plan(PlanItem {
            id: self.item_id.clone(),
            text,
        });
        sess.emit_turn_item_completed(turn_context, item).await;
    }
}

/// In plan mode we defer agent message starts until the parser emits non-plan
/// text. The parser buffers each line until it can rule out a tag prefix, so
/// plan-only outputs never show up as empty assistant messages.
async fn maybe_emit_pending_agent_message_start(
    sess: &Session,
    turn_context: &TurnContext,
    state: &mut PlanModeStreamState,
    item_id: &str,
) {
    if state.started_agent_message_items.contains(item_id) {
        return;
    }
    if let Some(item) = state.pending_agent_message_items.remove(item_id) {
        sess.emit_turn_item_started(turn_context, &item).await;
        state
            .started_agent_message_items
            .insert(item_id.to_string());
    }
}

/// Agent messages are text-only today; concatenate all text entries.
fn agent_message_text(item: &codex_protocol::items::AgentMessageItem) -> String {
    item.content
        .iter()
        .map(|entry| match entry {
            codex_protocol::items::AgentMessageContent::Text { text } => text.as_str(),
        })
        .collect()
}

pub(super) fn realtime_text_for_event(msg: &EventMsg) -> Option<String> {
    match msg {
        EventMsg::AgentMessage(event) => Some(event.message.clone()),
        EventMsg::ItemCompleted(event) => match &event.item {
            TurnItem::AgentMessage(item) => Some(agent_message_text(item)),
            _ => None,
        },
        EventMsg::Error(_)
        | EventMsg::Warning(_)
        | EventMsg::GuardianWarning(_)
        | EventMsg::RealtimeConversationStarted(_)
        | EventMsg::RealtimeConversationSdp(_)
        | EventMsg::RealtimeConversationRealtime(_)
        | EventMsg::RealtimeConversationClosed(_)
        | EventMsg::ModelReroute(_)
        | EventMsg::ModelVerification(_)
        | EventMsg::TurnModerationMetadata(_)
        | EventMsg::ContextCompacted(_)
        | EventMsg::ThreadRolledBack(_)
        | EventMsg::TurnStarted(_)
        | EventMsg::ThreadSettingsApplied(_)
        | EventMsg::TurnComplete(_)
        | EventMsg::TokenCount(_)
        | EventMsg::UserMessage(_)
        | EventMsg::AgentReasoning(_)
        | EventMsg::AgentReasoningRawContent(_)
        | EventMsg::AgentReasoningSectionBreak(_)
        | EventMsg::SessionConfigured(_)
        | EventMsg::ThreadGoalUpdated(_)
        | EventMsg::McpStartupUpdate(_)
        | EventMsg::McpStartupComplete(_)
        | EventMsg::McpToolCallBegin(_)
        | EventMsg::McpToolCallEnd(_)
        | EventMsg::WebSearchBegin(_)
        | EventMsg::WebSearchEnd(_)
        | EventMsg::ExecCommandBegin(_)
        | EventMsg::ExecCommandOutputDelta(_)
        | EventMsg::TerminalInteraction(_)
        | EventMsg::ExecCommandEnd(_)
        | EventMsg::PatchApplyBegin(_)
        | EventMsg::PatchApplyUpdated(_)
        | EventMsg::PatchApplyEnd(_)
        | EventMsg::ImageGenerationBegin(_)
        | EventMsg::ImageGenerationEnd(_)
        | EventMsg::ViewImageToolCall(_)
        | EventMsg::ExecApprovalRequest(_)
        | EventMsg::RequestPermissions(_)
        | EventMsg::RequestUserInput(_)
        | EventMsg::DynamicToolCallRequest(_)
        | EventMsg::DynamicToolCallResponse(_)
        | EventMsg::GuardianAssessment(_)
        | EventMsg::ElicitationRequest(_)
        | EventMsg::ApplyPatchApprovalRequest(_)
        | EventMsg::DeprecationNotice(_)
        | EventMsg::StreamError(_)
        | EventMsg::TurnDiff(_)
        | EventMsg::RealtimeConversationListVoicesResponse(_)
        | EventMsg::PlanUpdate(_)
        | EventMsg::TurnAborted(_)
        | EventMsg::ShutdownComplete
        | EventMsg::EnteredReviewMode(_)
        | EventMsg::ExitedReviewMode(_)
        | EventMsg::RawResponseItem(_)
        | EventMsg::ItemStarted(_)
        | EventMsg::HookStarted(_)
        | EventMsg::HookCompleted(_)
        | EventMsg::AgentMessageContentDelta(_)
        | EventMsg::PlanDelta(_)
        | EventMsg::ReasoningContentDelta(_)
        | EventMsg::ReasoningRawContentDelta(_)
        | EventMsg::CollabAgentSpawnBegin(_)
        | EventMsg::CollabAgentSpawnEnd(_)
        | EventMsg::CollabAgentInteractionBegin(_)
        | EventMsg::CollabAgentInteractionEnd(_)
        | EventMsg::CollabWaitingBegin(_)
        | EventMsg::CollabWaitingEnd(_)
        | EventMsg::CollabCloseBegin(_)
        | EventMsg::CollabCloseEnd(_)
        | EventMsg::CollabResumeBegin(_)
        | EventMsg::CollabResumeEnd(_) => None,
    }
}

/// Split the stream into normal assistant text vs. proposed plan content.
/// Normal text becomes AgentMessage deltas; plan content becomes PlanDelta +
/// TurnItem::Plan.
async fn handle_plan_segments(
    sess: &Session,
    turn_context: &TurnContext,
    state: &mut PlanModeStreamState,
    item_id: &str,
    segments: Vec<ProposedPlanSegment>,
) {
    for segment in segments {
        match segment {
            ProposedPlanSegment::Normal(delta) => {
                if delta.is_empty() {
                    continue;
                }
                let has_non_whitespace = delta.chars().any(|ch| !ch.is_whitespace());
                if !has_non_whitespace && !state.started_agent_message_items.contains(item_id) {
                    let entry = state
                        .leading_whitespace_by_item
                        .entry(item_id.to_string())
                        .or_default();
                    entry.push_str(&delta);
                    continue;
                }
                let delta = if !state.started_agent_message_items.contains(item_id) {
                    if let Some(prefix) = state.leading_whitespace_by_item.remove(item_id) {
                        format!("{prefix}{delta}")
                    } else {
                        delta
                    }
                } else {
                    delta
                };
                maybe_emit_pending_agent_message_start(sess, turn_context, state, item_id).await;

                let event = AgentMessageContentDeltaEvent {
                    thread_id: sess.thread_id.to_string(),
                    turn_id: turn_context.sub_id.clone(),
                    item_id: item_id.to_string(),
                    delta,
                };
                sess.send_event(turn_context, EventMsg::AgentMessageContentDelta(event))
                    .await;
            }
            ProposedPlanSegment::ProposedPlanStart => {
                if !state.plan_item_state.completed {
                    state.plan_item_state.start(sess, turn_context).await;
                }
            }
            ProposedPlanSegment::ProposedPlanDelta(delta) => {
                if !state.plan_item_state.completed {
                    if !state.plan_item_state.started {
                        state.plan_item_state.start(sess, turn_context).await;
                    }
                    state
                        .plan_item_state
                        .push_delta(sess, turn_context, &delta)
                        .await;
                }
            }
            ProposedPlanSegment::ProposedPlanEnd => {}
        }
    }
}

async fn emit_streamed_assistant_text_delta(
    sess: &Session,
    turn_context: &TurnContext,
    plan_mode_state: Option<&mut PlanModeStreamState>,
    item_id: &str,
    parsed: ParsedAssistantTextDelta,
) {
    if parsed.is_empty() {
        return;
    }
    if !parsed.citations.is_empty() {
        // Citation extraction is intentionally local for now; we strip citations from display text
        // but do not yet surface them in protocol events.
        let _citations = parsed.citations;
    }
    if let Some(state) = plan_mode_state {
        if !parsed.plan_segments.is_empty() {
            handle_plan_segments(sess, turn_context, state, item_id, parsed.plan_segments).await;
        }
        return;
    }
    if parsed.visible_text.is_empty() {
        return;
    }
    let event = AgentMessageContentDeltaEvent {
        thread_id: sess.thread_id.to_string(),
        turn_id: turn_context.sub_id.clone(),
        item_id: item_id.to_string(),
        delta: parsed.visible_text,
    };
    sess.send_event(turn_context, EventMsg::AgentMessageContentDelta(event))
        .await;
}

/// Flush buffered assistant text parser state when an assistant message item ends.
async fn flush_assistant_text_segments_for_item(
    sess: &Session,
    turn_context: &TurnContext,
    plan_mode_state: Option<&mut PlanModeStreamState>,
    parsers: &mut AssistantMessageStreamParsers,
    item_id: &str,
) {
    let parsed = parsers.finish_item(item_id);
    emit_streamed_assistant_text_delta(sess, turn_context, plan_mode_state, item_id, parsed).await;
}

/// Flush any remaining buffered assistant text parser state at response completion.
async fn flush_assistant_text_segments_all(
    sess: &Session,
    turn_context: &TurnContext,
    mut plan_mode_state: Option<&mut PlanModeStreamState>,
    parsers: &mut AssistantMessageStreamParsers,
) {
    for (item_id, parsed) in parsers.drain_finished() {
        emit_streamed_assistant_text_delta(
            sess,
            turn_context,
            plan_mode_state.as_deref_mut(),
            &item_id,
            parsed,
        )
        .await;
    }
}

/// Emit completion for plan items by parsing the finalized assistant message.
async fn maybe_complete_plan_item_from_message(
    sess: &Session,
    turn_context: &TurnContext,
    state: &mut PlanModeStreamState,
    item: &ResponseItem,
) {
    if let ResponseItem::Message { role, content, .. } = item
        && role == "assistant"
    {
        let mut text = String::new();
        for entry in content {
            if let ContentItem::OutputText { text: chunk } = entry {
                text.push_str(chunk);
            }
        }
        if let Some(plan_text) = extract_proposed_plan_text(&text) {
            let (plan_text, _citations) = strip_citations(&plan_text);
            if !state.plan_item_state.started {
                state.plan_item_state.start(sess, turn_context).await;
            }
            state
                .plan_item_state
                .complete_with_text(sess, turn_context, plan_text)
                .await;
        }
    }
}

/// Emit a completed agent message in plan mode, respecting deferred starts.
async fn emit_agent_message_in_plan_mode(
    sess: &Session,
    turn_context: &TurnContext,
    agent_message: codex_protocol::items::AgentMessageItem,
    state: &mut PlanModeStreamState,
) {
    let agent_message_id = agent_message.id.clone();
    let text = agent_message_text(&agent_message);
    if text.trim().is_empty() {
        state.pending_agent_message_items.remove(&agent_message_id);
        state.started_agent_message_items.remove(&agent_message_id);
        return;
    }

    maybe_emit_pending_agent_message_start(sess, turn_context, state, &agent_message_id).await;

    if !state
        .started_agent_message_items
        .contains(&agent_message_id)
    {
        let start_item = state
            .pending_agent_message_items
            .remove(&agent_message_id)
            .unwrap_or_else(|| {
                TurnItem::AgentMessage(codex_protocol::items::AgentMessageItem {
                    id: agent_message_id.clone(),
                    content: Vec::new(),
                    phase: None,
                    memory_citation: None,
                })
            });
        sess.emit_turn_item_started(turn_context, &start_item).await;
        state
            .started_agent_message_items
            .insert(agent_message_id.clone());
    }

    sess.emit_turn_item_completed(turn_context, TurnItem::AgentMessage(agent_message))
        .await;
    state.started_agent_message_items.remove(&agent_message_id);
}

/// Emit completion for a plan-mode turn item, handling agent messages specially.
async fn emit_turn_item_in_plan_mode(
    sess: &Session,
    turn_context: &TurnContext,
    turn_item: TurnItem,
    previously_active_item: Option<&TurnItem>,
    state: &mut PlanModeStreamState,
) {
    match turn_item {
        TurnItem::AgentMessage(agent_message) => {
            emit_agent_message_in_plan_mode(sess, turn_context, agent_message, state).await;
        }
        _ => {
            if previously_active_item.is_none() {
                sess.emit_turn_item_started(turn_context, &turn_item).await;
            }
            sess.emit_turn_item_completed(turn_context, turn_item).await;
        }
    }
}

/// Handle a completed assistant response item in plan mode, returning true if handled.
async fn handle_assistant_item_done_in_plan_mode(
    sess: &Session,
    turn_context: &TurnContext,
    turn_store: &codex_extension_api::ExtensionData,
    item: &ResponseItem,
    state: &mut PlanModeStreamState,
    previously_active_item: Option<&TurnItem>,
    last_agent_message: &mut Option<String>,
) -> bool {
    if let ResponseItem::Message { role, .. } = item
        && role == "assistant"
    {
        maybe_complete_plan_item_from_message(sess, turn_context, state, item).await;

        let mut finalized_facts = None;
        if let Some(finalized_turn_item) = finalize_non_tool_response_item(
            sess,
            turn_context,
            TurnItemContributorPolicy::Run(turn_store),
            item,
            /*plan_mode*/ true,
        )
        .await
        {
            finalized_facts = Some(finalized_turn_item.facts.clone());
            emit_turn_item_in_plan_mode(
                sess,
                turn_context,
                finalized_turn_item.turn_item,
                previously_active_item,
                state,
            )
            .await;
        }
        let final_last_agent_message = finalized_facts
            .as_ref()
            .and_then(|facts| facts.last_agent_message.clone());

        record_completed_response_item_with_finalized_facts(
            sess,
            turn_context,
            item,
            finalized_facts.as_ref(),
        )
        .await;
        if let Some(agent_message) = final_last_agent_message {
            *last_agent_message = Some(agent_message);
        }
        return true;
    }
    false
}

async fn drain_in_flight(
    in_flight: &mut FuturesOrdered<BoxFuture<'static, CodexResult<ResponseInputItem>>>,
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) -> CodexResult<()> {
    while let Some(res) = in_flight.next().await {
        match res {
            Ok(response_input) => {
                let response_item = response_input.into();
                sess.record_conversation_items(&turn_context, std::slice::from_ref(&response_item))
                    .await;
                mark_thread_memory_mode_polluted_if_external_context(
                    sess.as_ref(),
                    turn_context.as_ref(),
                    &response_item,
                )
                .await;
            }
            Err(err) => {
                error_or_panic(format!("in-flight tool future failed during drain: {err}"));
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[instrument(level = "trace",
    skip_all,
    fields(
        turn_id = %turn_context.sub_id,
        model = %turn_context.model_info.slug
    )
)]
async fn try_run_sampling_request(
    tool_runtime: ToolCallRuntime,
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    turn_store: Arc<codex_extension_api::ExtensionData>,
    client_session: &mut ModelClientSession,
    turn_metadata_header: Option<&str>,
    turn_diff_tracker: SharedTurnDiffTracker,
    prompt: &Prompt,
    cancellation_token: CancellationToken,
) -> CodexResult<SamplingRequestResult> {
    feedback_tags!(
        model = turn_context.model_info.slug.clone(),
        approval_policy = turn_context.approval_policy.value(),
        sandbox_policy = &turn_context.sandbox_policy(),
        effort = turn_context.reasoning_effort,
        auth_mode = sess.services.auth_manager.auth_mode(),
        features = sess.features.enabled_features(),
    );
    let inference_trace = sess.services.rollout_thread_trace.inference_trace_context(
        turn_context.sub_id.as_str(),
        turn_context.model_info.slug.as_str(),
        turn_context.provider.info().name.as_str(),
    );
    let mut stream = client_session
        .stream(
            prompt,
            &turn_context.model_info,
            &turn_context.session_telemetry,
            turn_context.reasoning_effort.clone(),
            turn_context.reasoning_summary,
            turn_context.config.service_tier.clone(),
            turn_metadata_header,
            &inference_trace,
        )
        .instrument(trace_span!("stream_request"))
        .or_cancel(&cancellation_token)
        .await??;
    let mut in_flight: FuturesOrdered<BoxFuture<'static, CodexResult<ResponseInputItem>>> =
        FuturesOrdered::new();
    let mut needs_follow_up = false;
    let mut last_agent_message: Option<String> = None;
    let mut active_item: Option<TurnItem> = None;
    let mut active_tool_argument_diff_consumer: Option<(
        String,
        Box<dyn ToolArgumentDiffConsumer>,
    )> = None;
    let mut should_emit_turn_diff = false;
    let mut should_emit_token_count = false;
    let reasoning_effort = turn_context.effective_reasoning_effort_for_tracing();
    let plan_mode = turn_context.collaboration_mode.mode == ModeKind::Plan;
    let mut assistant_message_stream_parsers = AssistantMessageStreamParsers::new(plan_mode);
    let mut plan_mode_state = plan_mode.then(|| PlanModeStreamState::new(&turn_context.sub_id));
    let defer_streamed_turn_items_for_contributors =
        !sess.services.extensions.turn_item_contributors().is_empty();
    let mut active_item_is_streaming_to_client = false;
    let receiving_span = trace_span!("receiving_stream");
    let outcome: CodexResult<SamplingRequestResult> = loop {
        let handle_responses = trace_span!(
            parent: &receiving_span,
            "handle_responses",
            otel.name = field::Empty,
            tool_name = field::Empty,
            from = field::Empty,
            codex.request.reasoning_effort = %reasoning_effort,
            gen_ai.usage.input_tokens = field::Empty,
            gen_ai.usage.cache_read.input_tokens = field::Empty,
            gen_ai.usage.output_tokens = field::Empty,
            codex.usage.reasoning_output_tokens = field::Empty,
            codex.usage.total_tokens = field::Empty,
        );

        let event = match stream
            .next()
            .instrument(trace_span!(parent: &handle_responses, "receiving"))
            .or_cancel(&cancellation_token)
            .await
        {
            Ok(event) => event,
            Err(codex_async_utils::CancelErr::Cancelled) => break Err(CodexErr::TurnAborted),
        };

        let event = match event {
            Some(Ok(event)) => event,
            Some(Err(err)) => break Err(err),
            None => {
                break Err(CodexErr::Stream(
                    "stream closed before response.completed".into(),
                    None,
                ));
            }
        };

        sess.services
            .session_telemetry
            .record_responses(&handle_responses, &event);
        record_turn_ttft_metric(&turn_context, &event).await;

        match event {
            ResponseEvent::Created => {}
            ResponseEvent::OutputItemDone(item) => {
                if let Some((_, mut consumer)) = active_tool_argument_diff_consumer.take()
                    && let Ok(Some(event)) = consumer.finish()
                {
                    sess.send_event(&turn_context, event).await;
                }
                let previously_active_item = active_item.take();
                let previously_streamed_item = if active_item_is_streaming_to_client {
                    previously_active_item
                } else {
                    None
                };
                active_item_is_streaming_to_client = false;
                if let Some(previous) = previously_streamed_item.as_ref()
                    && matches!(previous, TurnItem::AgentMessage(_))
                {
                    let item_id = previous.id();
                    flush_assistant_text_segments_for_item(
                        &sess,
                        &turn_context,
                        plan_mode_state.as_mut(),
                        &mut assistant_message_stream_parsers,
                        &item_id,
                    )
                    .await;
                }
                if let Some(state) = plan_mode_state.as_mut()
                    && handle_assistant_item_done_in_plan_mode(
                        &sess,
                        &turn_context,
                        turn_store.as_ref(),
                        &item,
                        state,
                        previously_streamed_item.as_ref(),
                        &mut last_agent_message,
                    )
                    .await
                {
                    continue;
                }

                let mut ctx = HandleOutputCtx {
                    sess: sess.clone(),
                    turn_context: turn_context.clone(),
                    turn_store: Arc::clone(&turn_store),
                    tool_runtime: tool_runtime.clone(),
                    cancellation_token: cancellation_token.child_token(),
                };

                let preempt_for_mailbox_mail = match &item {
                    ResponseItem::Message { role, phase, .. } => {
                        role == "assistant" && matches!(phase, Some(MessagePhase::Commentary))
                    }
                    ResponseItem::Reasoning { .. } => true,
                    ResponseItem::AgentMessage { .. } => false,
                    ResponseItem::LocalShellCall { .. }
                    | ResponseItem::FunctionCall { .. }
                    | ResponseItem::ToolSearchCall { .. }
                    | ResponseItem::FunctionCallOutput { .. }
                    | ResponseItem::CustomToolCall { .. }
                    | ResponseItem::CustomToolCallOutput { .. }
                    | ResponseItem::ToolSearchOutput { .. }
                    | ResponseItem::WebSearchCall { .. }
                    | ResponseItem::ImageGenerationCall { .. }
                    | ResponseItem::Compaction { .. }
                    | ResponseItem::CompactionTrigger
                    | ResponseItem::ContextCompaction { .. }
                    | ResponseItem::Other => false,
                };

                let output_result =
                    match handle_output_item_done(&mut ctx, item, previously_streamed_item)
                        .instrument(handle_responses)
                        .await
                    {
                        Ok(output_result) => output_result,
                        Err(err) => break Err(err),
                    };
                if let Some(tool_future) = output_result.tool_future {
                    in_flight.push_back(tool_future);
                }
                if let Some(agent_message) = output_result.last_agent_message {
                    last_agent_message = Some(agent_message);
                }
                needs_follow_up |= output_result.needs_follow_up;
                // todo: remove before stabilizing multi-agent v2
                if preempt_for_mailbox_mail && sess.input_queue.has_pending_mailbox_items().await {
                    break Ok(SamplingRequestResult {
                        needs_follow_up: true,
                        last_agent_message,
                    });
                }
            }
            ResponseEvent::OutputItemAdded(item) => {
                if let ResponseItem::CustomToolCall { call_id, name, .. } = &item {
                    let tool_name = ToolName::plain(name.as_str());
                    active_tool_argument_diff_consumer = tool_runtime
                        .create_diff_consumer(&tool_name)
                        .map(|consumer| (call_id.clone(), consumer));
                } else if matches!(&item, ResponseItem::FunctionCall { .. }) {
                    active_tool_argument_diff_consumer = None;
                }
                if let Some(turn_item) = handle_non_tool_response_item(
                    sess.as_ref(),
                    turn_context.as_ref(),
                    TurnItemContributorPolicy::Skip,
                    &item,
                    plan_mode,
                )
                .await
                {
                    let mut turn_item = turn_item;
                    let stream_item_to_client = !defer_streamed_turn_items_for_contributors;
                    let mut seeded_parsed: Option<ParsedAssistantTextDelta> = None;
                    let mut seeded_item_id: Option<String> = None;
                    if stream_item_to_client
                        && matches!(turn_item, TurnItem::AgentMessage(_))
                        && let Some(raw_text) = raw_assistant_output_text_from_item(&item)
                    {
                        let item_id = turn_item.id();
                        let mut seeded =
                            assistant_message_stream_parsers.seed_item_text(&item_id, &raw_text);
                        if let TurnItem::AgentMessage(agent_message) = &mut turn_item {
                            agent_message.content =
                                vec![codex_protocol::items::AgentMessageContent::Text {
                                    text: if plan_mode {
                                        String::new()
                                    } else {
                                        std::mem::take(&mut seeded.visible_text)
                                    },
                                }];
                        }
                        seeded_parsed = plan_mode.then_some(seeded);
                        seeded_item_id = Some(item_id);
                    }
                    if stream_item_to_client {
                        if let Some(state) = plan_mode_state.as_mut()
                            && matches!(turn_item, TurnItem::AgentMessage(_))
                        {
                            let item_id = turn_item.id();
                            state
                                .pending_agent_message_items
                                .insert(item_id, turn_item.clone());
                        } else {
                            sess.emit_turn_item_started(&turn_context, &turn_item).await;
                        }
                        if let (Some(state), Some(item_id), Some(parsed)) = (
                            plan_mode_state.as_mut(),
                            seeded_item_id.as_deref(),
                            seeded_parsed,
                        ) {
                            emit_streamed_assistant_text_delta(
                                &sess,
                                &turn_context,
                                Some(state),
                                item_id,
                                parsed,
                            )
                            .await;
                        }
                    }
                    active_item = Some(turn_item);
                    active_item_is_streaming_to_client = stream_item_to_client;
                }
            }
            ResponseEvent::ServerModel(server_model) => {
                if !turn_context
                    .server_model_warning_emitted
                    .load(Ordering::Relaxed)
                    && sess
                        .maybe_warn_on_server_model_mismatch(&turn_context, server_model)
                        .await
                {
                    turn_context
                        .server_model_warning_emitted
                        .store(true, Ordering::Relaxed);
                }
            }
            ResponseEvent::ModelVerifications(verifications) => {
                if !turn_context
                    .model_verification_emitted
                    .swap(true, Ordering::Relaxed)
                {
                    sess.emit_model_verification(&turn_context, verifications)
                        .await;
                }
            }
            ResponseEvent::TurnModerationMetadata(metadata) => {
                sess.emit_turn_moderation_metadata(&turn_context, metadata)
                    .await;
            }
            ResponseEvent::ServerReasoningIncluded(included) => {
                sess.set_server_reasoning_included(included).await;
            }
            ResponseEvent::RateLimits(snapshot) => {
                // Update internal state with latest rate limits, but defer sending until
                // token usage is available to avoid duplicate TokenCount events.
                sess.record_rate_limits_info(snapshot).await;
                should_emit_token_count = true;
            }
            ResponseEvent::ModelsEtag(etag) => {
                // Update internal state with latest models etag
                sess.services.models_manager.refresh_if_new_etag(etag).await;
            }
            ResponseEvent::Completed {
                token_usage,
                end_turn,
                ..
            } => {
                flush_assistant_text_segments_all(
                    &sess,
                    &turn_context,
                    plan_mode_state.as_mut(),
                    &mut assistant_message_stream_parsers,
                )
                .await;
                sess.record_token_usage_info(&turn_context, token_usage.as_ref())
                    .await;
                should_emit_token_count = true;
                should_emit_turn_diff = true;
                if let Some(false) = end_turn {
                    needs_follow_up = true;
                }
                break Ok(SamplingRequestResult {
                    needs_follow_up,
                    last_agent_message,
                });
            }
            ResponseEvent::OutputTextDelta(delta) => {
                // In review child threads, suppress assistant text deltas; the
                // UI will show a selection popup from the final ReviewOutput.
                if let Some(active) = active_item.as_ref() {
                    if !active_item_is_streaming_to_client {
                        continue;
                    }
                    let item_id = active.id();
                    if matches!(active, TurnItem::AgentMessage(_)) {
                        let parsed = assistant_message_stream_parsers.parse_delta(&item_id, &delta);
                        emit_streamed_assistant_text_delta(
                            &sess,
                            &turn_context,
                            plan_mode_state.as_mut(),
                            &item_id,
                            parsed,
                        )
                        .await;
                    } else {
                        let event = AgentMessageContentDeltaEvent {
                            thread_id: sess.thread_id.to_string(),
                            turn_id: turn_context.sub_id.clone(),
                            item_id,
                            delta,
                        };
                        sess.send_event(&turn_context, EventMsg::AgentMessageContentDelta(event))
                            .await;
                    }
                } else {
                    error_or_panic("OutputTextDelta without active item".to_string());
                }
            }
            ResponseEvent::ToolCallInputDelta {
                item_id: _,
                call_id,
                delta,
            } => {
                let Some((active_call_id, consumer)) = active_tool_argument_diff_consumer.as_mut()
                else {
                    continue;
                };
                let call_id = match call_id {
                    Some(call_id) if call_id.as_str() != active_call_id.as_str() => continue,
                    Some(call_id) => call_id,
                    None => active_call_id.clone(),
                };
                if let Some(event) = consumer.consume_diff(turn_context.as_ref(), call_id, &delta) {
                    sess.send_event(&turn_context, event).await;
                }
            }
            ResponseEvent::ReasoningSummaryDelta {
                delta,
                summary_index,
            } => {
                if let Some(active) = active_item.as_ref() {
                    if !active_item_is_streaming_to_client {
                        continue;
                    }
                    let event = ReasoningContentDeltaEvent {
                        thread_id: sess.thread_id.to_string(),
                        turn_id: turn_context.sub_id.clone(),
                        item_id: active.id(),
                        delta,
                        summary_index,
                    };
                    sess.send_event(&turn_context, EventMsg::ReasoningContentDelta(event))
                        .await;
                } else {
                    error_or_panic("ReasoningSummaryDelta without active item".to_string());
                }
            }
            ResponseEvent::ReasoningSummaryPartAdded { summary_index } => {
                if let Some(active) = active_item.as_ref() {
                    if !active_item_is_streaming_to_client {
                        continue;
                    }
                    let event =
                        EventMsg::AgentReasoningSectionBreak(AgentReasoningSectionBreakEvent {
                            item_id: active.id(),
                            summary_index,
                        });
                    sess.send_event(&turn_context, event).await;
                } else {
                    error_or_panic("ReasoningSummaryPartAdded without active item".to_string());
                }
            }
            ResponseEvent::ReasoningContentDelta {
                delta,
                content_index,
            } => {
                if let Some(active) = active_item.as_ref() {
                    if !active_item_is_streaming_to_client {
                        continue;
                    }
                    let event = ReasoningRawContentDeltaEvent {
                        thread_id: sess.thread_id.to_string(),
                        turn_id: turn_context.sub_id.clone(),
                        item_id: active.id(),
                        delta,
                        content_index,
                    };
                    sess.send_event(&turn_context, EventMsg::ReasoningRawContentDelta(event))
                        .await;
                } else {
                    error_or_panic("ReasoningRawContentDelta without active item".to_string());
                }
            }
        }
    };

    flush_assistant_text_segments_all(
        &sess,
        &turn_context,
        plan_mode_state.as_mut(),
        &mut assistant_message_stream_parsers,
    )
    .await;

    drain_in_flight(&mut in_flight, sess.clone(), turn_context.clone()).await?;

    if should_emit_token_count {
        // A tool call such as request_user_input can intentionally pause the turn. Emit token
        // counts only after pending tools resolve so clients do not see progress events while the
        // turn is waiting on the user. This also needs to happen before returning cancellation so
        // token usage already recorded from the completed response is still persisted.
        sess.send_token_count_event(&turn_context).await;
    }

    if cancellation_token.is_cancelled() {
        return Err(CodexErr::TurnAborted);
    }

    if should_emit_turn_diff {
        let unified_diff = {
            let tracker = turn_diff_tracker.lock().await;
            tracker.get_unified_diff()
        };
        if let Some(unified_diff) = unified_diff {
            let msg = EventMsg::TurnDiff(TurnDiffEvent { unified_diff });
            sess.clone().send_event(&turn_context, msg).await;
        }
    }

    outcome
}

pub(crate) fn get_last_assistant_message_from_turn(responses: &[ResponseItem]) -> Option<String> {
    for item in responses.iter().rev() {
        if let Some(message) = last_assistant_message_from_item(item, /*plan_mode*/ false) {
            return Some(message);
        }
    }
    None
}

#[cfg(test)]
#[path = "turn_tests.rs"]
mod tests;
