//! Construction and initial wiring for `ChatWidget`.

use super::*;

impl ChatWidget {
    pub(crate) fn new_with_app_event(common: ChatWidgetInit) -> Self {
        Self::new_with_op_target(common, CodexOpTarget::AppEvent)
    }

    pub(super) fn new_with_op_target(
        common: ChatWidgetInit,
        codex_op_target: CodexOpTarget,
    ) -> Self {
        let ChatWidgetInit {
            config,
            environment_manager,
            frame_requester,
            app_event_tx,
            workspace_command_runner,
            initial_user_message,
            enhanced_keys_supported,
            has_chatgpt_account,
            model_catalog,
            feedback,
            is_first_run,
            status_account_display,
            runtime_model_provider_base_url,
            initial_plan_type,
            model,
            startup_tooltip_override,
            status_line_invalid_items_warned,
            terminal_title_invalid_items_warned,
            session_telemetry,
        } = common;
        let model = model.filter(|m| !m.trim().is_empty());
        let mut config = config;
        config.model = model.clone();
        let prevent_idle_sleep = config.features.enabled(Feature::PreventIdleSleep);
        let mut rng = rand::rng();
        let placeholder = PLACEHOLDERS[rng.random_range(0..PLACEHOLDERS.len())].to_string();
        let side_placeholder =
            SIDE_PLACEHOLDERS[rng.random_range(0..SIDE_PLACEHOLDERS.len())].to_string();

        let model_override = model.as_deref();
        let model_for_header = model
            .clone()
            .unwrap_or_else(|| DEFAULT_MODEL_DISPLAY_NAME.to_string());
        let active_collaboration_mask =
            Self::initial_collaboration_mask(&config, model_catalog.as_ref(), model_override);
        let header_model = active_collaboration_mask
            .as_ref()
            .and_then(|mask| mask.model.clone())
            .unwrap_or_else(|| model_for_header.clone());
        let fallback_default = Settings {
            model: header_model.clone(),
            reasoning_effort: None,
            developer_instructions: None,
        };
        // Collaboration modes start in Default mode.
        let current_collaboration_mode = CollaborationMode {
            mode: ModeKind::Default,
            settings: fallback_default,
        };

        let active_cell = Some(Self::placeholder_session_header_cell(&config));

        let current_cwd = Some(config.cwd.to_path_buf());
        let effective_service_tier = config.service_tier.clone();
        let current_terminal_info = terminal_info();
        let runtime_keymap = RuntimeKeymap::from_config(&config.tui_keymap).ok();
        let default_keymap = RuntimeKeymap::defaults();
        let copy_last_response_binding = runtime_keymap
            .as_ref()
            .map(|keymap| keymap.app.copy.clone())
            .unwrap_or_else(|| default_keymap.app.copy.clone());
        let chat_keymap = runtime_keymap
            .as_ref()
            .map(|keymap| keymap.chat.clone())
            .unwrap_or_else(|| default_keymap.chat.clone());
        let queued_message_edit_hint_binding = queued_message_edit_hint_binding(
            &chat_keymap.edit_queued_message,
            current_terminal_info,
        );
        pets::start_configured_pet_load_if_needed(
            &config,
            /*ambient_pet_missing*/ true,
            frame_requester.clone(),
            app_event_tx.clone(),
        );
        let mut widget = Self {
            app_event_tx: app_event_tx.clone(),
            frame_requester: frame_requester.clone(),
            codex_op_target,
            bottom_pane: BottomPane::new(BottomPaneParams {
                frame_requester,
                app_event_tx,
                has_input_focus: true,
                enhanced_keys_supported,
                placeholder_text: placeholder.clone(),
                disable_paste_burst: config.disable_paste_burst,
                animations_enabled: config.animations,
                skills: None,
            }),
            transcript: TranscriptState::new(active_cell),
            raw_output_mode: config.tui_raw_output_mode,
            config,
            environment_manager,
            effective_service_tier,
            skills_all: Vec::new(),
            skills_initial_state: None,
            current_collaboration_mode,
            active_collaboration_mask,
            has_chatgpt_account,
            model_catalog,
            session_telemetry,
            session_header: SessionHeader::new(header_model),
            initial_user_message,
            status_account_display,
            runtime_model_provider_base_url,
            token_info: None,
            rate_limit_snapshots_by_limit_id: BTreeMap::new(),
            refreshing_status_outputs: Vec::new(),
            next_status_refresh_request_id: 0,
            plan_type: initial_plan_type,
            codex_rate_limit_reached_type: None,
            rate_limit_warnings: RateLimitWarningState::default(),
            warning_display_state: WarningDisplayState::default(),
            rate_limit_switch_prompt: RateLimitSwitchPromptState::default(),
            add_credits_nudge_email_in_flight: None,
            adaptive_chunking: AdaptiveChunkingPolicy::default(),
            stream_controller: None,
            plan_stream_controller: None,
            clipboard_lease: None,
            copy_last_response_binding,
            running_commands: HashMap::new(),
            collab_agent_metadata: HashMap::new(),
            pending_collab_spawn_requests: HashMap::new(),
            suppressed_exec_calls: HashSet::new(),
            last_unified_wait: None,
            unified_exec_wait_streak: None,
            turn_lifecycle: TurnLifecycleState::new(prevent_idle_sleep),
            task_complete_pending: false,
            unified_exec_processes: Vec::new(),
            mcp_startup_status: None,
            mcp_startup_expected_servers: None,
            mcp_startup_ignore_updates_until_next_start: false,
            mcp_startup_allow_terminal_only_next_round: false,
            mcp_startup_pending_next_round: HashMap::new(),
            mcp_startup_pending_next_round_saw_starting: false,
            connectors: ConnectorsState::default(),
            ide_context: IdeContextState::default(),
            plugins_cache: PluginsCacheState::default(),
            plugins_fetch_state: PluginListFetchState::default(),
            plugin_install_apps_needing_auth: Vec::new(),
            plugin_install_auth_flow: None,
            plugins_active_tab_id: None,
            newly_installed_marketplace_tab_id: None,
            interrupts: InterruptManager::new(),
            reasoning_buffer: String::new(),
            full_reasoning_buffer: String::new(),
            status_state: StatusState::default(),
            review: ReviewState::default(),
            active_hook_cell: None,
            ambient_pet: None,
            pet_picker_preview_state: crate::pets::PetPickerPreviewState::default(),
            pet_picker_preview_pet: None,
            pet_picker_preview_request_id: 0,
            pet_picker_preview_image_visible: std::cell::Cell::new(/*value*/ false),
            pet_selection_load_request_id: 0,
            #[cfg(test)]
            pet_image_support_override: None,
            thread_id: None,
            dismissed_plan_mode_nudge_scopes: HashSet::new(),
            thread_name: None,
            thread_rename_block_message: None,
            active_side_conversation: false,
            normal_placeholder_text: placeholder,
            side_placeholder_text: side_placeholder,
            forked_from: None,
            interrupted_turn_notice_mode: InterruptedTurnNoticeMode::Default,
            input_queue: InputQueueState::default(),
            chat_keymap,
            queued_message_edit_hint_binding,
            show_welcome_banner: is_first_run,
            startup_tooltip_override,
            suppress_session_configured_redraw: false,
            suppress_initial_user_message_submit: false,
            pending_notification: None,
            quit_shortcut_expires_at: None,
            quit_shortcut_key: None,
            turn_runtime_metrics: RuntimeMetricsSummary::default(),
            last_rendered_width: std::cell::Cell::new(None),
            feedback,
            current_rollout_path: None,
            current_cwd,
            workspace_command_runner,
            instruction_source_paths: Vec::new(),
            session_network_proxy: None,
            status_line_invalid_items_warned,
            terminal_title_invalid_items_warned,
            last_terminal_title: None,
            last_terminal_title_requires_action: false,
            terminal_title_setup_original_items: None,
            terminal_title_animation_origin: Instant::now(),
            status_line_project_root_name_cache: None,
            status_line_branch: None,
            status_line_branch_cwd: None,
            status_line_branch_pending: false,
            status_line_branch_lookup_complete: false,
            status_line_git_summary: None,
            status_line_git_summary_cwd: None,
            status_line_git_summary_pending: false,
            status_line_git_summary_lookup_complete: false,
            current_goal_status_indicator: None,
            current_goal_status: None,
            external_editor_state: ExternalEditorState::Closed,
            realtime_conversation: RealtimeConversationUiState::default(),
            last_rendered_user_message_display: None,
            last_non_retry_error: None,
        };

        widget.prefetch_rate_limits();
        if let Some(keymap) = runtime_keymap {
            widget.bottom_pane.set_keymap_bindings(&keymap);
        }
        widget
            .bottom_pane
            .set_vim_enabled(widget.config.tui_vim_mode_default);
        widget
            .bottom_pane
            .set_realtime_conversation_enabled(widget.realtime_conversation_enabled());
        widget
            .bottom_pane
            .set_audio_device_selection_enabled(widget.realtime_audio_device_selection_enabled());
        widget
            .bottom_pane
            .set_status_line_enabled(!widget.configured_status_line_items().is_empty());
        widget
            .bottom_pane
            .set_collaboration_modes_enabled(/*enabled*/ true);
        widget.sync_service_tier_commands();
        widget.sync_personality_command_enabled();
        widget.sync_plugins_command_enabled();
        widget.sync_goal_command_enabled();
        widget.sync_mentions_v2_enabled();
        widget
            .bottom_pane
            .set_queued_message_edit_binding(widget.queued_message_edit_hint_binding);
        #[cfg(target_os = "windows")]
        widget.bottom_pane.set_windows_degraded_sandbox_active(
            crate::legacy_core::windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED
                && matches!(
                    WindowsSandboxLevel::from_config(&widget.config),
                    WindowsSandboxLevel::RestrictedToken
                ),
        );
        widget.update_collaboration_mode_indicator();

        widget
            .bottom_pane
            .set_connectors_enabled(widget.connectors_enabled());
        widget.refresh_status_surfaces();

        widget
    }
}
