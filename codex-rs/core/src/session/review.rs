use super::step_settings::ResolvedStepSettings;
use super::*;

/// Spawn a review thread using the given prompt.
pub(super) async fn spawn_review_thread(
    sess: Arc<Session>,
    config: Arc<Config>,
    parent_turn_context: Arc<TurnContext>,
    sub_id: String,
    resolved: crate::review_prompts::ResolvedReviewRequest,
) {
    let model = config
        .review_model
        .clone()
        .unwrap_or_else(|| parent_turn_context.model_info().slug.clone());
    let available_models = sess
        .services
        .models_manager
        .list_models(
            RefreshStrategy::OnlineIfUncached,
            config.http_client_factory(),
        )
        .await;
    let review_model_info = sess
        .services
        .models_manager
        .get_model_info(&model, &config.to_models_manager_config())
        .await;
    // For reviews, disable web_search and view_image regardless of global settings.
    let mut review_features = sess.features.clone();
    let _ = review_features.disable(Feature::WebSearchRequest);
    let _ = review_features.disable(Feature::WebSearchCached);
    let _ = review_features.disable(Feature::Goals);
    let review_web_search_mode = WebSearchMode::Disabled;
    let unified_exec_shell_mode = UnifiedExecShellMode::for_session(
        review_features.get(),
        crate::tools::tool_user_shell_type(sess.services.user_shell.as_ref()),
        sess.services.shell_zsh_path.as_ref(),
        sess.services.main_execve_wrapper_exe.as_ref(),
    );

    let review_prompt = resolved.prompt.clone();
    let model_info = review_model_info.clone();
    let mut selected = parent_turn_context.initial_settings.selected().clone();
    let mut reasoning_effort = selected.collaboration_mode.reasoning_effort();

    // Build per‑turn client with the requested model/family.
    let mut per_turn_config = (*parent_turn_context.config).clone();
    // Preserve configured overrides without carrying over the parent model's defaults.
    per_turn_config.token_budget = config.token_budget.clone();
    per_turn_config.features = review_features.clone();
    if let Some(current_effort) = reasoning_effort.as_ref()
        && review_model_info.slug != parent_turn_context.model_info().slug
        && !review_model_info.used_fallback_model_metadata
        && !review_model_info
            .supported_reasoning_levels
            .iter()
            .any(|preset| &preset.effort == current_effort)
    {
        let supported_reasoning_levels = &review_model_info.supported_reasoning_levels;
        reasoning_effort = supported_reasoning_levels
            .get(supported_reasoning_levels.len().saturating_sub(1) / 2)
            .map(|preset| preset.effort.clone())
            .or_else(|| review_model_info.default_reasoning_level.clone());
    }
    if let Err(err) = per_turn_config.web_search_mode.set(review_web_search_mode) {
        let fallback_value = per_turn_config.web_search_mode.value();
        tracing::warn!(
            error = %err,
            ?review_web_search_mode,
            ?fallback_value,
            "review web_search_mode is disallowed by requirements; keeping constrained value"
        );
    }

    let session_source = parent_turn_context.session_source.clone();
    let (forked_from_thread_id, thread_source, service_tier) = {
        let state = sess.state.lock().await;
        (
            state.session_configuration.forked_from_thread_id,
            state.session_configuration.thread_source.clone(),
            state
                .session_configuration
                .step_settings
                .service_tier
                .clone()
                .or_else(|| config.service_tier.clone()),
        )
    };
    let auto_review_enabled = crate::guardian::routes_approval_policy_to_guardian(
        per_turn_config.permissions.approval_policy.value(),
        per_turn_config.approvals_reviewer,
    );
    // The original config may retain a requested tier unsupported by the parent
    // model. Resolve it against the review model and the shared feature gate.
    selected.collaboration_mode.settings.model = model.clone();
    selected.collaboration_mode.settings.reasoning_effort = reasoning_effort.clone();
    selected.service_tier = service_tier;
    let step_settings = Arc::new(ResolvedStepSettings::new(
        Arc::new(selected),
        Arc::new(model_info.clone()),
        review_features.enabled(Feature::FastMode),
    ));
    per_turn_config.model = Some(model);
    per_turn_config.model_reasoning_effort = reasoning_effort;
    per_turn_config.service_tier = step_settings.service_tier.clone();
    let per_turn_config = Arc::new(per_turn_config);
    let review_turn_id = sub_id.to_string();
    #[allow(deprecated)]
    let windows_sandbox_selection = parent_turn_context
        .initial_environments
        .primary()
        .map(TurnEnvironment::windows_sandbox_selection_for_turn_metadata)
        .unwrap_or_else(|| {
            crate::tools::sandboxing::configured_windows_sandbox_selection(
                parent_turn_context
                    .config
                    .effective_local_windows_sandbox_type(),
                parent_turn_context.windows_sandbox_level,
                &codex_utils_path_uri::PathUri::from_abs_path(&parent_turn_context.cwd),
            )
        });
    let turn_metadata_state = Arc::new(TurnMetadataState::new(
        sess.session_id().to_string(),
        sess.thread_id().to_string(),
        forked_from_thread_id,
        parent_turn_context.parent_thread_id,
        &session_source,
        thread_source,
        review_turn_id.clone(),
        #[allow(deprecated)]
        parent_turn_context.cwd.clone(),
        &parent_turn_context.permission_profile(),
        windows_sandbox_selection,
        parent_turn_context.network.is_some(),
        auto_review_enabled,
        &model_info,
    ));

    let review_turn_context = parent_turn_context.for_review(
        review_turn_id,
        per_turn_config,
        step_settings,
        available_models,
        unified_exec_shell_mode,
        turn_metadata_state,
    );

    // Seed the child task with the review prompt as the initial user message.
    let input = vec![TurnInput::UserInput {
        metadata: Default::default(),
        content: vec![UserInput::Text {
            text: review_prompt,
            // Review prompt is synthesized; no UI element ranges to preserve.
            text_elements: Vec::new(),
        }],
        client_id: None,
    }];
    let tc = Arc::new(review_turn_context);
    if tc
        .initial_environments
        .single_local_environment_cwd()
        .is_some()
    {
        tc.turn_metadata_state
            .spawn_git_enrichment_task(Arc::clone(&sess.services.git_root_discovery));
    }
    // TODO(ccunningham): Review turns currently rely on `spawn_task` for TurnComplete but do not
    // emit a parent TurnStarted. Consider giving review a full parent turn lifecycle
    // (TurnStarted + TurnComplete) for consistency with other standalone tasks.
    sess.spawn_task(Arc::clone(&tc), input, ReviewTask::new())
        .await;

    // Announce entering review mode so UIs can switch modes.
    let item = TurnItem::EnteredReviewMode(EnteredReviewModeItem {
        id: uuid::Uuid::now_v7().to_string(),
        target: resolved.target,
        user_facing_hint: resolved.user_facing_hint,
    });
    sess.emit_turn_item_started(&tc, &item).await;
    sess.emit_turn_item_completed(&tc, item).await;
}
