//! Startup bootstrap, draft handoff, and protected-input orchestration for the TUI app.
//!
//! Owns the main app run loop from app-server bootstrap through terminal shutdown. Startup input
//! remains isolated from protected interactive requests until the initialized composer owns it.

use super::*;

async fn resolve_runtime_model_provider_base_url(provider: &ModelProviderInfo) -> Option<String> {
    let provider = create_model_provider(provider.clone(), /*auth_manager*/ None);
    match provider.runtime_base_url().await {
        Ok(base_url) => base_url,
        Err(err) => {
            tracing::warn!(%err, "failed to resolve runtime model provider base URL for status");
            None
        }
    }
}

fn spawn_startup_thread_start(
    app_server: &AppServerSession,
    config: Config,
    app_event_tx: AppEventSender,
) {
    let request_handle = app_server.request_handle();
    let thread_params_mode = app_server.thread_params_mode();
    let remote_cwd_override = app_server.remote_cwd_override().map(Path::to_path_buf);
    tokio::spawn(async move {
        let result = crate::app_server_session::start_thread_with_request_handle(
            request_handle,
            config,
            thread_params_mode,
            remote_cwd_override,
        )
        .await
        .map_err(|err| format!("{err:#}"));
        app_event_tx.send(AppEvent::StartupThreadStarted { result });
    });
}

