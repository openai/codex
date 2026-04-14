use super::*;
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
    let platform_test_cwd = test_path_display("/tmp/project");
    if platform_test_cwd == "/tmp/project" {
        text
    } else {
        text = text.replace(&platform_test_cwd, "/tmp/project");

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
            used_percent: percent,
            window_minutes: Some(60),
            resets_at: None,
        }),
        secondary: None,
        credits: None,
        plan_type: None,
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
        SessionSource::Cli,
    )
}

pub(super) fn test_model_catalog(config: &Config) -> Arc<ModelCatalog> {
    let collaboration_modes_config = CollaborationModesConfig {
        default_mode_request_user_input: config
            .features
            .enabled(Feature::DefaultModeRequestUserInput),
    };
    Arc::new(ModelCatalog::new(
        crate::legacy_core::test_support::all_model_presets().clone(),
        collaboration_modes_config,
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
    let mut widget = ChatWidget {
        app_event_tx,
        codex_op_target: super::CodexOpTarget::Direct(op_tx),
        bottom_pane: bottom,
        active_cell: None,
        active_cell_revision: 0,
        config: cfg,
        current_collaboration_mode,
        active_collaboration_mask,
        has_chatgpt_account: false,
        model_catalog,
        session_telemetry,
        session_header: SessionHeader::new(resolved_model.clone()),
        initial_user_message: None,
        status_account_display: None,
        token_info: None,
        rate_limit_snapshots_by_limit_id: BTreeMap::new(),
        refreshing_status_outputs: Vec::new(),
        next_status_refresh_request_id: 0,
        plan_type: None,
        rate_limit_warnings: RateLimitWarningState::default(),
        rate_limit_switch_prompt: RateLimitSwitchPromptState::default(),
        adaptive_chunking: crate::streaming::chunking::AdaptiveChunkingPolicy::default(),
        stream_controller: None,
        plan_stream_controller: None,
        clipboard_lease: None,
        pending_guardian_review_status: PendingGuardianReviewStatus::default(),
        terminal_title_status_kind: TerminalTitleStatusKind::Working,
        last_agent_markdown: None,
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
        connectors_prefetch_in_flight: false,
        connectors_force_refetch_pending: false,
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
        last_turn_id: None,
        thread_name: None,
        forked_from: None,
        frame_requester: FrameRequester::test_dummy(),
        show_welcome_banner: true,
        startup_tooltip_override: None,
        queued_user_messages: VecDeque::new(),
        rejected_steers_queue: VecDeque::new(),
        pending_steers: VecDeque::new(),
        submit_pending_steers_after_interrupt: false,
        queued_message_edit_binding: crate::key_hint::alt(KeyCode::Up),
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
        last_separator_elapsed_secs: None,
        turn_runtime_metrics: RuntimeMetricsSummary::default(),
        last_rendered_width: std::cell::Cell::new(None),
        feedback: codex_feedback::CodexFeedback::new(),
        current_rollout_path: None,
        current_cwd: None,
        instruction_source_paths: Vec::new(),
        session_network_proxy: None,
        status_line_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        terminal_title_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        last_terminal_title: None,
        terminal_title_setup_original_items: None,
        terminal_title_animation_origin: Instant::now(),
        status_line_project_root_name_cache: None,
        status_line_branch: None,
        status_line_branch_cwd: None,
        status_line_branch_pending: false,
        status_line_branch_lookup_complete: false,
        external_editor_state: ExternalEditorState::Closed,
        realtime_conversation: RealtimeConversationUiState::default(),
        last_rendered_user_message_event: None,
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

    chat.model_catalog = Arc::new(ModelCatalog::new(
        models,
        CollaborationModesConfig {
            default_mode_request_user_input: chat
                .config
                .features
                .enabled(Feature::DefaultModeRequestUserInput),
        },
    ));
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

// --- Small helpers to tersely drive exec begin/end and snapshot active cell ---
pub(super) fn begin_exec_with_source(
    chat: &mut ChatWidget,
    call_id: &str,
    raw_cmd: &str,
    source: ExecCommandSource,
) -> ExecCommandBeginEvent {
    // Build the full command vec and parse it using core's parser,
    // then convert to protocol variants for the event payload.
    let command = vec!["bash".to_string(), "-lc".to_string(), raw_cmd.to_string()];
    let parsed_cmd: Vec<ParsedCommand> =
        codex_shell_command::parse_command::parse_command(&command);
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let interaction_input = None;
    let event = ExecCommandBeginEvent {
        call_id: call_id.to_string(),
        process_id: None,
        turn_id: "turn-1".to_string(),
        command,
        cwd,
        parsed_cmd,
        source,
        interaction_input,
    };
    chat.handle_codex_event(Event {
        id: call_id.to_string(),
        msg: EventMsg::ExecCommandBegin(event.clone()),
    });
    event
}

pub(super) fn begin_unified_exec_startup(
    chat: &mut ChatWidget,
    call_id: &str,
    process_id: &str,
    raw_cmd: &str,
) -> ExecCommandBeginEvent {
    let command = vec!["bash".to_string(), "-lc".to_string(), raw_cmd.to_string()];
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let event = ExecCommandBeginEvent {
        call_id: call_id.to_string(),
        process_id: Some(process_id.to_string()),
        turn_id: "turn-1".to_string(),
        command,
        cwd,
        parsed_cmd: Vec::new(),
        source: ExecCommandSource::UnifiedExecStartup,
        interaction_input: None,
    };
    chat.handle_codex_event(Event {
        id: call_id.to_string(),
        msg: EventMsg::ExecCommandBegin(event.clone()),
    });
    event
}

pub(super) fn terminal_interaction(
    chat: &mut ChatWidget,
    call_id: &str,
    process_id: &str,
    stdin: &str,
) {
    chat.handle_codex_event(Event {
        id: call_id.to_string(),
        msg: EventMsg::TerminalInteraction(TerminalInteractionEvent {
            call_id: call_id.to_string(),
            process_id: process_id.to_string(),
            stdin: stdin.to_string(),
        }),
    });
}

pub(super) fn complete_assistant_message(
    chat: &mut ChatWidget,
    item_id: &str,
    text: &str,
    phase: Option<MessagePhase>,
) {
    chat.handle_codex_event(Event {
        id: format!("raw-{item_id}"),
        msg: EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id: ThreadId::new(),
            turn_id: "turn-1".to_string(),
            item: TurnItem::AgentMessage(AgentMessageItem {
                id: item_id.to_string(),
                content: vec![AgentMessageContent::Text {
                    text: text.to_string(),
                }],
                phase,
                memory_citation: None,
            }),
        }),
    });
}

pub(super) fn pending_steer(text: &str) -> PendingSteer {
    PendingSteer {
        user_message: UserMessage::from(text),
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
    chat.handle_codex_event(Event {
        id: format!("raw-{item_id}"),
        msg: EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id: ThreadId::new(),
            turn_id: "turn-1".to_string(),
            item: TurnItem::UserMessage(UserMessageItem {
                id: item_id.to_string(),
                content,
            }),
        }),
    });
}

pub(super) fn begin_exec(
    chat: &mut ChatWidget,
    call_id: &str,
    raw_cmd: &str,
) -> ExecCommandBeginEvent {
    begin_exec_with_source(chat, call_id, raw_cmd, ExecCommandSource::Agent)
}

pub(super) fn end_exec(
    chat: &mut ChatWidget,
    begin_event: ExecCommandBeginEvent,
    stdout: &str,
    stderr: &str,
    exit_code: i32,
) {
    let aggregated = if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}{stderr}")
    };
    let ExecCommandBeginEvent {
        call_id,
        turn_id,
        command,
        cwd,
        parsed_cmd,
        source,
        interaction_input,
        process_id,
    } = begin_event;
    chat.handle_codex_event(Event {
        id: call_id.clone(),
        msg: EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id,
            process_id,
            turn_id,
            command,
            cwd,
            parsed_cmd,
            source,
            interaction_input,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            aggregated_output: aggregated.clone(),
            exit_code,
            duration: std::time::Duration::from_millis(5),
            formatted_output: aggregated,
            status: if exit_code == 0 {
                CoreExecCommandStatus::Completed
            } else {
                CoreExecCommandStatus::Failed
            },
        }),
    });
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
    chat.queued_message_edit_binding = queued_message_edit_binding_for_terminal(terminal_info);
    chat.bottom_pane
        .set_queued_message_edit_binding(chat.queued_message_edit_binding);

    // Simulate a running task so messages would normally be queued.
    chat.bottom_pane.set_task_running(/*running*/ true);

    // Seed two queued messages.
    chat.queued_user_messages
        .push_back(UserMessage::from("first queued".to_string()));
    chat.queued_user_messages
        .push_back(UserMessage::from("second queued".to_string()));
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
        logo: None,
        screenshots: Vec::new(),
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
        interface: Some(plugins_test_interface(
            display_name,
            description,
            /*long_description*/ None,
        )),
    }
}

