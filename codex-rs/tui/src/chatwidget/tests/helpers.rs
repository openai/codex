use super::*;
use codex_app_server_protocol::PluginAvailability;
use pretty_assertions::assert_eq;

pub(super) async fn test_config() -> Config {
    // Start from the built-in defaults so tests do not inherit host/system config.
    let codex_home = tempfile::Builder::new()
        .prefix("chatwidget-tests-")
        .tempdir()
        .expect("tempdir")
        .keep();
    let mut config =
        Config::load_default_with_cli_overrides_for_codex_home(codex_home.clone(), Vec::new())
            .await
            .expect("config");
    config.codex_home = codex_home.abs();
    config.sqlite_home = codex_home.clone();
    config.log_dir = codex_home.join("log");
    config.cwd = PathBuf::from(test_path_display("/tmp/project")).abs();
    config.config_layer_stack = ConfigLayerStack::default();
    config.startup_warnings.clear();
    config.user_instructions = None;
    config
}

pub(super) fn test_project_path() -> PathBuf {
    PathBuf::from(test_path_display("/tmp/project"))
}

pub(super) fn truncated_path_variants(path: &str) -> Vec<String> {
    let chars: Vec<char> = path.chars().collect();
    (1..chars.len())
        .map(|len| chars[..len].iter().collect::<String>())
        .collect()
}

pub(super) fn normalize_snapshot_paths(text: impl Into<String>) -> String {
    let mut text = text.into();

    for unix_path in ["/tmp/project", "/tmp/hooks.json"] {
        let platform_path = test_path_display(unix_path);
        if platform_path != unix_path {
            text = text.replace(&platform_path, unix_path);
        }
    }

    let platform_test_cwd = test_path_display("/tmp/project");
    if platform_test_cwd == "/tmp/project" {
        text
    } else {
        for platform_prefix in truncated_path_variants(&platform_test_cwd)
            .into_iter()
            .rev()
        {
            let unix_prefix: String = "/tmp/project"
                .chars()
                .take(platform_prefix.chars().count())
                .collect();
            text = text.replace(&format!("{platform_prefix}…"), &format!("{unix_prefix}…"));
        }

        text
    }
}