impl App {
    /// Recognizes queued requests before they become visible protected screens.
    pub(super) fn has_queued_startup_protected_request(&self) -> bool {
        self.startup_protected_input_boundary
            && (self
                .active_thread_rx
                .as_ref()
                .is_some_and(|receiver| !receiver.is_empty())
                || self
                    .pending_primary_events
                    .iter()
                    .any(|event| matches!(event, ThreadBufferedEvent::Request(_))))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        tui: &mut tui::Tui,
        mut app_server: AppServerSession,
        mut config: Config,
        launch_cwd: PathBuf,
        cli_kv_overrides: Vec<(String, TomlValue)>,
        harness_overrides: ConfigOverrides,
        loader_overrides: LoaderOverrides,
        cloud_config_bundle: CloudConfigBundleLoader,
        initial_prompt: Option<String>,
        initial_images: Vec<PathBuf>,
        session_selection: SessionSelection,
        feedback: codex_feedback::CodexFeedback,
        is_first_run: bool,
        should_prompt_windows_sandbox_nux_at_startup: bool,
        app_server_target: AppServerTarget,
        state_db: Option<StateDbHandle>,
        environment_manager: Arc<EnvironmentManager>,
        startup_elapsed_before_app: Duration,
        startup_bootstrap: Option<AppServerBootstrap>,
        startup_hooks_browser: Option<HooksListEntry>,
        mut startup_draft: StartupDraftPump,
    ) -> Result<AppExitInfo> {
        use tokio_stream::StreamExt;

        async fn shutdown_on_startup_error(
            app_server: AppServerSession,
            error: impl Into<color_eyre::eyre::Report>,
        ) -> Result<AppExitInfo> {
            if let Err(shutdown_error) = app_server.shutdown().await {
                tracing::warn!("app-server shutdown failed: {shutdown_error}");
            }
            Err(error.into())
        }

        fn render_startup_frame(app: &mut App, tui: &mut tui::Tui) -> Result<()> {
            app.chat_widget.pre_draw_tick();
            app.render_chat_widget_frame(tui, tui.terminal.last_known_screen_size)?;
            if app.chat_widget.has_active_view() && app.startup_protected_input_boundary {
                tui.discard_pending_input_before_interactive_screen()?;
                app.startup_pending_protected_request = false;
            }
            Ok(())
        }

        let startup_started_at = Instant::now();
        let (app_event_tx, mut app_event_rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(app_event_tx);
        emit_project_config_warnings(&app_event_tx, &config);
        emit_system_bwrap_warning(&app_event_tx, &config);
        tui.set_notification_settings(
            config.tui_notifications.method,
            config.tui_notifications.condition,
        );

        let harness_overrides =
            normalize_harness_overrides_for_cwd(harness_overrides, &config.cwd)?;
        let bootstrap = match startup_bootstrap {
            Some(bootstrap) => bootstrap,
            None => match startup_draft
                .run_until(tui, app_server.bootstrap(&config))
                .await
            {
                Ok(bootstrap) => bootstrap?,
                Err(err) => return shutdown_on_startup_error(app_server, err).await,
            },
        };
        let bootstrap_ms = bootstrap.duration.as_millis();
        if matches!(
            &session_selection,
            SessionSelection::StartFresh | SessionSelection::Exit
        ) {
            apply_managed_new_thread_defaults(
                &mut config,
                app_server.managed_new_thread_defaults(),
                &cli_kv_overrides,
                &harness_overrides,
            );
        }
        let mut model = config.model.clone().unwrap_or(bootstrap.default_model);
        let available_models = bootstrap.available_models;
        let remote_connection = crate::status::remote_connection::remote_connection_status_value(
            &app_server_target,
            app_server.server_version(),
        );
        if let Err(err) = startup_draft.flush_pending_events(tui).await {
            return shutdown_on_startup_error(app_server, err).await;
        }
        let exit_info = handle_model_migration_prompt_if_needed(
            tui,
            &mut config,
            model.as_str(),
            &app_event_tx,
            &available_models,
        )
        .await?;
        if let Some(exit_info) = exit_info {
            app_server
                .shutdown()
                .await
                .inspect_err(|err| {
                    tracing::warn!("app-server shutdown failed: {err}");
                })
                .ok();
            return Ok(exit_info);
        }
        if let Some(updated_model) = config.model.clone() {
            model = updated_model;
        }
        let model_catalog = Arc::new(ModelCatalog::new(available_models.clone()));
        let feedback_audience = bootstrap.feedback_audience;
        let auth_mode = bootstrap.auth_mode;
        let has_chatgpt_account = bootstrap.has_chatgpt_account;
        let has_codex_backend_auth = matches!(auth_mode, Some(TelemetryAuthMode::Chatgpt));
        let requires_openai_auth = bootstrap.requires_openai_auth;
        let status_account_display = bootstrap.status_account_display.clone();
        let initial_plan_type = bootstrap.plan_type;
        let session_telemetry = SessionTelemetry::new(
            ThreadId::new(),
            model.as_str(),
            model.as_str(),
            /*account_id*/ None,
            bootstrap.account_email.clone(),
            auth_mode,
            codex_login::default_client::originator().value,
            config.otel.log_user_prompt,
            user_agent(),
            serde_json::from_value(serde_json::json!("cli"))
                .unwrap_or_else(|err| panic!("cli session source should deserialize: {err}")),
        );
        if config
            .tui_status_line
            .as_ref()
            .is_some_and(|cmd| !cmd.is_empty())
        {
            session_telemetry.counter("codex.status_line", /*inc*/ 1, &[]);
        }

        let status_line_invalid_items_warned = Arc::new(AtomicBool::new(false));
        let terminal_title_invalid_items_warned = Arc::new(AtomicBool::new(false));
        let workspace_command_runner: WorkspaceCommandRunner = Arc::new(
            AppServerWorkspaceCommandRunner::new(app_server.request_handle()),
        );
        let runtime_model_provider_started_at = Instant::now();
        let runtime_model_provider_base_url = match startup_draft
            .run_until(
                tui,
                resolve_runtime_model_provider_base_url(&config.model_provider),
            )
            .await
        {
            Ok(base_url) => base_url,
            Err(err) => return shutdown_on_startup_error(app_server, err).await,
        };
        let runtime_model_provider_ms = runtime_model_provider_started_at.elapsed().as_millis();

        let enhanced_keys_supported = tui.enhanced_keys_supported();
        let wait_for_initial_session_configured =
            Self::should_wait_for_initial_session(&session_selection);
        let should_prompt_for_paused_goal_after_startup_resume =
            Self::should_prompt_for_paused_goal_after_startup_resume(
                &session_selection,
                &initial_prompt,
                &initial_images,
            );
        let thread_and_widget_started_at = Instant::now();
        let pending_startup_thread_start = matches!(
            &session_selection,
            SessionSelection::StartFresh | SessionSelection::Exit
        );
        let (mut chat_widget, initial_started_thread) = match session_selection {
            SessionSelection::StartFresh | SessionSelection::Exit => {
                spawn_startup_thread_start(&app_server, config.clone(), app_event_tx.clone());
                // Count a startup tooltip once the initial chat widget can render it.
                let startup_tooltip_override = match startup_draft
                    .run_until(
                        tui,
                        prepare_startup_tooltip_override(
                            &mut config,
                            &available_models,
                            is_first_run,
                        ),
                    )
                    .await
                {
                    Ok(tooltip_override) => tooltip_override,
                    Err(err) => return shutdown_on_startup_error(app_server, err).await,
                };
                let init = crate::chatwidget::ChatWidgetInit {
                    config: config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: app_event_tx.clone(),
                    workspace_command_runner: Some(workspace_command_runner.clone()),
                    initial_user_message: crate::chatwidget::create_initial_user_message(
                        initial_prompt.clone(),
                        initial_images.clone(),
                        // CLI prompt args are plain strings, so they don't provide element ranges.
                        Vec::new(),
                    ),
                    enhanced_keys_supported,
                    has_chatgpt_account,
                    has_codex_backend_auth,
                    model_catalog: model_catalog.clone(),
                    feedback: feedback.clone(),
                    is_first_run,
                    status_account_display: status_account_display.clone(),
                    runtime_model_provider_base_url: runtime_model_provider_base_url.clone(),
                    initial_plan_type,
                    model: Some(model.clone()),
                    startup_tooltip_override,
                    status_line_invalid_items_warned: status_line_invalid_items_warned.clone(),
                    terminal_title_invalid_items_warned: terminal_title_invalid_items_warned
                        .clone(),
                    session_telemetry: session_telemetry.clone(),
                };
                let mut chat_widget = ChatWidget::new_with_app_event(init);
                chat_widget.set_queue_submissions_until_session_configured(/*queue*/ true);
                (chat_widget, None)
            }
            SessionSelection::Resume(target_session) => {
                if let Some(history_mode) = target_session.history_mode {
                    app_server.remember_thread_history_mode(target_session.thread_id, history_mode);
                }
                let model_settings = config_persistence::resume_model_settings_for_overrides(
                    &config,
                    &harness_overrides,
                );
                let resumed = match startup_draft
                    .run_until(
                        tui,
                        app_server.resume_thread(
                            config.clone(),
                            target_session.thread_id,
                            model_settings,
                        ),
                    )
                    .await
                {
                    Ok(resumed) => resumed
                        .map_err(|err| session_start_error("resume", &target_session, err))?,
                    Err(err) => return shutdown_on_startup_error(app_server, err).await,
                };
                let init = crate::chatwidget::ChatWidgetInit {
                    config: config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: app_event_tx.clone(),
                    workspace_command_runner: Some(workspace_command_runner.clone()),
                    initial_user_message: crate::chatwidget::create_initial_user_message(
                        initial_prompt.clone(),
                        initial_images.clone(),
                        // CLI prompt args are plain strings, so they don't provide element ranges.
                        Vec::new(),
                    ),
                    enhanced_keys_supported,
                    has_chatgpt_account,
                    has_codex_backend_auth,
                    model_catalog: model_catalog.clone(),
                    feedback: feedback.clone(),
                    is_first_run,
                    status_account_display: status_account_display.clone(),
                    runtime_model_provider_base_url: runtime_model_provider_base_url.clone(),
                    initial_plan_type,
                    model: config.model.clone(),
                    startup_tooltip_override: None,
                    status_line_invalid_items_warned: status_line_invalid_items_warned.clone(),
                    terminal_title_invalid_items_warned: terminal_title_invalid_items_warned
                        .clone(),
                    session_telemetry: session_telemetry.clone(),
                };
                (ChatWidget::new_with_app_event(init), Some(resumed))
            }
            SessionSelection::Fork(target_session) => {
                session_telemetry.counter(
                    "codex.thread.fork",
                    /*inc*/ 1,
                    &[("source", "cli_subcommand")],
                );
                let forked = match startup_draft
                    .run_until(
                        tui,
                        app_server.fork_thread(config.clone(), target_session.thread_id),
                    )
                    .await
                {
                    Ok(forked) => {
                        forked.map_err(|err| session_start_error("fork", &target_session, err))?
                    }
                    Err(err) => return shutdown_on_startup_error(app_server, err).await,
                };
                let init = crate::chatwidget::ChatWidgetInit {
                    config: config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: app_event_tx.clone(),
                    workspace_command_runner: Some(workspace_command_runner.clone()),
                    initial_user_message: crate::chatwidget::create_initial_user_message(
                        initial_prompt.clone(),
                        initial_images.clone(),
                        // CLI prompt args are plain strings, so they don't provide element ranges.
                        Vec::new(),
                    ),
                    enhanced_keys_supported,
                    has_chatgpt_account,
                    has_codex_backend_auth,
                    model_catalog: model_catalog.clone(),
                    feedback: feedback.clone(),
                    is_first_run,
                    status_account_display: status_account_display.clone(),
                    runtime_model_provider_base_url: runtime_model_provider_base_url.clone(),
                    initial_plan_type,
                    model: config.model.clone(),
                    startup_tooltip_override: None,
                    status_line_invalid_items_warned: status_line_invalid_items_warned.clone(),
                    terminal_title_invalid_items_warned: terminal_title_invalid_items_warned
                        .clone(),
                    session_telemetry: session_telemetry.clone(),
                };
                (ChatWidget::new_with_app_event(init), Some(forked))
            }
        };
        chat_widget.note_rendered_width(tui.terminal.last_known_screen_size.width);
        chat_widget.remote_connection = remote_connection;
        let thread_and_widget_ms = thread_and_widget_started_at.elapsed().as_millis();
        chat_widget
            .maybe_prompt_windows_sandbox_enable(should_prompt_windows_sandbox_nux_at_startup);

        let file_search = FileSearchManager::new(config.cwd.to_path_buf(), app_event_tx.clone());
        let runtime_keymap = RuntimeKeymap::from_config(&config.tui_keymap).map_err(|err| {
            color_eyre::eyre::eyre!(
                "Invalid `tui.keymap` configuration: {err}\n\
Fix the config and retry.\n\
See the Codex keymap documentation for supported actions and examples."
            )
        })?;
        #[cfg(not(debug_assertions))]
        let upgrade_version = crate::updates::get_upgrade_version(&config);

        let mut app = Self {
            model_catalog,
            session_telemetry: session_telemetry.clone(),
            app_event_tx,
            chat_widget,
            workspace_command_runner: Some(workspace_command_runner),
            config,
            launch_cwd,
            runtime_working_directory_override: None,
            state_db,
            cli_kv_overrides,
            harness_overrides,
            loader_overrides,
            cloud_config_bundle,
            runtime_approval_policy_override: None,
            runtime_permission_profile_override: None,
            file_search,
            enhanced_keys_supported,
            keymap: runtime_keymap,
            key_chord_matcher: KeyChordMatcher::default(),
            transcript_cells: Vec::new(),
            last_rendered_history_tail: None,
            last_thread_usage_status_cell: None,
            pending_thread_usage_history_refresh: false,
            overlay: None,
            deferred_history_lines: Vec::new(),
            has_emitted_history_lines: false,
            transcript_reflow: TranscriptReflowState::default(),
            initial_history_replay_buffer: None,
            scrollback_has_older_history: false,
            commit_anim_running: Arc::new(AtomicBool::new(false)),
            status_line_invalid_items_warned: status_line_invalid_items_warned.clone(),
            terminal_title_invalid_items_warned: terminal_title_invalid_items_warned.clone(),
            skill_load_warnings: SkillLoadWarningState::default(),
            backtrack: BacktrackState::default(),
            backtrack_render_pending: false,
            feedback: feedback.clone(),
            feedback_audience,
            environment_manager,
            app_server_target,
            pending_update_action: None,
            pending_shutdown_exit_thread_id: None,
            windows_sandbox: WindowsSandboxState::default(),
            thread_event_channels: HashMap::new(),
            thread_event_listener_tasks: HashMap::new(),
            agent_navigation: AgentNavigationState::default(),
            side_threads: HashMap::new(),
            abandoned_side_threads: HashSet::new(),
            active_thread_id: None,
            active_thread_rx: None,
            primary_thread_id: None,
            last_subagent_backfill_attempt: None,
            primary_session_configured: None,
            pending_primary_events: VecDeque::new(),
            pending_app_server_requests: PendingAppServerRequests::default(),
            pending_startup_thread_start,
            startup_protected_input_boundary: true,
            startup_pending_protected_request: false,
            rate_limit_hard_stop_generation: 0,
            pending_plugin_enabled_writes: HashMap::new(),
            pending_hook_enabled_writes: HashMap::new(),
        };
        if let Some(entry) = startup_hooks_browser {
            app.chat_widget.open_hooks_browser(entry);
        }
        app.update_visible_history_rows(tui.terminal.last_known_screen_size);
        let initial_session_started_at = Instant::now();
        if let Some(started) = initial_started_thread {
            let thread_id = started.session.thread_id;
            if started.blocks_direct_input {
                app.mark_primary_thread_parent_owned(thread_id);
            }
            match startup_draft
                .run_until(
                    tui,
                    app.enqueue_primary_thread_session(started.session, started.turns),
                )
                .await
            {
                Ok(result) => result?,
                Err(err) => return shutdown_on_startup_error(app_server, err).await,
            }
            if should_prompt_for_paused_goal_after_startup_resume
                && let Err(err) = startup_draft
                    .run_until(
                        tui,
                        app.maybe_prompt_resume_paused_goal_after_resume(
                            &mut app_server,
                            thread_id,
                        ),
                    )
                    .await
            {
                return shutdown_on_startup_error(app_server, err).await;
            }
        }
        let initial_session_ms = initial_session_started_at.elapsed().as_millis();

        // On startup, if a managed filesystem sandbox is active, warn about
        // world-writable dirs on Windows.
        #[cfg(target_os = "windows")]
        {
            let startup_permission_profile = app.config.permissions.effective_permission_profile();
            let should_check = crate::windows_sandbox::level_from_config(&app.config)
                != WindowsSandboxLevel::Disabled
                && managed_filesystem_sandbox_is_restricted(&startup_permission_profile)
                && !app
                    .config
                    .notices
                    .hide_world_writable_warning
                    .unwrap_or(false);
            if should_check {
                let cwd = app.config.cwd.clone();
                let workspace_roots = app.config.effective_workspace_roots();
                let env_map: std::collections::HashMap<String, String> = std::env::vars().collect();
                let tx = app.app_event_tx.clone();
                let logs_base_dir = app.config.codex_home.clone();
                Self::spawn_world_writable_scan(
                    cwd,
                    workspace_roots,
                    env_map,
                    logs_base_dir,
                    startup_permission_profile,
                    tx,
                );
            }
        }

        if let Err(err) = startup_draft.flush_pending_events(tui).await {
            return shutdown_on_startup_error(app_server, err).await;
        }
        if Self::should_handle_active_thread_events(
            wait_for_initial_session_configured,
            app.active_thread_rx.is_some(),
        ) && let Err(err) = app.drain_active_thread_events(tui).await
        {
            return shutdown_on_startup_error(app_server, err).await;
        }
        if app_event_rx.is_empty()
            && !app.has_queued_startup_protected_request()
            && !app.startup_pending_protected_request
            && !app.chat_widget.has_active_view()
            && !app.chat_widget.has_pending_protected_request()
            && let Err(err) = startup_draft.flush_pending_paste_newline(tui).await
        {
            return shutdown_on_startup_error(app_server, err).await;
        }
        let mut pending_startup_draft = Some(startup_draft.into_draft());
        if app_event_rx.is_empty() && !app.has_queued_startup_protected_request() {
            app.chat_widget
                .restore_startup_draft_when_ready(&mut pending_startup_draft);
        }

        let event_stream_started_at = Instant::now();
        tui.schedule_screen_size_recheck(Duration::ZERO);
        if let Err(err) = render_startup_frame(&mut app, tui) {
            return shutdown_on_startup_error(app_server, err).await;
        }
        let tui_events = tui.event_stream();
        tokio::pin!(tui_events);
        tracing::info!(
            duration_ms = %(startup_elapsed_before_app + startup_started_at.elapsed()).as_millis(),
            bootstrap_ms = %bootstrap_ms,
            runtime_model_provider_ms = %runtime_model_provider_ms,
            thread_and_widget_ms = %thread_and_widget_ms,
            initial_session_ms = %initial_session_ms,
            event_stream_ms = %event_stream_started_at.elapsed().as_millis(),
            "tui startup initial frame scheduled"
        );
        app.refresh_startup_skills(&app_server);
        // Kick off a non-blocking rate-limit prefetch so the first `/status`
        // already has data and available reset credits can be surfaced, without
        // delaying the initial frame render.
        if requires_openai_auth && has_chatgpt_account {
            let reset_hint_request_id = app.chat_widget.start_rate_limit_reset_startup_check();
            app.refresh_rate_limits(
                &app_server,
                RateLimitRefreshOrigin::StartupPrefetch {
                    reset_hint_request_id,
                },
            );
        }

        let mut listen_for_app_server_events = true;
        let mut waiting_for_initial_session_configured = wait_for_initial_session_configured;
        let mut waiting_for_initial_session_header = true;

        #[cfg(not(debug_assertions))]
        let pre_loop_exit_reason = if let Some(latest_version) = upgrade_version {
            let control = Box::pin(app.handle_event(
                tui,
                &mut app_server,
                AppEvent::InsertHistoryCell(Box::new(UpdateAvailableHistoryCell::new(
                    latest_version,
                    crate::update_action::get_update_action(),
                ))),
            ))
            .await?;
            match control {
                AppRunControl::Continue => None,
                AppRunControl::Exit(exit_reason) => Some(exit_reason),
            }
        } else {
            None
        };
        #[cfg(debug_assertions)]
        let pre_loop_exit_reason: Option<ExitReason> = None;

        let exit_reason_result = if let Some(exit_reason) = pre_loop_exit_reason {
            Ok(exit_reason)
        } else {
            loop {
                let initial_session_header_pending = waiting_for_initial_session_header
                    && app.primary_session_configured.is_some()
                    && !app_event_rx.is_empty();
                let block_terminal_input_for_pending_startup_events = initial_session_header_pending
                    || (pending_startup_draft.is_some() || app.startup_protected_input_boundary)
                        && !app_event_rx.is_empty()
                    || (!waiting_for_initial_session_configured
                        && app.has_queued_startup_protected_request());
                let control = select! {
                    Some(event) = app_event_rx.recv() => {
                        let is_initial_session_header = matches!(
                            &event,
                            AppEvent::InsertHistoryCell(cell)
                                if cell.as_any().is::<history_cell::SessionInfoCell>()
                        );
                        let had_active_view = app.chat_widget.has_active_view();
                        match Box::pin(app.handle_event(tui, &mut app_server, event)).await {
                            Ok(AppRunControl::Continue) => {
                                if is_initial_session_header {
                                    waiting_for_initial_session_header = false;
                                }
                                if !had_active_view
                                    && app.chat_widget.has_active_view()
                                    && let Err(err) = render_startup_frame(&mut app, tui)
                                {
                                    break Err(err);
                                }
                                AppRunControl::Continue
                            }
                            Ok(AppRunControl::Exit(reason)) => AppRunControl::Exit(reason),
                            Err(err) => break Err(err),
                        }
                    }
                    active = async {
                        if let Some(rx) = app.active_thread_rx.as_mut() {
                            rx.recv().await
                        } else {
                            None
                        }
                    }, if App::should_handle_active_thread_events(
                        waiting_for_initial_session_configured,
                        app.active_thread_rx.is_some()
                    ) => {
                        if let Some(event) = active {
                            if let Err(err) = app.handle_active_thread_event(tui, &mut app_server, event).await {
                                break Err(err);
                            }
                        } else {
                            app.clear_active_thread().await;
                        }
                        AppRunControl::Continue
                    }
                    event = tui_events.next(), if !block_terminal_input_for_pending_startup_events => {
                        if let Some(event) = event {
                            if (matches!(
                                &event,
                                TuiEvent::Key(key)
                                    if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
                            ) || matches!(&event, TuiEvent::Paste(_)))
                                && pending_startup_draft.is_none()
                                && !waiting_for_initial_session_configured
                                && app_event_rx.is_empty()
                                && !app.startup_pending_protected_request
                                && app
                                    .active_thread_rx
                                    .as_ref()
                                    .is_none_or(tokio::sync::mpsc::Receiver::is_empty)
                                && !app.pending_primary_events.iter().any(|event| {
                                    matches!(event, ThreadBufferedEvent::Request(_))
                                })
                            {
                                app.startup_protected_input_boundary = false;
                            }
                            match app.handle_tui_event(tui, &mut app_server, event).await {
                                Ok(control) => control,
                                Err(err) => break Err(err),
                            }
                        } else {
                            tracing::warn!("terminal input stream closed; shutting down active thread");
                            app.handle_exit_mode(&mut app_server, ExitMode::ShutdownFirst).await
                        }
                    }
                    app_server_event = app_server.next_event(), if listen_for_app_server_events => {
                        match app_server_event {
                            Some(event) => app.handle_app_server_event(&app_server, event).await,
                            None => {
                                listen_for_app_server_events = false;
                                tracing::warn!("app-server event stream closed");
                            }
                        }
                        AppRunControl::Continue
                    }
                };
                if App::should_stop_waiting_for_initial_session(
                    waiting_for_initial_session_configured,
                    app.primary_thread_id,
                ) {
                    waiting_for_initial_session_configured = false;
                    let had_active_view = app.chat_widget.has_active_view();
                    if let Err(err) = app.drain_active_thread_events(tui).await {
                        break Err(err);
                    }
                    if !had_active_view
                        && app.chat_widget.has_active_view()
                        && let Err(err) = render_startup_frame(&mut app, tui)
                    {
                        break Err(err);
                    }
                }
                match control {
                    AppRunControl::Continue => {
                        if app_event_rx.is_empty() && !app.has_queued_startup_protected_request() {
                            app.chat_widget
                                .restore_startup_draft_when_ready(&mut pending_startup_draft);
                        }
                    }
                    AppRunControl::Exit(reason) => break Ok(reason),
                }
            }
        };
        if let Err(err) = app_server.shutdown().await {
            tracing::warn!(error = %err, "failed to shut down embedded app server");
        }
        let clear_pet_result = tui.clear_ambient_pet_image();
        let clear_result = tui.terminal.clear();
        let exit_reason = match exit_reason_result {
            Ok(exit_reason) => {
                clear_pet_result?;
                clear_result?;
                exit_reason
            }
            Err(err) => {
                if let Err(clear_pet_err) = clear_pet_result {
                    tracing::warn!(error = %clear_pet_err, "failed to clear ambient pet image");
                }
                if let Err(clear_err) = clear_result {
                    tracing::warn!(error = %clear_err, "failed to clear terminal UI");
                }
                return Err(err);
            }
        };
        let thread_id = app.chat_widget.thread_id().or(app.primary_thread_id);
        let resume_hint = resume_hint_for_resumable_thread(
            thread_id,
            app.chat_widget.thread_name(),
            app.chat_widget.rollout_path().as_deref(),
        );
        Ok(AppExitInfo {
            token_usage: app.token_usage(),
            thread_id,
            resume_hint,
            update_action: app.pending_update_action,
            exit_reason,
        })
    }
}