pub(super) fn plugins_test_curated_marketplace(
    plugins: Vec<PluginSummary>,
) -> PluginMarketplaceEntry {
    PluginMarketplaceEntry {
        name: OPENAI_CURATED_MARKETPLACE_NAME.to_string(),
        path: plugins_test_absolute_path("marketplaces/chatgpt"),
        interface: Some(MarketplaceInterface {
            display_name: Some("ChatGPT Marketplace".to_string()),
        }),
        plugins,
    }
}

pub(super) fn plugins_test_repo_marketplace(plugins: Vec<PluginSummary>) -> PluginMarketplaceEntry {
    PluginMarketplaceEntry {
        name: "repo".to_string(),
        path: plugins_test_absolute_path("marketplaces/repo"),
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
        remote_sync_error: None,
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
        marketplace_path: plugins_test_absolute_path("marketplaces/chatgpt"),
        summary,
        description: description.map(str::to_string),
        skills: skills
            .iter()
            .map(|name| SkillSummary {
                name: (*name).to_string(),
                description: format!("{name} description"),
                short_description: None,
                interface: None,
                path: plugins_test_absolute_path(&format!("skills/{name}/SKILL.md")),
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

pub(super) async fn assert_hook_events_snapshot(
    event_name: codex_protocol::protocol::HookEventName,
    run_id: &str,
    status_message: &str,
    snapshot_name: &str,
) {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "hook-1".into(),
        msg: EventMsg::HookStarted(codex_protocol::protocol::HookStartedEvent {
            turn_id: None,
            run: codex_protocol::protocol::HookRunSummary {
                id: run_id.to_string(),
                event_name,
                handler_type: codex_protocol::protocol::HookHandlerType::Command,
                execution_mode: codex_protocol::protocol::HookExecutionMode::Sync,
                scope: codex_protocol::protocol::HookScope::Turn,
                source_path: PathBuf::from("/tmp/hooks.json"),
                display_order: 0,
                status: codex_protocol::protocol::HookRunStatus::Running,
                status_message: Some(status_message.to_string()),
                started_at: 1,
                completed_at: None,
                duration_ms: None,
                entries: vec![],
            },
        }),
    });
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

    chat.handle_codex_event(Event {
        id: "hook-1".into(),
        msg: EventMsg::HookCompleted(codex_protocol::protocol::HookCompletedEvent {
            turn_id: None,
            run: codex_protocol::protocol::HookRunSummary {
                id: run_id.to_string(),
                event_name,
                handler_type: codex_protocol::protocol::HookHandlerType::Command,
                execution_mode: codex_protocol::protocol::HookExecutionMode::Sync,
                scope: codex_protocol::protocol::HookScope::Turn,
                source_path: PathBuf::from("/tmp/hooks.json"),
                display_order: 0,
                status: codex_protocol::protocol::HookRunStatus::Completed,
                status_message: Some(status_message.to_string()),
                started_at: 1,
                completed_at: Some(11),
                duration_ms: Some(10),
                entries: vec![
                    codex_protocol::protocol::HookOutputEntry {
                        kind: codex_protocol::protocol::HookOutputEntryKind::Warning,
                        text: "Heads up from the hook".to_string(),
                    },
                    codex_protocol::protocol::HookOutputEntry {
                        kind: codex_protocol::protocol::HookOutputEntryKind::Context,
                        text: "Remember the startup checklist.".to_string(),
                    },
                ],
            },
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let combined = cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_chatwidget_snapshot!(snapshot_name, combined);
}

fn hook_event_label(event_name: codex_protocol::protocol::HookEventName) -> &'static str {
    match event_name {
        codex_protocol::protocol::HookEventName::PreToolUse => "PreToolUse",
        codex_protocol::protocol::HookEventName::PostToolUse => "PostToolUse",
        codex_protocol::protocol::HookEventName::SessionStart => "SessionStart",
        codex_protocol::protocol::HookEventName::UserPromptSubmit => "UserPromptSubmit",
        codex_protocol::protocol::HookEventName::Stop => "Stop",
    }
}