pub(super) fn normalized_backend_snapshot<T: std::fmt::Display>(value: &T) -> String {
    let platform_test_cwd = test_path_display("/tmp/project");
    let rendered = format!("{value}");

    if platform_test_cwd == "/tmp/project" {
        return rendered;
    }

    rendered
        .lines()
        .map(|line| {
            if let Some(content) = line
                .strip_prefix('"')
                .and_then(|line| line.strip_suffix('"'))
            {
                let width = content.chars().count();
                let normalized = normalize_snapshot_paths(content);
                format!("\"{normalized:width$}\"")
            } else {
                normalize_snapshot_paths(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn invalid_value(
    candidate: impl Into<String>,
    allowed: impl Into<String>,
) -> ConstraintError {
    ConstraintError::InvalidValue {
        field_name: "<unknown>",
        candidate: candidate.into(),
        allowed: allowed.into(),
        requirement_source: RequirementSource::Unknown,
    }
}

pub(super) fn snapshot(percent: f64) -> RateLimitSnapshot {
    RateLimitSnapshot {
        limit_id: None,
        limit_name: None,
        primary: Some(RateLimitWindow {
            used_percent: percent.round() as i32,
            window_duration_mins: Some(60),
            resets_at: None,
        }),
        secondary: None,
        credits: None,
        plan_type: None,
        rate_limit_reached_type: None,
    }
}

pub(super) fn test_session_telemetry(config: &Config, model: &str) -> SessionTelemetry {
    let model_info = crate::legacy_core::test_support::construct_model_info_offline(model, config);
    SessionTelemetry::new(
        ThreadId::new(),
        model,
        model_info.slug.as_str(),
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "test_originator".to_string(),
        /*log_user_prompts*/ false,
        "test".to_string(),
        crate::test_support::session_source_cli(),
    )
}

pub(super) fn test_model_catalog(_config: &Config) -> Arc<ModelCatalog> {
    Arc::new(ModelCatalog::new(
        crate::legacy_core::test_support::all_model_presets().clone(),
    ))
}

// --- Helpers for tests that need direct construction and event draining ---
pub(super) async fn make_chatwidget_manual(
    model_override: Option<&str>,
) -> (
    ChatWidget,
    tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    tokio::sync::mpsc::UnboundedReceiver<Op>,
) {
    let (tx_raw, rx) = unbounded_channel::<AppEvent>();
    let app_event_tx = AppEventSender::new(tx_raw);
    let (op_tx, op_rx) = unbounded_channel::<Op>();
    let mut cfg = test_config().await;
    let resolved_model = model_override.map(str::to_owned).unwrap_or_else(|| {
        crate::legacy_core::test_support::get_model_offline(cfg.model.as_deref())
    });
    if let Some(model) = model_override {
        cfg.model = Some(model.to_string());
    }
    let prevent_idle_sleep = cfg.features.enabled(Feature::PreventIdleSleep);
    let session_telemetry = test_session_telemetry(&cfg, resolved_model.as_str());
    let mut bottom = BottomPane::new(BottomPaneParams {
        app_event_tx: app_event_tx.clone(),
        frame_requester: FrameRequester::test_dummy(),
        has_input_focus: true,
        enhanced_keys_supported: false,
        placeholder_text: "Ask Codex to do anything".to_string(),
        disable_paste_burst: false,
        animations_enabled: cfg.animations,
        skills: None,
    });
    bottom.set_collaboration_modes_enabled(/*enabled*/ true);
    let model_catalog = test_model_catalog(&cfg);
    let reasoning_effort = None;
    let base_mode = CollaborationMode {
        mode: ModeKind::Default,
        settings: Settings {
            model: resolved_model.clone(),
            reasoning_effort,
            developer_instructions: None,
        },
    };
    let current_collaboration_mode = base_mode;
    let active_collaboration_mask = collaboration_modes::default_mask(model_catalog.as_ref());
    let effective_service_tier = cfg
        .service_tier
        .as_deref()
        .and_then(ServiceTier::from_request_value);
    let mut widget = ChatWidget {
        app_event_tx,
        codex_op_target: super::CodexOpTarget::Direct(op_tx),
        bottom_pane: bottom,
        active_cell: None,
        active_cell_revision: 0,
        raw_output_mode: cfg.tui_raw_output_mode,
        config: cfg,
        effective_service_tier,
        current_collaboration_mode,
        active_collaboration_mask,
        has_chatgpt_account: false,
        model_catalog,
        session_telemetry,
        session_header: SessionHeader::new(resolved_model.clone()),
        initial_user_message: None,
        status_account_display: None,
        runtime_model_provider_base_url: None,
        token_info: None,
        rate_limit_snapshots_by_limit_id: BTreeMap::new(),
        refreshing_status_outputs: Vec::new(),
        next_status_refresh_request_id: 0,
        plan_type: None,
        codex_rate_limit_reached_type: None,
        rate_limit_warnings: RateLimitWarningState::default(),
        warning_display_state: WarningDisplayState::default(),
        rate_limit_switch_prompt: RateLimitSwitchPromptState::default(),
        add_credits_nudge_email_in_flight: None,
        adaptive_chunking: crate::streaming::chunking::AdaptiveChunkingPolicy::default(),
        stream_controller: None,
        plan_stream_controller: None,
        clipboard_lease: None,
        copy_last_response_binding: crate::keymap::RuntimeKeymap::defaults().app.copy,
        pending_guardian_review_status: PendingGuardianReviewStatus::default(),
        recent_auto_review_denials: RecentAutoReviewDenials::default(),
        terminal_title_status_kind: TerminalTitleStatusKind::Working,
        last_agent_markdown: None,
        agent_turn_markdowns: Vec::new(),
        visible_user_turn_count: 0,
        copy_history_evicted_by_rollback: false,
        latest_proposed_plan_markdown: None,
        saw_copy_source_this_turn: false,
        running_commands: HashMap::new(),
        collab_agent_metadata: HashMap::new(),
        pending_collab_spawn_requests: HashMap::new(),
        suppressed_exec_calls: HashSet::new(),
        skills_all: Vec::new(),
        skills_initial_state: None,
        last_unified_wait: None,
        unified_exec_wait_streak: None,
        turn_sleep_inhibitor: SleepInhibitor::new(prevent_idle_sleep),
        task_complete_pending: false,
        unified_exec_processes: Vec::new(),
        agent_turn_running: false,
        mcp_startup_status: None,
        mcp_startup_expected_servers: None,
        mcp_startup_ignore_updates_until_next_start: false,
        mcp_startup_allow_terminal_only_next_round: false,
        mcp_startup_pending_next_round: HashMap::new(),
        mcp_startup_pending_next_round_saw_starting: false,
        connectors_cache: ConnectorsCacheState::default(),
        connectors_partial_snapshot: None,
        plugin_install_apps_needing_auth: Vec::new(),
        plugin_install_auth_flow: None,
        plugins_active_tab_id: None,
        newly_installed_marketplace_tab_id: None,
        connectors_prefetch_in_flight: false,
        connectors_force_refetch_pending: false,
        ide_context: super::super::ide_context::IdeContextState::default(),
        plugins_cache: PluginsCacheState::default(),
        plugins_fetch_state: PluginListFetchState::default(),
        interrupts: InterruptManager::new(),
        reasoning_buffer: String::new(),
        full_reasoning_buffer: String::new(),
        current_status: StatusIndicatorState::working(),
        active_hook_cell: None,
        retry_status_header: None,
        pending_status_indicator_restore: false,
        suppress_queue_autosend: false,
        thread_id: None,
        dismissed_plan_mode_nudge_scopes: HashSet::new(),
        last_turn_id: None,
        budget_limited_turn_ids: HashSet::new(),
        thread_name: None,
        thread_rename_block_message: None,
        active_side_conversation: false,
        normal_placeholder_text: "Ask Codex to do anything".to_string(),
        side_placeholder_text: "Check recently modified functions for compatibility".to_string(),
        forked_from: None,
        interrupted_turn_notice_mode: InterruptedTurnNoticeMode::Default,
        frame_requester: FrameRequester::test_dummy(),
        show_welcome_banner: true,
        startup_tooltip_override: None,
        queued_user_messages: VecDeque::new(),
        queued_user_message_history_records: VecDeque::new(),
        user_turn_pending_start: false,
        rejected_steers_queue: VecDeque::new(),
        rejected_steer_history_records: VecDeque::new(),
        pending_steers: VecDeque::new(),
        submit_pending_steers_after_interrupt: false,
        chat_keymap: crate::keymap::RuntimeKeymap::defaults().chat,
        queued_message_edit_hint_binding: Some(crate::key_hint::alt(KeyCode::Up)),
        suppress_session_configured_redraw: false,
        suppress_initial_user_message_submit: false,
        pending_notification: None,
        quit_shortcut_expires_at: None,
        quit_shortcut_key: None,
        is_review_mode: false,
        pre_review_token_info: None,
        needs_final_message_separator: false,
        had_work_activity: false,
        saw_plan_update_this_turn: false,
        saw_plan_item_this_turn: false,
        last_plan_progress: None,
        plan_delta_buffer: String::new(),
        plan_item_active: false,
        turn_runtime_metrics: RuntimeMetricsSummary::default(),
        last_rendered_width: std::cell::Cell::new(None),
        feedback: codex_feedback::CodexFeedback::new(),
        current_rollout_path: None,
        current_cwd: None,
        workspace_command_runner: None,
        instruction_source_paths: Vec::new(),
        session_network_proxy: None,
        status_line_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        terminal_title_invalid_items_warned: Arc::new(AtomicBool::new(false)),
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
        goal_status_active_turn_started_at: None,
        external_editor_state: ExternalEditorState::Closed,
        realtime_conversation: RealtimeConversationUiState::default(),
        last_rendered_user_message_display: None,
        last_non_retry_error: None,
    };
    widget.set_model(&resolved_model);
    (widget, rx, op_rx)
}

// ChatWidget may emit other `Op`s (e.g. history/logging updates) on the same channel; this helper
// filters until we see a submission op.
pub(super) fn next_submit_op(op_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Op>) -> Op {
    loop {
        match op_rx.try_recv() {
            Ok(op @ Op::UserTurn { .. }) => return op,
            Ok(_) => continue,
            Err(TryRecvError::Empty) => panic!("expected a submit op but queue was empty"),
            Err(TryRecvError::Disconnected) => panic!("expected submit op but channel closed"),
        }
    }
}

pub(super) fn next_interrupt_op(op_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Op>) {
    loop {
        match op_rx.try_recv() {
            Ok(Op::Interrupt) => return,
            Ok(_) => continue,
            Err(TryRecvError::Empty) => panic!("expected interrupt op but queue was empty"),
            Err(TryRecvError::Disconnected) => panic!("expected interrupt op but channel closed"),
        }
    }
}

pub(super) fn next_realtime_close_op(op_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Op>) {
    loop {
        match op_rx.try_recv() {
            Ok(Op::RealtimeConversationClose) => return,
            Ok(_) => continue,
            Err(TryRecvError::Empty) => {
                panic!("expected realtime close op but queue was empty")
            }
            Err(TryRecvError::Disconnected) => {
                panic!("expected realtime close op but channel closed")
            }
        }
    }
}

pub(super) fn assert_no_submit_op(op_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Op>) {
    while let Ok(op) = op_rx.try_recv() {
        assert!(
            !matches!(op, Op::UserTurn { .. }),
            "unexpected submit op: {op:?}"
        );
    }
}

pub(crate) fn set_chatgpt_auth(chat: &mut ChatWidget) {
    chat.has_chatgpt_account = true;
    chat.model_catalog = test_model_catalog(&chat.config);
}

fn test_model_info(slug: &str, priority: i32, supports_fast_mode: bool) -> ModelInfo {
    let additional_speed_tiers = if supports_fast_mode {
        vec![codex_protocol::openai_models::SPEED_TIER_FAST]
    } else {
        Vec::new()
    };
    serde_json::from_value(json!({
        "slug": slug,
        "display_name": slug,
        "description": format!("{slug} description"),
        "default_reasoning_level": "medium",
        "supported_reasoning_levels": [{"effort": "medium", "description": "medium"}],
        "shell_type": "shell_command",
        "visibility": "list",
        "supported_in_api": true,
        "priority": priority,
        "additional_speed_tiers": additional_speed_tiers,
        "availability_nux": null,
        "upgrade": null,
        "base_instructions": "base instructions",
        "supports_reasoning_summaries": false,
        "default_reasoning_summary": "none",
        "support_verbosity": false,
        "default_verbosity": null,
        "apply_patch_tool_type": null,
        "truncation_policy": {"mode": "bytes", "limit": 10_000},
        "supports_parallel_tool_calls": false,
        "supports_image_detail_original": false,
        "context_window": 272_000,
        "experimental_supported_tools": [],
    }))
    .expect("valid model info")
}

pub(crate) fn set_fast_mode_test_catalog(chat: &mut ChatWidget) {
    let models: Vec<ModelPreset> = ModelsResponse {
        models: vec![
            test_model_info(
                "gpt-5.4", /*priority*/ 0, /*supports_fast_mode*/ true,
            ),
            test_model_info(
                "gpt-5.3-codex",
                /*priority*/ 1,
                /*supports_fast_mode*/ false,
            ),
        ],
    }
    .models
    .into_iter()
    .map(Into::into)
    .collect();

    chat.model_catalog = Arc::new(ModelCatalog::new(models));
}

pub(crate) async fn make_chatwidget_manual_with_sender() -> (
    ChatWidget,
    AppEventSender,
    tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    tokio::sync::mpsc::UnboundedReceiver<Op>,
) {
    let (widget, rx, op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let app_event_tx = widget.app_event_tx.clone();
    (widget, app_event_tx, rx, op_rx)
}

pub(super) fn drain_insert_history(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> Vec<Vec<ratatui::text::Line<'static>>> {
    let mut out = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = ev {
            let mut lines = cell.display_lines(/*width*/ 80);
            if !cell.is_stream_continuation() && !out.is_empty() && !lines.is_empty() {
                lines.insert(0, "".into());
            }
            out.push(lines)
        }
    }
    out
}

pub(super) fn lines_to_single_string(lines: &[ratatui::text::Line<'static>]) -> String {
    let mut s = String::new();
    for line in lines {
        for span in &line.spans {
            s.push_str(&span.content);
        }
        s.push('\n');
    }
    s
}

pub(super) fn status_line_text(chat: &ChatWidget) -> Option<String> {
    chat.status_line_text()
}

pub(super) fn make_token_info(total_tokens: i64, context_window: i64) -> TokenUsageInfo {
    fn usage(total_tokens: i64) -> TokenUsage {
        TokenUsage {
            total_tokens,
            ..TokenUsage::default()
        }
    }

    TokenUsageInfo {
        total_token_usage: usage(total_tokens),
        last_token_usage: usage(total_tokens),
        model_context_window: Some(context_window),
    }
}

fn thread_id(chat: &ChatWidget) -> String {
    chat.thread_id.map(|id| id.to_string()).unwrap_or_default()
}

fn token_usage_breakdown(usage: TokenUsage) -> codex_app_server_protocol::TokenUsageBreakdown {
    codex_app_server_protocol::TokenUsageBreakdown {
        total_tokens: usage.total_tokens,
        input_tokens: usage.input_tokens,
        cached_input_tokens: usage.cached_input_tokens,
        output_tokens: usage.output_tokens,
        reasoning_output_tokens: usage.reasoning_output_tokens,
    }
}

pub(super) fn handle_token_count(chat: &mut ChatWidget, info: Option<TokenUsageInfo>) {
    match info {
        Some(info) => {
            chat.handle_server_notification(
                ServerNotification::ThreadTokenUsageUpdated(
                    codex_app_server_protocol::ThreadTokenUsageUpdatedNotification {
                        thread_id: thread_id(chat),
                        turn_id: chat
                            .last_turn_id
                            .clone()
                            .unwrap_or_else(|| "turn-1".to_string()),
                        token_usage: codex_app_server_protocol::ThreadTokenUsage {
                            total: token_usage_breakdown(info.total_token_usage),
                            last: token_usage_breakdown(info.last_token_usage),
                            model_context_window: info.model_context_window,
                        },
                    },
                ),
                /*replay_kind*/ None,
            );
        }
        None => chat.set_token_info(/*info*/ None),
    }
}

pub(super) fn handle_error(
    chat: &mut ChatWidget,
    message: impl Into<String>,
    codex_error_info: Option<CodexErrorInfo>,
) {
    chat.handle_server_notification(
        ServerNotification::Error(ErrorNotification {
            error: AppServerTurnError {
                message: message.into(),
                codex_error_info,
                additional_details: None,
            },
            will_retry: false,
            thread_id: thread_id(chat),
            turn_id: chat
                .last_turn_id
                .clone()
                .unwrap_or_else(|| "turn-1".to_string()),
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn handle_stream_error(
    chat: &mut ChatWidget,
    message: impl Into<String>,
    additional_details: Option<String>,
) {
    handle_stream_error_with_replay(chat, message, additional_details, /*replay_kind*/ None);
}

pub(super) fn handle_stream_error_with_replay(
    chat: &mut ChatWidget,
    message: impl Into<String>,
    additional_details: Option<String>,
    replay_kind: Option<ReplayKind>,
) {
    chat.handle_server_notification(
        ServerNotification::Error(ErrorNotification {
            error: AppServerTurnError {
                message: message.into(),
                codex_error_info: None,
                additional_details,
            },
            will_retry: true,
            thread_id: thread_id(chat),
            turn_id: chat
                .last_turn_id
                .clone()
                .unwrap_or_else(|| "turn-1".to_string()),
        }),
        replay_kind,
    );
}

pub(super) fn handle_warning(chat: &mut ChatWidget, message: impl Into<String>) {
    chat.handle_server_notification(
        ServerNotification::Warning(WarningNotification {
            thread_id: Some(thread_id(chat)),
            message: message.into(),
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn handle_model_verification(
    chat: &mut ChatWidget,
    verifications: Vec<AppServerModelVerification>,
) {
    chat.handle_server_notification(
        ServerNotification::ModelVerification(ModelVerificationNotification {
            thread_id: thread_id(chat),
            turn_id: chat
                .last_turn_id
                .clone()
                .unwrap_or_else(|| "turn-1".to_string()),
            verifications,
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn handle_agent_message_delta(chat: &mut ChatWidget, delta: impl Into<String>) {
    chat.handle_server_notification(
        ServerNotification::AgentMessageDelta(
            codex_app_server_protocol::AgentMessageDeltaNotification {
                thread_id: thread_id(chat),
                turn_id: chat
                    .last_turn_id
                    .clone()
                    .unwrap_or_else(|| "turn-1".to_string()),
                item_id: "msg-1".to_string(),
                delta: delta.into(),
            },
        ),
        /*replay_kind*/ None,
    );
}

pub(super) fn handle_agent_reasoning_delta(chat: &mut ChatWidget, delta: impl Into<String>) {
    chat.handle_server_notification(
        ServerNotification::ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaNotification {
            thread_id: thread_id(chat),
            turn_id: chat
                .last_turn_id
                .clone()
                .unwrap_or_else(|| "turn-1".to_string()),
            item_id: "reasoning-1".to_string(),
            delta: delta.into(),
            summary_index: 0,
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn handle_agent_reasoning_final(chat: &mut ChatWidget) {
    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: thread_id(chat),
            turn_id: chat
                .last_turn_id
                .clone()
                .unwrap_or_else(|| "turn-1".to_string()),
            completed_at_ms: 0,
            item: AppServerThreadItem::Reasoning {
                id: "reasoning-1".to_string(),
                summary: Vec::new(),
                content: Vec::new(),
            },
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn handle_entered_review_mode(chat: &mut ChatWidget, review: impl Into<String>) {
    chat.handle_server_notification(
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: thread_id(chat),
            turn_id: chat
                .last_turn_id
                .clone()
                .unwrap_or_else(|| "turn-1".to_string()),
            started_at_ms: 0,
            item: AppServerThreadItem::EnteredReviewMode {
                id: "review-start".to_string(),
                review: review.into(),
            },
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn replay_entered_review_mode(chat: &mut ChatWidget, review: impl Into<String>) {
    chat.replay_thread_item(
        AppServerThreadItem::EnteredReviewMode {
            id: "review-start".to_string(),
            review: review.into(),
        },
        "turn-1".to_string(),
        ReplayKind::ThreadSnapshot,
    );
}

pub(super) fn handle_exited_review_mode(chat: &mut ChatWidget) {
    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: thread_id(chat),
            turn_id: chat
                .last_turn_id
                .clone()
                .unwrap_or_else(|| "turn-1".to_string()),
            completed_at_ms: 0,
            item: AppServerThreadItem::ExitedReviewMode {
                id: "review-end".to_string(),
                review: String::new(),
            },
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn handle_exec_approval_request(
    chat: &mut ChatWidget,
    id: impl Into<String>,
    event: ExecApprovalRequestEvent,
) {
    chat.on_exec_approval_request(id.into(), event);
}

pub(super) fn handle_apply_patch_approval_request(
    chat: &mut ChatWidget,
    id: impl Into<String>,
    event: ApplyPatchApprovalRequestEvent,
) {
    chat.on_apply_patch_approval_request(id.into(), event);
}

fn file_update_changes_from_tui(changes: HashMap<PathBuf, FileChange>) -> Vec<FileUpdateChange> {
    changes
        .into_iter()
        .map(|(path, change)| {
            let (kind, diff) = match change {
                FileChange::Add { content } => (PatchChangeKind::Add, content),
                FileChange::Delete { content } => (PatchChangeKind::Delete, content),
                FileChange::Update {
                    unified_diff,
                    move_path,
                } => (PatchChangeKind::Update { move_path }, unified_diff),
            };
            FileUpdateChange {
                path: path.display().to_string(),
                kind,
                diff,
            }
        })
        .collect()
}

pub(super) fn handle_patch_apply_begin(
    chat: &mut ChatWidget,
    call_id: impl Into<String>,
    turn_id: impl Into<String>,
    changes: HashMap<PathBuf, FileChange>,
) {
    chat.handle_server_notification(
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: thread_id(chat),
            turn_id: turn_id.into(),
            started_at_ms: 0,
            item: AppServerThreadItem::FileChange {
                id: call_id.into(),
                changes: file_update_changes_from_tui(changes),
                status: AppServerPatchApplyStatus::InProgress,
            },
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn handle_patch_apply_end(
    chat: &mut ChatWidget,
    call_id: impl Into<String>,
    turn_id: impl Into<String>,
    changes: HashMap<PathBuf, FileChange>,
    status: AppServerPatchApplyStatus,
) {
    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: thread_id(chat),
            turn_id: turn_id.into(),
            completed_at_ms: 0,
            item: AppServerThreadItem::FileChange {
                id: call_id.into(),
                changes: file_update_changes_from_tui(changes),
                status,
            },
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn handle_view_image_tool_call(
    chat: &mut ChatWidget,
    call_id: impl Into<String>,
    path: AbsolutePathBuf,
) {
    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: thread_id(chat),
            turn_id: "turn-1".to_string(),
            completed_at_ms: 0,
            item: AppServerThreadItem::ImageView {
                id: call_id.into(),
                path,
            },
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn handle_image_generation_end(
    chat: &mut ChatWidget,
    call_id: impl Into<String>,
    revised_prompt: Option<String>,
    saved_path: Option<AbsolutePathBuf>,
) {
    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: thread_id(chat),
            turn_id: "turn-1".to_string(),
            completed_at_ms: 0,
            item: AppServerThreadItem::ImageGeneration {
                id: call_id.into(),
                status: "completed".to_string(),
                revised_prompt,
                result: String::new(),
                saved_path,
            },
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn replay_user_message_inputs(
    chat: &mut ChatWidget,
    item_id: &str,
    content: Vec<AppServerUserInput>,
    replay_kind: ReplayKind,
) {
    chat.replay_thread_item(
        AppServerThreadItem::UserMessage {
            id: item_id.to_string(),
            content,
        },
        "turn-1".to_string(),
        replay_kind,
    );
}

pub(super) fn replay_user_message_text(
    chat: &mut ChatWidget,
    item_id: &str,
    text: impl Into<String>,
    replay_kind: ReplayKind,
) {
    replay_user_message_inputs(
        chat,
        item_id,
        vec![AppServerUserInput::Text {
            text: text.into(),
            text_elements: Vec::new(),
        }],
        replay_kind,
    );
}

pub(super) fn replay_agent_message(
    chat: &mut ChatWidget,
    item_id: &str,
    text: impl Into<String>,
    replay_kind: ReplayKind,
) {
    chat.replay_thread_item(
        AppServerThreadItem::AgentMessage {
            id: item_id.to_string(),
            text: text.into(),
            phase: Some(MessagePhase::FinalAnswer),
            memory_citation: None,
        },
        "turn-1".to_string(),
        replay_kind,
    );
}

pub(super) fn replay_turn_started(chat: &mut ChatWidget, replay_kind: ReplayKind) {
    chat.handle_server_notification(
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: thread_id(chat),
            turn: app_server_turn(
                "turn-1",
                AppServerTurnStatus::InProgress,
                /*duration_ms*/ None,
                /*error*/ None,
            ),
        }),
        Some(replay_kind),
    );
}

pub(super) fn replay_agent_message_delta(
    chat: &mut ChatWidget,
    delta: impl Into<String>,
    replay_kind: ReplayKind,
) {
    chat.handle_server_notification(
        ServerNotification::AgentMessageDelta(
            codex_app_server_protocol::AgentMessageDeltaNotification {
                thread_id: thread_id(chat),
                turn_id: "turn-1".to_string(),
                item_id: "msg-1".to_string(),
                delta: delta.into(),
            },
        ),
        Some(replay_kind),
    );
}

// --- Small helpers to tersely drive exec begin/end and snapshot active cell ---
pub(super) fn begin_exec_with_source(
    chat: &mut ChatWidget,
    call_id: &str,
    raw_cmd: &str,
    source: ExecCommandSource,
) -> AppServerThreadItem {
    // Build the full command vec and parse it using core's parser,
    // then convert to protocol variants for the event payload.
    let command = vec!["bash".to_string(), "-lc".to_string(), raw_cmd.to_string()];
    let command_actions = codex_shell_command::parse_command::parse_command(&command)
        .into_iter()
        .map(|parsed| AppServerCommandAction::from_core_with_cwd(parsed, &chat.config.cwd))
        .collect();
    let item = AppServerThreadItem::CommandExecution {
        id: call_id.to_string(),
        command: codex_shell_command::parse_command::shlex_join(&command),
        cwd: chat.config.cwd.clone(),
        process_id: None,
        source,
        status: AppServerCommandExecutionStatus::InProgress,
        command_actions,
        aggregated_output: None,
        exit_code: None,
        duration_ms: None,
    };
    handle_exec_begin(chat, item.clone());
    item
}

pub(super) fn begin_unified_exec_startup(
    chat: &mut ChatWidget,
    call_id: &str,
    process_id: &str,
    raw_cmd: &str,
) -> AppServerThreadItem {
    let command = vec!["bash".to_string(), "-lc".to_string(), raw_cmd.to_string()];
    let item = AppServerThreadItem::CommandExecution {
        id: call_id.to_string(),
        command: codex_shell_command::parse_command::shlex_join(&command),
        cwd: chat.config.cwd.clone(),
        process_id: Some(process_id.to_string()),
        source: ExecCommandSource::UnifiedExecStartup,
        status: AppServerCommandExecutionStatus::InProgress,
        command_actions: Vec::new(),
        aggregated_output: None,
        exit_code: None,
        duration_ms: None,
    };
    handle_exec_begin(chat, item.clone());
    item
}

pub(super) fn handle_exec_begin(chat: &mut ChatWidget, item: AppServerThreadItem) {
    chat.handle_server_notification(
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: thread_id(chat),
            turn_id: chat
                .last_turn_id
                .clone()
                .unwrap_or_else(|| "turn-1".to_string()),
            started_at_ms: 0,
            item,
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn terminal_interaction(
    chat: &mut ChatWidget,
    call_id: &str,
    process_id: &str,
    stdin: &str,
) {
    chat.handle_server_notification(
        ServerNotification::TerminalInteraction(
            codex_app_server_protocol::TerminalInteractionNotification {
                thread_id: thread_id(chat),
                turn_id: chat
                    .last_turn_id
                    .clone()
                    .unwrap_or_else(|| "turn-1".to_string()),
                item_id: call_id.to_string(),
                process_id: process_id.to_string(),
                stdin: stdin.to_string(),
            },
        ),
        /*replay_kind*/ None,
    );
}

pub(super) fn complete_assistant_message(
    chat: &mut ChatWidget,
    item_id: &str,
    text: &str,
    phase: Option<MessagePhase>,
) {
    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: chat.thread_id.map(|id| id.to_string()).unwrap_or_default(),
            turn_id: "turn-1".to_string(),
            completed_at_ms: 0,
            item: AppServerThreadItem::AgentMessage {
                id: item_id.to_string(),
                text: text.to_string(),
                phase,
                memory_citation: None,
            },
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn pending_steer(text: &str) -> PendingSteer {
    PendingSteer {
        user_message: UserMessage::from(text),
        history_record: UserMessageHistoryRecord::UserMessageText,
        compare_key: PendingSteerCompareKey {
            message: text.to_string(),
            image_count: 0,
        },
    }
}

pub(super) fn complete_user_message(chat: &mut ChatWidget, item_id: &str, text: &str) {
    complete_user_message_for_inputs(
        chat,
        item_id,
        vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }],
    );
}

pub(super) fn complete_user_message_for_inputs(
    chat: &mut ChatWidget,
    item_id: &str,
    content: Vec<UserInput>,
) {
    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: chat.thread_id.map(|id| id.to_string()).unwrap_or_default(),
            turn_id: "turn-1".to_string(),
            completed_at_ms: 0,
            item: AppServerThreadItem::UserMessage {
                id: item_id.to_string(),
                content,
            },
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn app_server_turn(
    turn_id: &str,
    status: AppServerTurnStatus,
    duration_ms: Option<i64>,
    error: Option<AppServerTurnError>,
) -> AppServerTurn {
    AppServerTurn {
        id: turn_id.to_string(),
        items_view: codex_app_server_protocol::TurnItemsView::Full,
        items: Vec::new(),
        status,
        error,
        started_at: None,
        completed_at: None,
        duration_ms,
    }
}

pub(super) fn handle_turn_started(chat: &mut ChatWidget, turn_id: &str) {
    chat.handle_server_notification(
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: chat.thread_id.map(|id| id.to_string()).unwrap_or_default(),
            turn: app_server_turn(
                turn_id,
                AppServerTurnStatus::InProgress,
                /*duration_ms*/ None,
                /*error*/ None,
            ),
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn handle_turn_completed(
    chat: &mut ChatWidget,
    turn_id: &str,
    duration_ms: Option<i64>,
) {
    chat.handle_server_notification(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: chat.thread_id.map(|id| id.to_string()).unwrap_or_default(),
            turn: app_server_turn(
                turn_id,
                AppServerTurnStatus::Completed,
                duration_ms,
                /*error*/ None,
            ),
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn handle_turn_interrupted(chat: &mut ChatWidget, turn_id: &str) {
    chat.handle_server_notification(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: chat.thread_id.map(|id| id.to_string()).unwrap_or_default(),
            turn: app_server_turn(
                turn_id,
                AppServerTurnStatus::Interrupted,
                /*duration_ms*/ None,
                /*error*/ None,
            ),
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn handle_budget_limited_turn(chat: &mut ChatWidget, turn_id: &str) {
    chat.budget_limited_turn_ids.insert(turn_id.to_string());
    handle_turn_interrupted(chat, turn_id);
}

pub(super) fn begin_exec(
    chat: &mut ChatWidget,
    call_id: &str,
    raw_cmd: &str,
) -> AppServerThreadItem {
    begin_exec_with_source(chat, call_id, raw_cmd, ExecCommandSource::Agent)
}

pub(super) fn end_exec(
    chat: &mut ChatWidget,
    begin_item: AppServerThreadItem,
    stdout: &str,
    stderr: &str,
    exit_code: i32,
) {
    let aggregated = if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}{stderr}")
    };
    let AppServerThreadItem::CommandExecution {
        id,
        command,
        cwd,
        process_id,
        source,
        command_actions,
        ..
    } = begin_item
    else {
        panic!("expected command execution item");
    };
    handle_exec_end(
        chat,
        AppServerThreadItem::CommandExecution {
            id,
            command,
            cwd,
            process_id,
            source,
            status: if exit_code == 0 {
                AppServerCommandExecutionStatus::Completed
            } else {
                AppServerCommandExecutionStatus::Failed
            },
            command_actions,
            aggregated_output: (!aggregated.is_empty()).then_some(aggregated),
            exit_code: Some(exit_code),
            duration_ms: Some(5),
        },
    );
}

pub(super) fn handle_exec_end(chat: &mut ChatWidget, item: AppServerThreadItem) {
    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: thread_id(chat),
            turn_id: chat
                .last_turn_id
                .clone()
                .unwrap_or_else(|| "turn-1".to_string()),
            completed_at_ms: 0,
            item,
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn active_blob(chat: &ChatWidget) -> String {
    let lines = chat
        .active_cell
        .as_ref()
        .expect("active cell present")
        .display_lines(/*width*/ 80);
    lines_to_single_string(&lines)
}

pub(super) fn active_hook_blob(chat: &ChatWidget) -> String {
    let Some(cell) = chat.active_hook_cell.as_ref() else {
        return "<empty>\n".to_string();
    };
    let lines = cell.display_lines(/*width*/ 80);
    lines_to_single_string(&lines)
}

pub(super) fn expire_quiet_hook_linger(chat: &mut ChatWidget) {
    if let Some(cell) = chat.active_hook_cell.as_mut() {
        cell.expire_quiet_runs_now_for_test();
    }
    chat.pre_draw_tick();
}

pub(super) fn reveal_running_hooks(chat: &mut ChatWidget) {
    if let Some(cell) = chat.active_hook_cell.as_mut() {
        cell.reveal_running_runs_now_for_test();
    }
    chat.pre_draw_tick();
}

pub(super) fn reveal_running_hooks_after_delayed_redraw(chat: &mut ChatWidget) {
    if let Some(cell) = chat.active_hook_cell.as_mut() {
        cell.reveal_running_runs_after_delayed_redraw_for_test();
    }
    chat.pre_draw_tick();
}

pub(super) fn get_available_model(chat: &ChatWidget, model: &str) -> ModelPreset {
    let models = chat
        .model_catalog
        .try_list_models()
        .expect("models lock available");
    models
        .iter()
        .find(|&preset| preset.model == model)
        .cloned()
        .unwrap_or_else(|| panic!("{model} preset not found"))
}

pub(super) async fn assert_shift_left_edits_most_recent_queued_message_for_terminal(
    terminal_info: TerminalInfo,
) {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.queued_message_edit_hint_binding =
        Some(queued_message_edit_binding_for_terminal(terminal_info));
    chat.bottom_pane
        .set_queued_message_edit_binding(chat.queued_message_edit_hint_binding);

    // Simulate a running task so messages would normally be queued.
    chat.bottom_pane.set_task_running(/*running*/ true);

    // Seed two queued messages.
    chat.queued_user_messages
        .push_back(UserMessage::from("first queued".to_string()).into());
    chat.queued_user_messages
        .push_back(UserMessage::from("second queued".to_string()).into());
    chat.refresh_pending_input_preview();

    // Press Shift+Left to edit the most recent (last) queued message.
    chat.handle_key_event(KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT));

    // Composer should now contain the last queued message.
    assert_eq!(
        chat.bottom_pane.composer_text(),
        "second queued".to_string()
    );
    // And the queue should now contain only the remaining (older) item.
    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().text,
        "first queued"
    );
}

pub(super) fn render_bottom_first_row(chat: &ChatWidget, width: u16) -> String {
    let height = chat.desired_height(width);
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    chat.render(area, &mut buf);
    for y in 0..area.height {
        let mut row = String::new();
        for x in 0..area.width {
            let s = buf[(x, y)].symbol();
            if s.is_empty() {
                row.push(' ');
            } else {
                row.push_str(s);
            }
        }
        if !row.trim().is_empty() {
            return row;
        }
    }
    String::new()
}

pub(super) fn render_bottom_popup(chat: &ChatWidget, width: u16) -> String {
    let height = chat.desired_height(width);
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    chat.render(area, &mut buf);

    let mut lines: Vec<String> = (0..area.height)
        .map(|row| {
            let mut line = String::new();
            for col in 0..area.width {
                let symbol = buf[(area.x + col, area.y + row)].symbol();
                if symbol.is_empty() {
                    line.push(' ');
                } else {
                    line.push_str(symbol);
                }
            }
            line.trim_end().to_string()
        })
        .collect();

    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}

pub(super) fn strip_osc8_for_snapshot(text: &str) -> String {
    // Snapshots should assert the visible popup text, not terminal hyperlink escapes.
    let bytes = text.as_bytes();
    let mut stripped = String::with_capacity(text.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i..].starts_with(b"\x1B]8;;") {
            i += 5;
            while i < bytes.len() {
                if bytes[i] == b'\x07' {
                    i += 1;
                    break;
                }
                if i + 1 < bytes.len() && bytes[i] == b'\x1B' && bytes[i + 1] == b'\\' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }

        let ch = text[i..]
            .chars()
            .next()
            .expect("slice should always contain a char");
        stripped.push(ch);
        i += ch.len_utf8();
    }

    stripped
}

pub(super) fn plugins_test_absolute_path(path: &str) -> AbsolutePathBuf {
    std::env::temp_dir()
        .join("codex-plugin-menu-tests")
        .join(path)
        .abs()
}

pub(super) fn plugins_test_interface(
    display_name: Option<&str>,
    short_description: Option<&str>,
    long_description: Option<&str>,
) -> PluginInterface {
    PluginInterface {
        display_name: display_name.map(str::to_string),
        short_description: short_description.map(str::to_string),
        long_description: long_description.map(str::to_string),
        developer_name: None,
        category: None,
        capabilities: Vec::new(),
        website_url: None,
        privacy_policy_url: None,
        terms_of_service_url: None,
        default_prompt: None,
        brand_color: None,
        composer_icon: None,
        composer_icon_url: None,
        logo: None,
        logo_url: None,
        screenshots: Vec::new(),
        screenshot_urls: Vec::new(),
    }
}

pub(super) fn plugins_test_summary(
    id: &str,
    name: &str,
    display_name: Option<&str>,
    description: Option<&str>,
    installed: bool,
    enabled: bool,
    install_policy: PluginInstallPolicy,
) -> PluginSummary {
    PluginSummary {
        id: id.to_string(),
        name: name.to_string(),
        source: PluginSource::Local {
            path: plugins_test_absolute_path(&format!("plugins/{name}")),
        },
        installed,
        enabled,
        install_policy,
        auth_policy: PluginAuthPolicy::OnInstall,
        availability: PluginAvailability::Available,
        interface: Some(plugins_test_interface(
            display_name,
            description,
            /*long_description*/ None,
        )),
        keywords: Vec::new(),
    }
}

pub(super) fn plugins_test_curated_marketplace(
    plugins: Vec<PluginSummary>,
) -> PluginMarketplaceEntry {
    PluginMarketplaceEntry {
        name: OPENAI_CURATED_MARKETPLACE_NAME.to_string(),
        path: Some(plugins_test_absolute_path("marketplaces/chatgpt")),
        interface: Some(MarketplaceInterface {
            display_name: Some("ChatGPT Marketplace".to_string()),
        }),
        plugins,
    }
}

pub(super) fn plugins_test_repo_marketplace(plugins: Vec<PluginSummary>) -> PluginMarketplaceEntry {
    PluginMarketplaceEntry {
        name: "repo".to_string(),
        path: Some(plugins_test_absolute_path("marketplaces/repo")),
        interface: Some(MarketplaceInterface {
            display_name: Some("Repo Marketplace".to_string()),
        }),
        plugins,
    }
}

pub(super) fn plugins_test_response(
    marketplaces: Vec<PluginMarketplaceEntry>,
) -> PluginListResponse {
    PluginListResponse {
        marketplaces,
        marketplace_load_errors: Vec::new(),
        featured_plugin_ids: Vec::new(),
    }
}

pub(super) fn render_loaded_plugins_popup(
    chat: &mut ChatWidget,
    response: PluginListResponse,
) -> String {
    let cwd = chat.config.cwd.clone();
    chat.on_plugins_loaded(cwd.to_path_buf(), Ok(response));
    chat.add_plugins_output();
    render_bottom_popup(chat, /*width*/ 100)
}

pub(super) fn plugins_test_detail(
    summary: PluginSummary,
    description: Option<&str>,
    skills: &[&str],
    apps: &[(&str, bool)],
    mcp_servers: &[&str],
) -> PluginDetail {
    PluginDetail {
        marketplace_name: "ChatGPT Marketplace".to_string(),
        marketplace_path: Some(plugins_test_absolute_path("marketplaces/chatgpt")),
        summary,
        description: description.map(str::to_string),
        skills: skills
            .iter()
            .map(|name| SkillSummary {
                name: (*name).to_string(),
                description: format!("{name} description"),
                short_description: None,
                interface: None,
                path: Some(plugins_test_absolute_path(&format!(
                    "skills/{name}/SKILL.md"
                ))),
                enabled: true,
            })
            .collect(),
        apps: apps
            .iter()
            .map(|(name, needs_auth)| AppSummary {
                id: format!("{name}-id"),
                name: (*name).to_string(),
                description: Some(format!("{name} app")),
                install_url: Some(format!("https://example.test/{name}")),
                needs_auth: *needs_auth,
            })
            .collect(),
        mcp_servers: mcp_servers.iter().map(|name| (*name).to_string()).collect(),
    }
}

pub(super) fn plugins_test_popup_row_position(popup: &str, needle: &str) -> usize {
    popup
        .find(needle)
        .unwrap_or_else(|| panic!("expected popup to contain {needle}: {popup}"))
}

pub(super) fn type_plugins_search_query(chat: &mut ChatWidget, query: &str) {
    for ch in query.chars() {
        chat.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
    }
}

pub(super) fn handle_hook_started(chat: &mut ChatWidget, run: AppServerHookRunSummary) {
    chat.handle_server_notification(
        ServerNotification::HookStarted(AppServerHookStartedNotification {
            thread_id: thread_id(chat),
            turn_id: None,
            run,
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn handle_hook_completed(chat: &mut ChatWidget, run: AppServerHookRunSummary) {
    chat.handle_server_notification(
        ServerNotification::HookCompleted(AppServerHookCompletedNotification {
            thread_id: thread_id(chat),
            turn_id: None,
            run,
        }),
        /*replay_kind*/ None,
    );
}

pub(super) fn hook_run(
    run_id: &str,
    event_name: codex_app_server_protocol::HookEventName,
    status: codex_app_server_protocol::HookRunStatus,
    status_message: &str,
    entries: Vec<codex_app_server_protocol::HookOutputEntry>,
) -> codex_app_server_protocol::HookRunSummary {
    codex_app_server_protocol::HookRunSummary {
        id: run_id.to_string(),
        event_name,
        handler_type: codex_app_server_protocol::HookHandlerType::Command,
        execution_mode: codex_app_server_protocol::HookExecutionMode::Sync,
        scope: codex_app_server_protocol::HookScope::Turn,
        source_path: PathBuf::from(test_path_display("/tmp/hooks.json")).abs(),
        source: codex_app_server_protocol::HookSource::User,
        display_order: 0,
        status,
        status_message: Some(status_message.to_string()),
        started_at: 1,
        completed_at: matches!(
            status,
            codex_app_server_protocol::HookRunStatus::Completed
                | codex_app_server_protocol::HookRunStatus::Failed
                | codex_app_server_protocol::HookRunStatus::Blocked
                | codex_app_server_protocol::HookRunStatus::Stopped
        )
        .then_some(11),
        duration_ms: matches!(
            status,
            codex_app_server_protocol::HookRunStatus::Completed
                | codex_app_server_protocol::HookRunStatus::Failed
                | codex_app_server_protocol::HookRunStatus::Blocked
                | codex_app_server_protocol::HookRunStatus::Stopped
        )
        .then_some(10),
        entries,
    }
}

pub(super) async fn assert_hook_events_snapshot(
    event_name: codex_app_server_protocol::HookEventName,
    run_id: &str,
    status_message: &str,
    snapshot_name: &str,
) {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    handle_hook_started(
        &mut chat,
        hook_run(
            run_id,
            event_name,
            codex_app_server_protocol::HookRunStatus::Running,
            status_message,
            Vec::new(),
        ),
    );
    assert!(
        drain_insert_history(&mut rx).is_empty(),
        "hook start should update the live hook cell instead of writing history"
    );
    reveal_running_hooks(&mut chat);
    assert!(
        active_hook_blob(&chat).contains(&format!(
            "Running {} hook: {status_message}",
            hook_event_label(event_name)
        )),
        "hook start should render in the live hook cell"
    );

    handle_hook_completed(
        &mut chat,
        hook_run(
            run_id,
            event_name,
            codex_app_server_protocol::HookRunStatus::Completed,
            status_message,
            vec![
                codex_app_server_protocol::HookOutputEntry {
                    kind: codex_app_server_protocol::HookOutputEntryKind::Warning,
                    text: "Heads up from the hook".to_string(),
                },
                codex_app_server_protocol::HookOutputEntry {
                    kind: codex_app_server_protocol::HookOutputEntryKind::Context,
                    text: "Remember the startup checklist.".to_string(),
                },
            ],
        ),
    );

    let cells = drain_insert_history(&mut rx);
    let combined = cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_chatwidget_snapshot!(snapshot_name, combined);
}

fn hook_event_label(event_name: codex_app_server_protocol::HookEventName) -> &'static str {
    match event_name {
        codex_app_server_protocol::HookEventName::PreToolUse => "PreToolUse",
        codex_app_server_protocol::HookEventName::PermissionRequest => "PermissionRequest",
        codex_app_server_protocol::HookEventName::PostToolUse => "PostToolUse",
        codex_app_server_protocol::HookEventName::SessionStart => "SessionStart",
        codex_app_server_protocol::HookEventName::UserPromptSubmit => "UserPromptSubmit",
        codex_app_server_protocol::HookEventName::Stop => "Stop",
    }
}
