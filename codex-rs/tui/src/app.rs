use crate::app_backtrack::BacktrackState;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ApprovalRequest;
use crate::chatwidget::ChatWidget;
use crate::diff_render::DiffSummary;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::file_search::FileSearchManager;
use crate::history_cell::HistoryCell;
use crate::model_migration::ModelMigrationOutcome;
use crate::model_migration::run_model_migration_prompt;
use crate::pager_overlay::Overlay;
use crate::render::highlight::highlight_bash_to_lines;
use crate::render::renderable::Renderable;
use crate::resume_picker::ResumeSelection;
use crate::tui;
use crate::tui::TuiEvent;
use crate::update_action::UpdateAction;
use codex_ansi_escape::ansi_escape_line;
use codex_common::model_presets::ModelUpgrade;
use codex_common::model_presets::all_model_presets;
use codex_core::AuthManager;
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::config::edit::ConfigEditsBuilder;
use codex_core::config::persist_alarm_script;
use codex_core::config::persist_auto_checkpoint;
use codex_core::config::persist_auto_commit;
use codex_core::config::persist_auto_compact_settings;
use codex_core::config::persist_global_prompt;
use codex_core::config::persist_model_selection;
use codex_core::config::persist_prompt_aliases;
use codex_core::config::persist_prompt_presets;
#[cfg(target_os = "windows")]
#[cfg(target_os = "windows")]
use codex_core::features::Feature;
use codex_core::git_info::get_git_repo_root;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::FinalOutput;
#[cfg(target_os = "windows")]
use codex_core::protocol::Op;
use codex_core::protocol::SessionSource;
use codex_core::protocol::TokenUsage;
use codex_core::protocol_config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::ConversationId;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use color_eyre::eyre::eyre;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use tokio::select;
use tokio::sync::mpsc::unbounded_channel;

#[cfg(not(debug_assertions))]
use crate::history_cell::UpdateAvailableHistoryCell;

#[derive(Debug, Clone)]
pub struct AppExitInfo {
    pub token_usage: TokenUsage,
    pub conversation_id: Option<ConversationId>,
    pub update_action: Option<UpdateAction>,
}

fn session_summary(
    token_usage: TokenUsage,
    conversation_id: Option<ConversationId>,
) -> Option<SessionSummary> {
    if token_usage.is_zero() {
        return None;
    }

    let usage_line = FinalOutput::from(token_usage).to_string();
    let resume_command =
        conversation_id.map(|conversation_id| format!("codex resume {conversation_id}"));
    Some(SessionSummary {
        usage_line,
        resume_command,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionSummary {
    usage_line: String,
    resume_command: Option<String>,
}

fn should_show_model_migration_prompt(
    current_model: &str,
    target_model: &str,
    hide_prompt_flag: Option<bool>,
) -> bool {
    if target_model == current_model || hide_prompt_flag.unwrap_or(false) {
        return false;
    }

    all_model_presets()
        .iter()
        .filter(|preset| preset.upgrade.is_some())
        .any(|preset| preset.model == current_model)
}

async fn handle_model_migration_prompt_if_needed(
    tui: &mut tui::Tui,
    config: &mut Config,
    app_event_tx: &AppEventSender,
) -> Option<AppExitInfo> {
    let upgrade = all_model_presets()
        .iter()
        .find(|preset| preset.model == config.model)
        .and_then(|preset| preset.upgrade.as_ref());

    if let Some(ModelUpgrade {
        id: target_model,
        reasoning_effort_mapping,
    }) = upgrade
    {
        let target_model = target_model.to_string();
        let hide_prompt_flag = config.notices.hide_gpt5_1_migration_prompt;
        if !should_show_model_migration_prompt(&config.model, &target_model, hide_prompt_flag) {
            return None;
        }

        match run_model_migration_prompt(tui).await {
            ModelMigrationOutcome::Accepted => {
                app_event_tx.send(AppEvent::PersistModelMigrationPromptAcknowledged {
                    migration_config: "hide_gpt5_1_migration_prompt".to_string(),
                });
                config.model = target_model.to_string();
                if let Some(family) = find_family_for_model(&target_model) {
                    config.model_family = family;
                }

                let mapped_effort = if let Some(reasoning_effort_mapping) = reasoning_effort_mapping
                    && let Some(reasoning_effort) = config.model_reasoning_effort
                {
                    reasoning_effort_mapping
                        .get(&reasoning_effort)
                        .cloned()
                        .or(config.model_reasoning_effort)
                } else {
                    config.model_reasoning_effort
                };

                config.model_reasoning_effort = mapped_effort;

                app_event_tx.send(AppEvent::UpdateModel(target_model.clone()));
                app_event_tx.send(AppEvent::UpdateReasoningEffort(mapped_effort));
                app_event_tx.send(AppEvent::PersistModelSelection {
                    model: target_model.clone(),
                    effort: mapped_effort,
                });
            }
            ModelMigrationOutcome::Exit => {
                return Some(AppExitInfo {
                    token_usage: TokenUsage::default(),
                    conversation_id: None,
                    update_action: None,
                });
            }
        }
    }

    None
}

pub(crate) struct App {
    pub(crate) server: Arc<ConversationManager>,
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) chat_widget: ChatWidget,
    pub(crate) auth_manager: Arc<AuthManager>,

    /// Config is stored here so we can recreate ChatWidgets as needed.
    pub(crate) config: Config,
    pub(crate) active_profile: Option<String>,

    pub(crate) file_search: FileSearchManager,

    pub(crate) transcript_cells: Vec<Arc<dyn HistoryCell>>,

    // Pager overlay state (Transcript or Static like Diff)
    pub(crate) overlay: Option<Overlay>,
    pub(crate) deferred_history_lines: Vec<Line<'static>>,
    has_emitted_history_lines: bool,

    pub(crate) enhanced_keys_supported: bool,

    /// Controls the animation thread that sends CommitTick events.
    pub(crate) commit_anim_running: Arc<AtomicBool>,

    // Esc-backtracking state grouped
    pub(crate) backtrack: crate::app_backtrack::BacktrackState,
    pub(crate) feedback: codex_feedback::CodexFeedback,
    /// Set when the user confirms an update; propagated on exit.
    pub(crate) pending_update_action: Option<UpdateAction>,

    // One-shot suppression of the next world-writable scan after user confirmation.
    skip_world_writable_scan_once: bool,
}

impl App {
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        tui: &mut tui::Tui,
        auth_manager: Arc<AuthManager>,
        mut config: Config,
        active_profile: Option<String>,
        initial_prompt: Option<String>,
        initial_images: Vec<PathBuf>,
        resume_selection: ResumeSelection,
        feedback: codex_feedback::CodexFeedback,
    ) -> Result<AppExitInfo> {
        use tokio_stream::StreamExt;
        let (app_event_tx, mut app_event_rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(app_event_tx);

        let exit_info =
            handle_model_migration_prompt_if_needed(tui, &mut config, &app_event_tx).await;
        if let Some(exit_info) = exit_info {
            return Ok(exit_info);
        }

        let conversation_manager = Arc::new(ConversationManager::new(
            auth_manager.clone(),
            SessionSource::Cli,
        ));

        let enhanced_keys_supported = tui.enhanced_keys_supported();

        let mut chat_widget = match resume_selection {
            ResumeSelection::StartFresh | ResumeSelection::Exit => {
                let init = crate::chatwidget::ChatWidgetInit {
                    config: config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: app_event_tx.clone(),
                    initial_prompt: initial_prompt.clone(),
                    initial_images: initial_images.clone(),
                    enhanced_keys_supported,
                    auth_manager: auth_manager.clone(),
                    feedback: feedback.clone(),
                };
                ChatWidget::new(init, conversation_manager.clone())
            }
            ResumeSelection::Resume(path) => {
                let resumed = conversation_manager
                    .resume_conversation_from_rollout(
                        config.clone(),
                        path.clone(),
                        auth_manager.clone(),
                    )
                    .await
                    .wrap_err_with(|| {
                        format!("Failed to resume session from {}", path.display())
                    })?;
                let init = crate::chatwidget::ChatWidgetInit {
                    config: config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: app_event_tx.clone(),
                    initial_prompt: initial_prompt.clone(),
                    initial_images: initial_images.clone(),
                    enhanced_keys_supported,
                    auth_manager: auth_manager.clone(),
                    feedback: feedback.clone(),
                };
                ChatWidget::new_from_existing(
                    init,
                    resumed.conversation,
                    resumed.session_configured,
                )
            }
        };

        chat_widget.maybe_prompt_windows_sandbox_enable();

        let file_search = FileSearchManager::new(config.cwd.clone(), app_event_tx.clone());
        #[cfg(not(debug_assertions))]
        let upgrade_version = crate::updates::get_upgrade_version(&config);

        let mut app = Self {
            server: conversation_manager,
            app_event_tx,
            chat_widget,
            auth_manager: auth_manager.clone(),
            config,
            active_profile,
            file_search,
            enhanced_keys_supported,
            transcript_cells: Vec::new(),
            overlay: None,
            deferred_history_lines: Vec::new(),
            has_emitted_history_lines: false,
            commit_anim_running: Arc::new(AtomicBool::new(false)),
            backtrack: BacktrackState::default(),
            feedback: feedback.clone(),
            pending_update_action: None,
            skip_world_writable_scan_once: false,
        };

        // On startup, if Auto mode (workspace-write) or ReadOnly is active, warn about world-writable dirs on Windows.
        #[cfg(target_os = "windows")]
        {
            let should_check = codex_core::get_platform_sandbox().is_some()
                && matches!(
                    app.config.sandbox_policy,
                    codex_core::protocol::SandboxPolicy::WorkspaceWrite { .. }
                        | codex_core::protocol::SandboxPolicy::ReadOnly
                )
                && !app
                    .config
                    .notices
                    .hide_world_writable_warning
                    .unwrap_or(false);
            if should_check {
                let cwd = app.config.cwd.clone();
                let env_map: std::collections::HashMap<String, String> = std::env::vars().collect();
                let tx = app.app_event_tx.clone();
                let logs_base_dir = app.config.codex_home.clone();
                Self::spawn_world_writable_scan(cwd, env_map, logs_base_dir, tx);
            }
        }

        #[cfg(not(debug_assertions))]
        if let Some(latest_version) = upgrade_version {
            app.handle_event(
                tui,
                AppEvent::InsertHistoryCell(Box::new(UpdateAvailableHistoryCell::new(
                    latest_version,
                    crate::update_action::get_update_action(),
                ))),
            )
            .await?;
        }

        let tui_events = tui.event_stream();
        tokio::pin!(tui_events);

        tui.frame_requester().schedule_frame();

        while select! {
            Some(event) = app_event_rx.recv() => {
                app.handle_event(tui, event).await?
            }
            Some(event) = tui_events.next() => {
                app.handle_tui_event(tui, event).await?
            }
        } {}
        tui.terminal.clear()?;
        Ok(AppExitInfo {
            token_usage: app.token_usage(),
            conversation_id: app.chat_widget.conversation_id(),
            update_action: app.pending_update_action,
        })
    }

    pub(crate) async fn handle_tui_event(
        &mut self,
        tui: &mut tui::Tui,
        event: TuiEvent,
    ) -> Result<bool> {
        if self.overlay.is_some() {
            let _ = self.handle_backtrack_overlay_event(tui, event).await?;
        } else {
            match event {
                TuiEvent::Key(key_event) => {
                    self.handle_key_event(tui, key_event).await;
                }
                TuiEvent::Paste(pasted) => {
                    // Many terminals convert newlines to \r when pasting (e.g., iTerm2),
                    // but tui-textarea expects \n. Normalize CR to LF.
                    // [tui-textarea]: https://github.com/rhysd/tui-textarea/blob/4d18622eeac13b309e0ff6a55a46ac6706da68cf/src/textarea.rs#L782-L783
                    // [iTerm2]: https://github.com/gnachman/iTerm2/blob/5d0c0d9f68523cbd0494dad5422998964a2ecd8d/sources/iTermPasteHelper.m#L206-L216
                    let pasted = pasted.replace("\r", "\n");
                    self.chat_widget.handle_paste(pasted);
                }
                TuiEvent::Draw => {
                    self.chat_widget.maybe_post_pending_notification(tui);
                    if self
                        .chat_widget
                        .handle_paste_burst_tick(tui.frame_requester())
                    {
                        return Ok(true);
                    }
                    tui.draw(
                        self.chat_widget.desired_height(tui.terminal.size()?.width),
                        |frame| {
                            self.chat_widget.render(frame.area(), frame.buffer);
                            if let Some((x, y)) = self.chat_widget.cursor_pos(frame.area()) {
                                frame.set_cursor_position((x, y));
                            }
                        },
                    )?;
                }
            }
        }
        Ok(true)
    }

    async fn handle_event(&mut self, tui: &mut tui::Tui, event: AppEvent) -> Result<bool> {
        match event {
            AppEvent::NewSession => {
                let summary = session_summary(
                    self.chat_widget.token_usage(),
                    self.chat_widget.conversation_id(),
                );
                let init = crate::chatwidget::ChatWidgetInit {
                    config: self.config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: self.app_event_tx.clone(),
                    initial_prompt: None,
                    initial_images: Vec::new(),
                    enhanced_keys_supported: self.enhanced_keys_supported,
                    auth_manager: self.auth_manager.clone(),
                    feedback: self.feedback.clone(),
                };
                self.chat_widget = ChatWidget::new(init, self.server.clone());
                if let Some(summary) = summary {
                    let mut lines: Vec<Line<'static>> = vec![summary.usage_line.clone().into()];
                    if let Some(command) = summary.resume_command {
                        let spans = vec!["To continue this session, run ".into(), command.cyan()];
                        lines.push(spans.into());
                    }
                    self.chat_widget.add_plain_history_lines(lines);
                }
                tui.frame_requester().schedule_frame();
            }
            AppEvent::InsertHistoryCell(cell) => {
                let cell: Arc<dyn HistoryCell> = cell.into();
                if let Some(Overlay::Transcript(t)) = &mut self.overlay {
                    t.insert_cell(cell.clone());
                    tui.frame_requester().schedule_frame();
                }
                self.transcript_cells.push(cell.clone());
                let mut display = cell.display_lines(tui.terminal.last_known_screen_size.width);
                if !display.is_empty() {
                    // Only insert a separating blank line for new cells that are not
                    // part of an ongoing stream. Streaming continuations should not
                    // accrue extra blank lines between chunks.
                    if !cell.is_stream_continuation() {
                        if self.has_emitted_history_lines {
                            display.insert(0, Line::from(""));
                        } else {
                            self.has_emitted_history_lines = true;
                        }
                    }
                    if self.overlay.is_some() {
                        self.deferred_history_lines.extend(display);
                    } else {
                        tui.insert_history_lines(display);
                    }
                }
            }
            AppEvent::StartCommitAnimation => {
                if self
                    .commit_anim_running
                    .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    let tx = self.app_event_tx.clone();
                    let running = self.commit_anim_running.clone();
                    thread::spawn(move || {
                        while running.load(Ordering::Relaxed) {
                            thread::sleep(Duration::from_millis(50));
                            tx.send(AppEvent::CommitTick);
                        }
                    });
                }
            }
            AppEvent::StopCommitAnimation => {
                self.commit_anim_running.store(false, Ordering::Release);
            }
            AppEvent::CommitTick => {
                self.chat_widget.on_commit_tick();
            }
            AppEvent::CodexEvent(event) => {
                self.chat_widget.handle_codex_event(event);
            }
            AppEvent::ConversationHistory(ev) => {
                self.on_conversation_history_for_backtrack(tui, ev).await?;
            }
            AppEvent::ExitRequest => {
                return Ok(false);
            }
            AppEvent::CodexOp(op) => self.chat_widget.submit_op(op),
            AppEvent::DiffResult(text) => {
                // Clear the in-progress state in the bottom pane
                self.chat_widget.on_diff_complete();
                // Enter alternate screen using TUI helper and build pager lines
                let _ = tui.enter_alt_screen();
                let pager_lines: Vec<ratatui::text::Line<'static>> = if text.trim().is_empty() {
                    vec!["No changes detected.".italic().into()]
                } else {
                    text.lines().map(ansi_escape_line).collect()
                };
                self.overlay = Some(Overlay::new_static_with_lines(
                    pager_lines,
                    "D I F F".to_string(),
                ));
                tui.frame_requester().schedule_frame();
            }
            AppEvent::CheckpointCommand { action, name } => match action {
                CheckpointAction::Save => self.handle_checkpoint_save(name),
                CheckpointAction::Load => self.handle_checkpoint_load(name),
            },
            AppEvent::TodoCommand { action } => {
                self.handle_todo_action(action);
            }
            AppEvent::AliasCommand { action } => {
                self.handle_alias_action(action);
            }
            AppEvent::PresetCommand { action } => {
                self.handle_preset_action(tui, action);
            }
            AppEvent::CommitCommand { action } => {
                self.handle_commit_action(action).await;
            }
            AppEvent::SetAutoCompact {
                enabled,
                threshold_percent,
            } => {
                let previous_enabled = self.chat_widget.auto_compact_enabled();
                let previous_threshold = self.chat_widget.auto_compact_threshold_percent();
                let new_threshold = threshold_percent.unwrap_or(previous_threshold);
                self.chat_widget
                    .set_auto_compact_settings(enabled, new_threshold);
                self.config.auto_compact = enabled;
                self.config.auto_compact_threshold_percent = new_threshold;

                if let Err(err) =
                    persist_auto_compact_settings(&self.config.codex_home, enabled, new_threshold)
                        .await
                {
                    tracing::error!(
                        error = %err,
                        "failed to persist auto-compaction preference"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Failed to save automatic compaction preference: {err:#}"
                    ));
                } else {
                    let (message, hint) = if enabled {
                        if previous_enabled && previous_threshold == new_threshold {
                            (
                                "Automatic compaction already enabled.".to_string(),
                                Some(format!(
                                    "Will run `/compact` automatically when less than {new_threshold}% of the context remains."
                                )),
                            )
                        } else if previous_enabled {
                            (
                                format!(
                                    "Automatic compaction threshold set to {new_threshold}%."
                                ),
                                Some(
                                    "We'll keep summarizing automatically when the remaining context falls below that percentage."
                                        .to_string(),
                                ),
                            )
                        } else {
                            (
                                format!(
                                    "Automatic compaction enabled ({new_threshold}% threshold)."
                                ),
                                Some(
                                    "Codex will run `/compact` when the remaining context drops below this threshold.".to_string(),
                                ),
                            )
                        }
                    } else if previous_enabled {
                        (
                            "Automatic compaction disabled.".to_string(),
                            Some("Run `/compact auto` to enable it again.".to_string()),
                        )
                    } else {
                        (
                            "Automatic compaction already disabled.".to_string(),
                            Some("Use `/compact auto` to enable automatic summaries.".to_string()),
                        )
                    };

                    self.chat_widget.add_info_message(message, hint);
                }
            }
            AppEvent::SetCheckpointAutomation { enabled } => {
                if enabled && let Err(err) = self.ensure_checkpoint_dir() {
                    self.chat_widget.add_error_message(format!(
                        "Failed to enable automatic checkpoints: {err:#}"
                    ));
                    return Ok(true);
                }
                self.auto_checkpoint_state = None;
                let previous = self.auto_checkpoint_enabled;
                self.auto_checkpoint_enabled = enabled;
                self.config.auto_checkpoint = enabled;
                self.chat_widget.set_auto_checkpoint_enabled(enabled);
                if let Err(err) = persist_auto_checkpoint(&self.config.codex_home, enabled).await {
                    tracing::error!(
                        error = %err,
                        "failed to persist auto-checkpoint preference"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Failed to save automatic checkpoint preference: {err:#}"
                    ));
                }
                if enabled {
                    let message = if previous {
                        "Automatic checkpoints already enabled."
                    } else {
                        "Automatic checkpoints enabled."
                    };
                    let hint = if previous {
                        Some("Checkpoints continue to save after each Codex response using names like abc123.".to_string())
                    } else {
                        Some("A checkpoint will be saved after each Codex response using names like abc123.".to_string())
                    };
                    self.chat_widget.add_info_message(message.to_string(), hint);
                } else {
                    let message = if previous {
                        "Automatic checkpoints disabled."
                    } else {
                        "Automatic checkpoints already disabled."
                    };
                    let hint = if previous {
                        Some(
                            "Run `/checkpoint auto` to enable automatic checkpoints again."
                                .to_string(),
                        )
                    } else {
                        Some("Automatic checkpoints remain disabled.".to_string())
                    };
                    self.chat_widget.add_info_message(message.to_string(), hint);
                }
            }
            AppEvent::AutoCheckpointTick => {
                if self.auto_checkpoint_enabled {
                    self.handle_auto_checkpoint_save();
                }
            }
            AppEvent::AutoCommitTick => {
                if self.auto_commit_enabled {
                    self.handle_commit_action(CommitAction::Perform {
                        message: None,
                        auto: true,
                    })
                    .await;
                }
            }
            AppEvent::StartFileSearch(query) => {
                if !query.is_empty() {
                    self.file_search.on_user_query(query);
                }
            }
            AppEvent::FileSearchResult { query, matches } => {
                self.chat_widget.apply_file_search_result(query, matches);
            }
            AppEvent::RateLimitSnapshotFetched(snapshot) => {
                self.chat_widget.on_rate_limit_snapshot(Some(snapshot));
            }
            AppEvent::UpdateReasoningEffort(effort) => {
                self.on_update_reasoning_effort(effort);
            }
            AppEvent::UpdateModel(model) => {
                self.chat_widget.set_model(&model);
                self.config.model = model.clone();
                if let Some(family) = find_family_for_model(&model) {
                    self.config.model_family = family;
                }
            }
            AppEvent::OpenReasoningPopup { model } => {
                self.chat_widget.open_reasoning_popup(model);
            }
            AppEvent::OpenFullAccessConfirmation { preset } => {
                self.chat_widget.open_full_access_confirmation(preset);
            }
            AppEvent::OpenWorldWritableWarningConfirmation {
                preset,
                sample_paths,
                extra_count,
                failed_scan,
            } => {
                self.chat_widget.open_world_writable_warning_confirmation(
                    preset,
                    sample_paths,
                    extra_count,
                    failed_scan,
                );
            }
            AppEvent::OpenFeedbackNote {
                category,
                include_logs,
            } => {
                self.chat_widget.open_feedback_note(category, include_logs);
            }
            AppEvent::OpenFeedbackConsent { category } => {
                self.chat_widget.open_feedback_consent(category);
            }
            AppEvent::OpenWindowsSandboxEnablePrompt { preset } => {
                self.chat_widget.open_windows_sandbox_enable_prompt(preset);
            }
            AppEvent::EnableWindowsSandboxForAuto { preset } => {
                #[cfg(target_os = "windows")]
                {
                    let profile = self.active_profile.as_deref();
                    let feature_key = Feature::WindowsSandbox.key();
                    match ConfigEditsBuilder::new(&self.config.codex_home)
                        .with_profile(profile)
                        .set_feature_enabled(feature_key, true)
                        .apply()
                        .await
                    {
                        Ok(()) => {
                            self.config.set_windows_sandbox_globally(true);
                            self.chat_widget.clear_forced_auto_mode_downgrade();
                            if let Some((sample_paths, extra_count, failed_scan)) =
                                self.chat_widget.world_writable_warning_details()
                            {
                                self.app_event_tx.send(
                                    AppEvent::OpenWorldWritableWarningConfirmation {
                                        preset: Some(preset.clone()),
                                        sample_paths,
                                        extra_count,
                                        failed_scan,
                                    },
                                );
                            } else {
                                self.app_event_tx.send(AppEvent::CodexOp(
                                    Op::OverrideTurnContext {
                                        cwd: None,
                                        approval_policy: Some(preset.approval),
                                        sandbox_policy: Some(preset.sandbox.clone()),
                                        model: None,
                                        effort: None,
                                        summary: None,
                                    },
                                ));
                                self.app_event_tx
                                    .send(AppEvent::UpdateAskForApprovalPolicy(preset.approval));
                                self.app_event_tx
                                    .send(AppEvent::UpdateSandboxPolicy(preset.sandbox.clone()));
                                self.chat_widget.add_info_message(
                                    "Enabled the Windows sandbox feature and switched to Auto mode."
                                        .to_string(),
                                    None,
                                );
                            }
                        }
                        Err(err) => {
                            tracing::error!(
                                error = %err,
                                "failed to enable Windows sandbox feature"
                            );
                            self.chat_widget.add_error_message(format!(
                                "Failed to enable the Windows sandbox feature: {err}"
                            ));
                        }
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = preset;
                }
            }
            AppEvent::PersistModelSelection { model, effort } => {
                let profile = self.active_profile.as_deref();
                match ConfigEditsBuilder::new(&self.config.codex_home)
                    .with_profile(profile)
                    .set_model(Some(model.as_str()), effort)
                    .apply()
                    .await
                {
                    Ok(()) => {
                        let effort_label = effort
                            .map(|eff| format!(" with {eff} reasoning"))
                            .unwrap_or_else(|| " with default reasoning".to_string());
                        if let Some(profile) = profile {
                            self.chat_widget.add_info_message(
                                format!(
                                    "Model changed to {model}{effort_label} for {profile} profile"
                                ),
                                None,
                            );
                        } else {
                            self.chat_widget.add_info_message(
                                format!("Model changed to {model}{effort_label}"),
                                None,
                            );
                        }
                    }
                    Err(err) => {
                        tracing::error!(
                            error = %err,
                            "failed to persist model selection"
                        );
                        if let Some(profile) = profile {
                            self.chat_widget.add_error_message(format!(
                                "Failed to save model for profile `{profile}`: {err}"
                            ));
                        } else {
                            self.chat_widget
                                .add_error_message(format!("Failed to save default model: {err}"));
                        }
                    }
                }
            }
            AppEvent::UpdateAskForApprovalPolicy(policy) => {
                self.chat_widget.set_approval_policy(policy);
            }
            AppEvent::UpdateSandboxPolicy(policy) => {
                #[cfg(target_os = "windows")]
                let policy_is_workspace_write_or_ro = matches!(
                    policy,
                    codex_core::protocol::SandboxPolicy::WorkspaceWrite { .. }
                        | codex_core::protocol::SandboxPolicy::ReadOnly
                );

                self.config.sandbox_policy = policy.clone();
                #[cfg(target_os = "windows")]
                if !matches!(policy, codex_core::protocol::SandboxPolicy::ReadOnly)
                    || codex_core::get_platform_sandbox().is_some()
                {
                    self.config.forced_auto_mode_downgraded_on_windows = false;
                }
                self.chat_widget.set_sandbox_policy(policy);

                // If sandbox policy becomes workspace-write or read-only, run the Windows world-writable scan.
                #[cfg(target_os = "windows")]
                {
                    // One-shot suppression if the user just confirmed continue.
                    if self.skip_world_writable_scan_once {
                        self.skip_world_writable_scan_once = false;
                        return Ok(true);
                    }

                    let should_check = codex_core::get_platform_sandbox().is_some()
                        && policy_is_workspace_write_or_ro
                        && !self.chat_widget.world_writable_warning_hidden();
                    if should_check {
                        let cwd = self.config.cwd.clone();
                        let env_map: std::collections::HashMap<String, String> =
                            std::env::vars().collect();
                        let tx = self.app_event_tx.clone();
                        let logs_base_dir = self.config.codex_home.clone();
                        Self::spawn_world_writable_scan(cwd, env_map, logs_base_dir, tx);
                    }
                }
            }
            AppEvent::SkipNextWorldWritableScan => {
                self.skip_world_writable_scan_once = true;
            }
            AppEvent::UpdateFullAccessWarningAcknowledged(ack) => {
                self.chat_widget.set_full_access_warning_acknowledged(ack);
            }
            AppEvent::UpdateWorldWritableWarningAcknowledged(ack) => {
                self.chat_widget
                    .set_world_writable_warning_acknowledged(ack);
            }
            AppEvent::UpdateRateLimitSwitchPromptHidden(hidden) => {
                self.chat_widget.set_rate_limit_switch_prompt_hidden(hidden);
            }
            AppEvent::PersistFullAccessWarningAcknowledged => {
                if let Err(err) = ConfigEditsBuilder::new(&self.config.codex_home)
                    .set_hide_full_access_warning(true)
                    .apply()
                    .await
                {
                    tracing::error!(
                        error = %err,
                        "failed to persist full access warning acknowledgement"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Failed to save full access confirmation preference: {err}"
                    ));
                }
            }
            AppEvent::PersistWorldWritableWarningAcknowledged => {
                if let Err(err) = ConfigEditsBuilder::new(&self.config.codex_home)
                    .set_hide_world_writable_warning(true)
                    .apply()
                    .await
                {
                    tracing::error!(
                        error = %err,
                        "failed to persist world-writable warning acknowledgement"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Failed to save Auto mode warning preference: {err}"
                    ));
                }
            }
            AppEvent::PersistRateLimitSwitchPromptHidden => {
                if let Err(err) = ConfigEditsBuilder::new(&self.config.codex_home)
                    .set_hide_rate_limit_model_nudge(true)
                    .apply()
                    .await
                {
                    tracing::error!(
                        error = %err,
                        "failed to persist rate limit switch prompt preference"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Failed to save rate limit reminder preference: {err}"
                    ));
                }
            }
            AppEvent::PersistModelMigrationPromptAcknowledged { migration_config } => {
                if let Err(err) = ConfigEditsBuilder::new(&self.config.codex_home)
                    .set_hide_model_migration_prompt(&migration_config, true)
                    .apply()
                    .await
                {
                    tracing::error!(error = %err, "failed to persist model migration prompt acknowledgement");
                    self.chat_widget.add_error_message(format!(
                        "Failed to save model migration prompt preference: {err}"
                    ));
                }
            }
            AppEvent::OpenApprovalsPopup => {
                self.chat_widget.open_approvals_popup();
            }
            AppEvent::OpenReviewBranchPicker(cwd) => {
                self.chat_widget.show_review_branch_picker(&cwd).await;
            }
            AppEvent::OpenReviewCommitPicker(cwd) => {
                self.chat_widget.show_review_commit_picker(&cwd).await;
            }
            AppEvent::OpenReviewCustomPrompt => {
                self.chat_widget.show_review_custom_prompt();
            }
            AppEvent::FullScreenApprovalRequest(request) => match request {
                ApprovalRequest::ApplyPatch { cwd, changes, .. } => {
                    let _ = tui.enter_alt_screen();
                    let diff_summary = DiffSummary::new(changes, cwd);
                    self.overlay = Some(Overlay::new_static_with_renderables(
                        vec![diff_summary.into()],
                        "P A T C H".to_string(),
                    ));
                }
                ApprovalRequest::Exec { command, .. } => {
                    let _ = tui.enter_alt_screen();
                    let full_cmd = strip_bash_lc_and_escape(&command);
                    let full_cmd_lines = highlight_bash_to_lines(&full_cmd);
                    self.overlay = Some(Overlay::new_static_with_lines(
                        full_cmd_lines,
                        "E X E C".to_string(),
                    ));
                }
            },
        }
        Ok(true)
    }

    pub(crate) fn token_usage(&self) -> codex_core::protocol::TokenUsage {
        self.chat_widget.token_usage()
    }

    fn on_update_reasoning_effort(&mut self, effort: Option<ReasoningEffortConfig>) {
        self.chat_widget.set_reasoning_effort(effort);
        self.config.model_reasoning_effort = effort;
    }

    fn handle_todo_action(&mut self, action: TodoAction) {
        match action {
            TodoAction::Add { text } => {
                self.chat_widget.add_todo_item(text);
            }
            TodoAction::List => {
                self.chat_widget.show_todo_list(None);
            }
            TodoAction::Complete { index } => {
                self.chat_widget.complete_todo(index);
            }
            TodoAction::Auto { enabled } => {
                self.chat_widget.set_todo_auto_enabled(enabled);
            }
        }
    }

    fn handle_alias_action(&mut self, action: AliasAction) {
        match action {
            AliasAction::Add { name } => {
                self.chat_widget.open_alias_prompt_editor(name);
            }
            AliasAction::Store { name, prompt } => {
                self.chat_widget.store_alias(name, prompt);
            }
            AliasAction::Remove { name } => {
                self.chat_widget.remove_alias(&name);
            }
            AliasAction::List => {
                self.chat_widget.list_aliases();
            }
        }
    }

    async fn handle_commit_action(&mut self, action: CommitAction) {
        match action {
            CommitAction::SetAuto { enabled } => {
                let previous = self.auto_commit_enabled;
                self.auto_commit_enabled = enabled;
                self.config.auto_commit = enabled;
                self.chat_widget.set_auto_commit_enabled(enabled);
                if let Err(err) = persist_auto_commit(&self.config.codex_home, enabled).await {
                    tracing::error!(
                        error = %err,
                        "failed to persist auto-commit preference"
                    );
                    self.chat_widget.add_error_message(format!(
                        "Failed to save automatic commit preference: {err:#}"
                    ));
                }

                let (message, hint) = if enabled {
                    if previous {
                        (
                            "Auto-commit already enabled.".to_string(),
                            Some(
                                "Codex will continue committing after each response using a generated summary.".to_string(),
                            ),
                        )
                    } else {
                        (
                            "Auto-commit enabled.".to_string(),
                            Some(
                                "Codex will commit after each response using a generated summary. Use `/commit auto off` to disable.".to_string(),
                            ),
                        )
                    }
                } else if previous {
                    (
                        "Auto-commit disabled.".to_string(),
                        Some("Run `/commit auto` to enable it again.".to_string()),
                    )
                } else {
                    (
                        "Auto-commit already disabled.".to_string(),
                        Some("Changes will no longer be committed automatically.".to_string()),
                    )
                };

                self.chat_widget.add_info_message(message, hint);
            }
            CommitAction::Perform { message, auto } => {
                let agent_summary = if auto {
                    self.chat_widget.take_last_agent_commit_summary()
                } else {
                    None
                };
                if let Err(err) = self.perform_commit(message, auto, agent_summary).await {
                    self.chat_widget.add_error_message(err);
                }
            }
        }
    }

    async fn perform_commit(
        &mut self,
        provided_message: Option<String>,
        auto: bool,
        agent_summary: Option<String>,
    ) -> Result<(), String> {
        if get_git_repo_root(&self.config.cwd).is_none() {
            return Err(
                "Cannot commit: current workspace is not inside a git repository.".to_string(),
            );
        }

        let status_output = Command::new("git")
            .args(["status", "--porcelain=v1"])
            .current_dir(&self.config.cwd)
            .output()
            .await
            .map_err(|err| format!("Failed to run git status: {err}"))?;

        if !status_output.status.success() {
            let stderr = String::from_utf8_lossy(&status_output.stderr);
            let message = stderr.trim();
            return Err(if message.is_empty() {
                "git status failed.".to_string()
            } else {
                format!("git status failed: {message}")
            });
        }

        let status_text = String::from_utf8(status_output.stdout)
            .map_err(|_| "git status produced invalid UTF-8 output.".to_string())?;

        if status_text.trim().is_empty() {
            if auto {
                self.chat_widget
                    .add_info_message("Auto-commit skipped (no changes).".to_string(), None);
            } else {
                self.chat_widget.add_info_message(
                    "Nothing to commit.".to_string(),
                    Some("Working tree is clean.".to_string()),
                );
            }
            return Ok(());
        }

        let (summary, preview) = Self::summarize_status(&status_text);

        let mut final_message = provided_message
            .map(|m| Self::sanitize_commit_message(&m))
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| summary.clone());

        if final_message.is_empty() {
            final_message = "Update workspace".to_string();
        }

        if auto && !final_message.to_lowercase().starts_with("auto commit") {
            final_message = format!("Auto commit: {final_message}");
        }

        let commit_body = agent_summary
            .map(|msg| msg.trim().to_string())
            .filter(|msg| !msg.is_empty());

        let add_output = Command::new("git")
            .args(["add", "--all"])
            .current_dir(&self.config.cwd)
            .output()
            .await
            .map_err(|err| format!("Failed to run git add: {err}"))?;

        if !add_output.status.success() {
            let stderr = String::from_utf8_lossy(&add_output.stderr);
            let message = stderr.trim();
            return Err(if message.is_empty() {
                "git add failed.".to_string()
            } else {
                format!("git add failed: {message}")
            });
        }

        let mut commit_command = Command::new("git");
        commit_command.arg("commit").arg("-m").arg(&final_message);
        if let Some(body) = commit_body.as_ref() {
            commit_command.arg("-m").arg(body);
        }
        let commit_output = commit_command
            .current_dir(&self.config.cwd)
            .output()
            .await
            .map_err(|err| format!("Failed to run git commit: {err}"))?;

        let stdout = String::from_utf8_lossy(&commit_output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&commit_output.stderr).to_string();

        if !commit_output.status.success() {
            let combined = format!("{stdout}{stderr}");
            if combined.contains("nothing to commit")
                || combined.contains("no changes added to commit")
            {
                let message = if auto {
                    "Auto-commit skipped (nothing to commit).".to_string()
                } else {
                    "Nothing to commit.".to_string()
                };
                self.chat_widget.add_info_message(message, None);
                return Ok(());
            }

            let message = stderr.trim();
            return Err(if message.is_empty() {
                "git commit failed.".to_string()
            } else {
                format!("git commit failed: {message}")
            });
        }

        let title = if auto {
            "Auto-commit created."
        } else {
            "Committed changes."
        };

        let mut hint_parts = Vec::new();
        hint_parts.push(format!("Message: {final_message}"));

        if !summary.is_empty() && summary != final_message {
            hint_parts.push(format!("Summary: {summary}"));
        }

        if let Some(body) = commit_body.as_ref()
            && let Some(first_line) = body
                .lines()
                .next()
                .map(str::trim)
                .filter(|line| !line.is_empty())
        {
            hint_parts.push(format!("Response: {first_line}"));
        }

        if !preview.is_empty() {
            let files = preview.join(", ");
            hint_parts.push(format!("Files: {files}"));
        }

        let first_stdout_line = stdout.lines().next().unwrap_or("").trim();
        if !first_stdout_line.is_empty() && !first_stdout_line.contains(&final_message) {
            hint_parts.push(first_stdout_line.to_string());
        }

        let hint = if hint_parts.is_empty() {
            None
        } else {
            Some(hint_parts.join(" • "))
        };

        self.chat_widget.add_info_message(title.to_string(), hint);

        Ok(())
    }

    fn sanitize_commit_message(raw: &str) -> String {
        if raw.trim().is_empty() {
            return String::new();
        }
        raw.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn summarize_status(status_text: &str) -> (String, Vec<String>) {
        let mut added = Vec::new();
        let mut modified = Vec::new();
        let mut deleted = Vec::new();
        let mut renamed = Vec::new();
        let mut preview = Vec::new();

        for line in status_text.lines() {
            if let Some(change) = Self::parse_status_line(line) {
                match change.category {
                    StatusCategory::Added => added.push(change.path.clone()),
                    StatusCategory::Modified => modified.push(change.path.clone()),
                    StatusCategory::Deleted => deleted.push(change.path.clone()),
                    StatusCategory::Renamed => renamed.push(change.path.clone()),
                    StatusCategory::Other => modified.push(change.path.clone()),
                }
                if preview.len() < 6 {
                    preview.push(change.preview);
                }
            }
        }

        let added = Self::dedup_summary_names(added);
        let modified = Self::dedup_summary_names(modified);
        let deleted = Self::dedup_summary_names(deleted);
        let renamed = Self::dedup_summary_names(renamed);

        let mut parts = Vec::new();
        if !added.is_empty() {
            parts.push(Self::format_summary_segment("Add", &added));
        }
        if !modified.is_empty() {
            parts.push(Self::format_summary_segment("Update", &modified));
        }
        if !deleted.is_empty() {
            parts.push(Self::format_summary_segment("Remove", &deleted));
        }
        if !renamed.is_empty() {
            parts.push(Self::format_summary_segment("Rename", &renamed));
        }

        let summary = if parts.is_empty() {
            "Update workspace".to_string()
        } else {
            parts.join("; ")
        };

        (summary, preview)
    }

    fn parse_status_line(line: &str) -> Option<ParsedChange> {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            return None;
        }

        if let Some(rest) = trimmed.strip_prefix("?? ") {
            let path = rest.trim().to_string();
            if path.is_empty() {
                return None;
            }
            return Some(ParsedChange {
                category: StatusCategory::Added,
                path: path.clone(),
                preview: format!("? {path}"),
            });
        }

        if trimmed.len() < 3 {
            return None;
        }

        let mut chars = trimmed.chars();
        let mut primary = chars.next().unwrap_or(' ');
        let secondary = chars.next().unwrap_or(' ');
        if primary == ' ' {
            primary = secondary;
        }
        if primary == ' ' {
            return None;
        }

        let path_part = &trimmed[3..];
        if path_part.trim().is_empty() {
            return None;
        }

        let category = match primary {
            'A' | 'C' => StatusCategory::Added,
            'M' => StatusCategory::Modified,
            'D' => StatusCategory::Deleted,
            'R' => StatusCategory::Renamed,
            _ => StatusCategory::Other,
        };

        if category == StatusCategory::Renamed
            && let Some(idx) = path_part.rfind("->")
        {
            let old = path_part[..idx].trim();
            let new = path_part[idx + 2..].trim();
            if new.is_empty() {
                return None;
            }
            return Some(ParsedChange {
                category,
                path: new.to_string(),
                preview: format!("R {} → {}", Self::short_name(old), Self::short_name(new)),
            });
        }

        let path = path_part.trim().to_string();
        let label = match primary {
            'A' | 'C' => 'A',
            'M' => 'M',
            'D' => 'D',
            'R' => 'R',
            '?' => '?',
            other => other,
        };

        Some(ParsedChange {
            category,
            path: path.clone(),
            preview: format!("{label} {path}"),
        })
    }

    fn dedup_summary_names(mut items: Vec<String>) -> Vec<String> {
        items.retain(|item| !item.is_empty());
        items.sort();
        items.dedup();
        items
    }

    fn format_summary_segment(action: &str, files: &[String]) -> String {
        match files.len() {
            0 => String::new(),
            1 => format!("{action} {}", Self::short_name(&files[0])),
            2 => format!(
                "{action} {} and {}",
                Self::short_name(&files[0]),
                Self::short_name(&files[1])
            ),
            _ => format!(
                "{action} {}, {} and {} more",
                Self::short_name(&files[0]),
                Self::short_name(&files[1]),
                files.len() - 2
            ),
        }
    }

    fn short_name(path: &str) -> String {
        let p = Path::new(path);
        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
            name.to_string()
        } else if let Some(component) = p.components().next_back() {
            component.as_os_str().to_string_lossy().to_string()
        } else {
            path.to_string()
        }
    }

    fn handle_checkpoint_save(&mut self, name: Option<String>) {
        match self.create_checkpoint(name) {
            Ok((checkpoint_name, path)) => {
                let hint = format!("Saved to {}", path.display());
                self.chat_widget
                    .add_info_message(format!("Checkpoint '{checkpoint_name}' saved."), Some(hint));
            }
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to save checkpoint: {err:#}"));
            }
        }
    }

    fn handle_auto_checkpoint_save(&mut self) {
        if !self.auto_checkpoint_enabled {
            return;
        }

        let context = self.collect_checkpoint_context();
        let session_id = self
            .chat_widget
            .conversation_id()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown-session".to_string());

        let sanitized_session = Self::sanitize_session_id(&session_id);
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let file_name = format!("{sanitized_session}.md");
        let dir = match self.ensure_checkpoint_dir() {
            Ok(dir) => dir,
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to update auto checkpoint: {err:#}"));
                return;
            }
        };
        if let Err(err) = Self::migrate_legacy_auto_checkpoint(&dir, &sanitized_session) {
            tracing::warn!(
                error = %err,
                "failed to migrate legacy auto checkpoint filename"
            );
        }
        let path = dir.join(file_name);

        match self.append_auto_checkpoint(&path, &session_id, &date, context) {
            Ok(true) => {
                let file_display = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.to_string())
                    .unwrap_or_else(|| path.display().to_string());
                self.chat_widget.add_info_message(
                    format!("Auto checkpoint `{file_display}` updated."),
                    Some(path.display().to_string()),
                );
            }
            Ok(false) => {
                // Nothing new to append; stay silent.
            }
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to update auto checkpoint: {err:#}"));
            }
        }
    }

    fn append_auto_checkpoint(
        &mut self,
        path: &Path,
        session_id: &str,
        date: &str,
        context: CheckpointContext,
    ) -> Result<bool> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!("failed to create checkpoint directory {}", parent.display())
            })?;
        }

        if self
            .auto_checkpoint_state
            .as_ref()
            .map(|state| state.path != path)
            .unwrap_or(true)
        {
            self.initialize_auto_checkpoint_file(path, session_id, date)?;
            self.auto_checkpoint_state = Some(AutoCheckpointState {
                path: path.to_path_buf(),
                user_count: 0,
                response_count: 0,
                plan_count: 0,
            });
        }

        let Some(state) = self.auto_checkpoint_state.as_mut() else {
            return Err(eyre!("auto-checkpoint state missing after initialization"));
        };

        state.user_count = state.user_count.min(context.user_prompts.len());
        state.response_count = state.response_count.min(context.agent_responses.len());
        state.plan_count = state.plan_count.min(context.plan_updates.len());

        let mut body = String::new();
        let timestamp = Utc::now().to_rfc3339();

        if context.user_prompts.len() > state.user_count {
            for (offset, prompt) in context.user_prompts[state.user_count..].iter().enumerate() {
                let number = state.user_count + offset + 1;
                let _ = writeln!(body, "\n### User Prompt {number}");
                let _ = writeln!(body, "{}", prompt.trim());
            }
            state.user_count = context.user_prompts.len();
        }

        if context.agent_responses.len() > state.response_count {
            for (offset, response) in context.agent_responses[state.response_count..]
                .iter()
                .enumerate()
            {
                let number = state.response_count + offset + 1;
                let _ = writeln!(body, "\n### Assistant Response {number}");
                let _ = writeln!(body, "{}", response.trim());
            }
            state.response_count = context.agent_responses.len();
        }

        if context.plan_updates.len() > state.plan_count {
            for (offset, plan) in context.plan_updates[state.plan_count..].iter().enumerate() {
                let number = state.plan_count + offset + 1;
                let _ = writeln!(body, "\n### Plan Update {number}");
                let plan_markdown = Self::format_plan_update_markdown(plan);
                if plan_markdown.is_empty() {
                    let _ = writeln!(body, "(no plan items)");
                } else {
                    let _ = writeln!(body, "{plan_markdown}");
                }
            }
            state.plan_count = context.plan_updates.len();
        }

        if body.trim().is_empty() {
            return Ok(false);
        }

        let mut block = String::new();
        let _ = writeln!(block, "\n## Auto Update {timestamp}");
        block.push_str(&body);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .wrap_err_with(|| format!("failed to open auto checkpoint {}", path.display()))?;
        IoWrite::write_all(&mut file, block.as_bytes())
            .wrap_err_with(|| format!("failed to append to auto checkpoint {}", path.display()))?;

        Ok(true)
    }

    fn migrate_legacy_auto_checkpoint(dir: &Path, sanitized_session: &str) -> Result<()> {
        let target = dir.join(format!("{sanitized_session}.md"));
        if target.exists() {
            return Ok(());
        }

        let read_dir = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => {
                return Err(err).wrap_err_with(|| format!("failed to read {}", dir.display()));
            }
        };

        for entry in read_dir {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if !path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("md"))
                .unwrap_or(false)
            {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if stem == sanitized_session {
                // Another process already migrated the file.
                return Ok(());
            }
            if stem.ends_with(sanitized_session)
                && Self::has_legacy_auto_checkpoint_prefix(
                    &stem[..stem.len() - sanitized_session.len()],
                )
            {
                fs::rename(&path, &target).wrap_err_with(|| {
                    format!(
                        "failed to migrate legacy auto checkpoint from {} to {}",
                        path.display(),
                        target.display()
                    )
                })?;
                break;
            }
        }

        Ok(())
    }

    fn has_legacy_auto_checkpoint_prefix(prefix: &str) -> bool {
        let bytes = prefix.as_bytes();
        if bytes.len() != 11 {
            return false;
        }
        bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes[10] == b'-'
            && bytes[..4].iter().all(u8::is_ascii_digit)
            && bytes[5..7].iter().all(u8::is_ascii_digit)
            && bytes[8..10].iter().all(u8::is_ascii_digit)
    }

    fn handle_checkpoint_load(&mut self, name: Option<String>) {
        if let Some(name) = name {
            let sanitized = Self::sanitize_checkpoint_name(&name);
            if sanitized.is_empty() {
                self.chat_widget.add_error_message(
                    "Checkpoint name must contain at least one alphanumeric character.".to_string(),
                );
                return;
            }
            let path = self.checkpoint_dir().join(format!("{sanitized}.md"));
            if !path.exists() {
                self.chat_widget.add_error_message(format!(
                    "Checkpoint '{sanitized}' not found in {}.",
                    self.checkpoint_dir().display()
                ));
                if let Ok(names) = self.list_checkpoint_names()
                    && !names.is_empty()
                {
                    self.chat_widget.add_info_message(
                        "Available checkpoints:".to_string(),
                        Some(names.join(", ")),
                    );
                }
                return;
            }
            match fs::read_to_string(&path) {
                Ok(contents) => {
                    let composer_text =
                        format!("Continue from checkpoint `{sanitized}`.\n\n{contents}");
                    self.chat_widget.set_composer_text(composer_text);
                    self.chat_widget.add_info_message(
                        format!(
                            "Checkpoint '{sanitized}' loaded into the composer. Review and send when ready."
                        ),
                        Some(path.display().to_string()),
                    );
                }
                Err(err) => {
                    self.chat_widget.add_error_message(format!(
                        "Failed to load checkpoint '{sanitized}': {err}"
                    ));
                }
            }
        } else {
            match self.list_checkpoint_names() {
                Ok(names) if names.is_empty() => {
                    self.chat_widget.add_info_message(
                        "No checkpoints saved yet.".to_string(),
                        Some("Use `/checkpoint save` to create one.".to_string()),
                    );
                }
                Ok(names) => {
                    self.chat_widget.add_info_message(
                        "Available checkpoints:".to_string(),
                        Some(names.join(", ")),
                    );
                }
                Err(err) => {
                    self.chat_widget
                        .add_error_message(format!("Failed to enumerate checkpoints: {err:#}"));
                }
            }
        }
    }

    fn initialize_auto_checkpoint_file(
        &self,
        path: &Path,
        session_id: &str,
        date: &str,
    ) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .wrap_err_with(|| format!("failed to create auto checkpoint {}", path.display()))?;

        let mut header = String::new();
        let timestamp = Utc::now().to_rfc3339();
        let _ = writeln!(header, "# Auto Checkpoint {date} ({session_id})");
        let _ = writeln!(header, "- Created at: {timestamp}");
        let _ = writeln!(header, "- Session ID: `{session_id}`");
        let _ = writeln!(
            header,
            "- Working directory: `{}`",
            self.config.cwd.display()
        );
        let _ = writeln!(header, "- Auto mode: enabled");
        header.push('\n');

        IoWrite::write_all(&mut file, header.as_bytes()).wrap_err_with(|| {
            format!(
                "failed to write header for auto checkpoint {}",
                path.display()
            )
        })?;

        Ok(())
    }

    fn format_plan_update_markdown(update: &UpdatePlanArgs) -> String {
        let mut out = String::new();

        if let Some(explanation) = update
            .explanation
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            let _ = writeln!(out, "{explanation}");
        }

        if update.plan.is_empty() {
            let _ = writeln!(out, "(no plan items)");
        } else {
            for PlanItemArg { step, status } in &update.plan {
                let checkbox = match status {
                    StepStatus::Completed => "[x]",
                    StepStatus::InProgress => "[~]",
                    StepStatus::Pending => "[ ]",
                };
                let _ = writeln!(out, "- {} {}", checkbox, step.trim());
            }
        }

        out.trim_end().to_string()
    }

    fn create_checkpoint(&self, name: Option<String>) -> Result<(String, PathBuf)> {
        let chosen_name = self.choose_checkpoint_name(name);
        let dir = self.ensure_checkpoint_dir()?;
        let file_path = dir.join(format!("{chosen_name}.md"));
        let contents = self.build_checkpoint_markdown(&chosen_name);
        fs::write(&file_path, contents)
            .wrap_err_with(|| format!("failed to write checkpoint {}", file_path.display()))?;
        Ok((chosen_name, file_path))
    }

    fn checkpoint_dir(&self) -> PathBuf {
        self.config.cwd.join(".codex").join("checkpoints")
    }

    fn ensure_checkpoint_dir(&self) -> Result<PathBuf> {
        let dir = self.checkpoint_dir();
        fs::create_dir_all(&dir)
            .wrap_err_with(|| format!("failed to create checkpoint directory {}", dir.display()))?;
        Ok(dir)
    }

    fn choose_checkpoint_name(&self, provided: Option<String>) -> String {
        if let Some(name) = provided
            .and_then(|n| {
                let trimmed = n.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .map(|candidate| Self::sanitize_checkpoint_name(&candidate))
            .filter(|sanitized| !sanitized.is_empty())
        {
            name
        } else {
            Self::generate_random_checkpoint_name()
        }
    }

    fn sanitize_checkpoint_name(input: &str) -> String {
        let mut result = String::new();
        let mut last_was_dash = false;
        for ch in input.chars() {
            let normalized = if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else if ch == '_' {
                ch
            } else {
                '-'
            };
            if normalized == '-' {
                if result.is_empty() || last_was_dash {
                    continue;
                }
                last_was_dash = true;
                result.push(normalized);
            } else {
                last_was_dash = false;
                result.push(normalized);
            }
            if result.len() >= 48 {
                break;
            }
        }
        while matches!(result.chars().last(), Some('-') | Some('_')) {
            result.pop();
        }
        result
    }

    fn sanitize_session_id(input: &str) -> String {
        let mut result = String::new();
        let mut last_was_dash = false;
        for ch in input.chars() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                result.push(ch);
                last_was_dash = false;
            } else if !last_was_dash {
                result.push('-');
                last_was_dash = true;
            }
            if result.len() >= 64 {
                break;
            }
        }
        while result.ends_with('-') {
            result.pop();
        }
        if result.is_empty() {
            "session".to_string()
        } else {
            result
        }
    }

    fn generate_random_checkpoint_name() -> String {
        let date_prefix = Utc::now().format("%Y-%m-%d").to_string();
        let mut rng = rand::rng();
        let hash: String = (0..6)
            .map(|_| {
                let value: u8 = rng.random_range(0..36);
                if value < 10 {
                    (b'0' + value) as char
                } else {
                    (b'a' + (value - 10)) as char
                }
            })
            .collect();
        format!("{date_prefix}-{hash}")
    }

    fn build_checkpoint_markdown(&self, name: &str) -> String {
        let mut out = String::new();
        let timestamp = Utc::now().to_rfc3339();
        let _ = writeln!(out, "# Checkpoint {name}");
        out.push('\n');
        let _ = writeln!(out, "- Saved at: {timestamp}");
        let _ = writeln!(out, "- Working directory: `{}`", self.config.cwd.display());
        if let Some(conversation_id) = self.chat_widget.conversation_id() {
            let _ = writeln!(out, "- Conversation ID: `{conversation_id}`");
        }
        out.push('\n');

        let context = self.collect_checkpoint_context();
        if context.user_prompts.is_empty()
            && context.agent_responses.is_empty()
            && context.plan_updates.is_empty()
        {
            out.push_str("_No conversation history captured yet._\n");
            return out;
        }

        if !context.user_prompts.is_empty() {
            out.push_str("## User Prompts\n\n");
            for (idx, prompt) in context.user_prompts.iter().enumerate() {
                let _ = writeln!(out, "### Prompt {}", idx + 1);
                out.push('\n');
                Self::append_markdown_block(&mut out, prompt);
            }
        }

        if !context.agent_responses.is_empty() {
            out.push_str("## Codex Responses\n\n");
            for (idx, response) in context.agent_responses.iter().enumerate() {
                let _ = writeln!(out, "### Response {}", idx + 1);
                out.push('\n');
                Self::append_markdown_block(&mut out, response);
            }
        }

        if !context.plan_updates.is_empty() {
            out.push_str("## Plan Updates\n\n");
            for (idx, update) in context.plan_updates.iter().enumerate() {
                let _ = writeln!(out, "### Update {}", idx + 1);
                out.push('\n');
                if let Some(expl) = update
                    .explanation
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    let _ = writeln!(out, "> {expl}");
                    out.push('\n');
                }
                if update.plan.is_empty() {
                    out.push_str("_No steps provided._\n\n");
                } else {
                    for step in &update.plan {
                        let marker = Self::step_status_marker(&step.status);
                        let trimmed_step = step.step.trim();
                        let _ = writeln!(out, "- [{marker}] {trimmed_step}");
                    }
                    out.push('\n');
                }
            }
        }

        out
    }

    fn list_checkpoint_names(&self) -> Result<Vec<String>> {
        let dir = self.checkpoint_dir();
        let read_dir = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => {
                return Err(err).wrap_err_with(|| format!("failed to read {}", dir.display()));
            }
        };
        let mut names: Vec<String> = Vec::new();
        for entry in read_dir {
            let entry = entry?;
            let path = entry.path();
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("md"))
                .unwrap_or(false)
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    fn collect_checkpoint_context(&self) -> CheckpointContext {
        let mut prompts: Vec<String> = Vec::new();
        let mut responses: Vec<String> = Vec::new();
        let mut plan_updates: Vec<UpdatePlanArgs> = Vec::new();

        for cell in &self.transcript_cells {
            let history = cell.as_ref();
            if let Some(user) = history.as_any().downcast_ref::<UserHistoryCell>() {
                let text = user.message.trim();
                if !text.is_empty() {
                    prompts.push(text.to_string());
                }
                continue;
            }
            if let Some(agent) = history.as_any().downcast_ref::<AgentMessageCell>() {
                let lines = agent.transcript_lines(u16::MAX);
                let plain = Self::lines_to_plain(&lines);
                let first_segment = plain.trim_start().starts_with('•');
                let cleaned = Self::clean_agent_text(&plain);
                if cleaned.is_empty() {
                    continue;
                }
                if !first_segment && !responses.is_empty() {
                    if let Some(last) = responses.last_mut() {
                        if !last.ends_with('\n') {
                            last.push('\n');
                        }
                        last.push_str(&cleaned);
                    }
                } else {
                    responses.push(cleaned);
                }
                continue;
            }
            if let Some(plan) = history.as_any().downcast_ref::<PlanUpdateCell>() {
                plan_updates.push(plan.to_update_args());
            }
        }

        CheckpointContext {
            user_prompts: prompts,
            agent_responses: responses,
            plan_updates,
        }
    }

    fn lines_to_plain(lines: &[Line<'static>]) -> String {
        let mut out = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            for span in &line.spans {
                out.push_str(span.content.as_ref());
            }
        }
        out.trim().to_string()
    }

    fn clean_agent_text(text: &str) -> String {
        let trimmed = text.trim();
        let without_bullet = trimmed
            .strip_prefix("• ")
            .or_else(|| trimmed.strip_prefix('•'))
            .unwrap_or(trimmed);
        without_bullet.trim().to_string()
    }

    fn append_markdown_block(out: &mut String, text: &str) {
        out.push_str("```\n");
        out.push_str(text);
        if !text.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("```\n\n");
    }

    fn step_status_marker(status: &StepStatus) -> &'static str {
        match status {
            StepStatus::Completed => "x",
            StepStatus::InProgress => "~",
            StepStatus::Pending => " ",
        }
    }

    async fn handle_key_event(&mut self, tui: &mut tui::Tui, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Char('t'),
                modifiers: crossterm::event::KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } => {
                // Enter alternate screen and set viewport to full size.
                let _ = tui.enter_alt_screen();
                self.overlay = Some(Overlay::new_transcript(self.transcript_cells.clone()));
                tui.frame_requester().schedule_frame();
            }
            // Esc primes/advances backtracking only in normal (not working) mode
            // with the composer focused and empty. In any other state, forward
            // Esc so the active UI (e.g. status indicator, modals, popups)
            // handles it.
            KeyEvent {
                code: KeyCode::Esc,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                if self.chat_widget.is_normal_backtrack_mode()
                    && self.chat_widget.composer_is_empty()
                {
                    self.handle_backtrack_esc_key(tui);
                } else {
                    self.chat_widget.handle_key_event(key_event);
                }
            }
            // Enter confirms backtrack when primed + count > 0. Otherwise pass to widget.
            KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            } if self.backtrack.primed
                && self.backtrack.nth_user_message != usize::MAX
                && self.chat_widget.composer_is_empty() =>
            {
                // Delegate to helper for clarity; preserves behavior.
                self.confirm_backtrack_from_main();
            }
            KeyEvent {
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                // Any non-Esc key press should cancel a primed backtrack.
                // This avoids stale "Esc-primed" state after the user starts typing
                // (even if they later backspace to empty).
                if key_event.code != KeyCode::Esc && self.backtrack.primed {
                    self.reset_backtrack_state();
                }
                self.chat_widget.handle_key_event(key_event);
            }
            _ => {
                // Ignore Release key events.
            }
        };
    }

    #[cfg(target_os = "windows")]
    fn spawn_world_writable_scan(
        cwd: PathBuf,
        env_map: std::collections::HashMap<String, String>,
        logs_base_dir: PathBuf,
        tx: AppEventSender,
    ) {
        #[inline]
        fn normalize_windows_path_for_display(p: &std::path::Path) -> String {
            let canon = dunce::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
            canon.display().to_string().replace('/', "\\")
        }
        tokio::task::spawn_blocking(move || {
            let result = codex_windows_sandbox::preflight_audit_everyone_writable(
                &cwd,
                &env_map,
                Some(logs_base_dir.as_path()),
            );
            if let Ok(ref paths) = result
                && !paths.is_empty()
            {
                let as_strings: Vec<String> = paths
                    .iter()
                    .map(|p| normalize_windows_path_for_display(p))
                    .collect();
                let sample_paths: Vec<String> = as_strings.iter().take(3).cloned().collect();
                let extra_count = if as_strings.len() > sample_paths.len() {
                    as_strings.len() - sample_paths.len()
                } else {
                    0
                };

                tx.send(AppEvent::OpenWorldWritableWarningConfirmation {
                    preset: None,
                    sample_paths,
                    extra_count,
                    failed_scan: false,
                });
            } else if result.is_err() {
                // Scan failed: still warn, but with no examples and mark as failed.
                let sample_paths: Vec<String> = Vec::new();
                let extra_count = 0usize;
                tx.send(AppEvent::OpenWorldWritableWarningConfirmation {
                    preset: None,
                    sample_paths,
                    extra_count,
                    failed_scan: true,
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_backtrack::BacktrackState;
    use crate::app_backtrack::user_count;
    use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
    use crate::file_search::FileSearchManager;
    use crate::history_cell::AgentMessageCell;
    use crate::history_cell::HistoryCell;
    use crate::history_cell::UserHistoryCell;
    use crate::history_cell::new_session_info;
    use codex_core::AuthManager;
    use codex_core::CodexAuth;
    use codex_core::ConversationManager;
    use codex_core::protocol::SessionConfiguredEvent;
    use codex_protocol::ConversationId;
    use ratatui::prelude::Line;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    fn make_test_app() -> App {
        let (chat_widget, app_event_tx, _rx, _op_rx) = make_chatwidget_manual_with_sender();
        let config = chat_widget.config_ref().clone();
        let server = Arc::new(ConversationManager::with_auth(CodexAuth::from_api_key(
            "Test API Key",
        )));
        let auth_manager =
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
        let file_search = FileSearchManager::new(config.cwd.clone(), app_event_tx.clone());

        App {
            server,
            app_event_tx,
            chat_widget,
            auth_manager,
            config,
            active_profile: None,
            file_search,
            transcript_cells: Vec::new(),
            overlay: None,
            deferred_history_lines: Vec::new(),
            has_emitted_history_lines: false,
            enhanced_keys_supported: false,
            commit_anim_running: Arc::new(AtomicBool::new(false)),
            backtrack: BacktrackState::default(),
            feedback: codex_feedback::CodexFeedback::new(),
            pending_update_action: None,
            skip_world_writable_scan_once: false,
        }
    }

    #[test]
    fn model_migration_prompt_only_shows_for_deprecated_models() {
        assert!(should_show_model_migration_prompt("gpt-5", "gpt-5.1", None));
        assert!(should_show_model_migration_prompt(
            "gpt-5-codex",
            "gpt-5.1-codex",
            None
        ));
        assert!(should_show_model_migration_prompt(
            "gpt-5-codex-mini",
            "gpt-5.1-codex-mini",
            None
        ));
        assert!(!should_show_model_migration_prompt(
            "gpt-5.1-codex",
            "gpt-5.1-codex",
            None
        ));
    }

    #[test]
    fn model_migration_prompt_respects_hide_flag_and_self_target() {
        assert!(!should_show_model_migration_prompt(
            "gpt-5",
            "gpt-5.1",
            Some(true)
        ));
        assert!(!should_show_model_migration_prompt(
            "gpt-5.1", "gpt-5.1", None
        ));
    }

    #[test]
    fn update_reasoning_effort_updates_config() {
        let mut app = make_test_app();
        app.config.model_reasoning_effort = Some(ReasoningEffortConfig::Medium);
        app.chat_widget
            .set_reasoning_effort(Some(ReasoningEffortConfig::Medium));

        app.on_update_reasoning_effort(Some(ReasoningEffortConfig::High));

        assert_eq!(
            app.config.model_reasoning_effort,
            Some(ReasoningEffortConfig::High)
        );
        assert_eq!(
            app.chat_widget.config_ref().model_reasoning_effort,
            Some(ReasoningEffortConfig::High)
        );
    }

    #[test]
    fn backtrack_selection_with_duplicate_history_targets_unique_turn() {
        let mut app = make_test_app();

        let user_cell = |text: &str| -> Arc<dyn HistoryCell> {
            Arc::new(UserHistoryCell {
                message: text.to_string(),
            }) as Arc<dyn HistoryCell>
        };
        let agent_cell = |text: &str| -> Arc<dyn HistoryCell> {
            Arc::new(AgentMessageCell::new(
                vec![Line::from(text.to_string())],
                true,
            )) as Arc<dyn HistoryCell>
        };

        let make_header = |is_first| {
            let event = SessionConfiguredEvent {
                session_id: ConversationId::new(),
                model: "gpt-test".to_string(),
                reasoning_effort: None,
                history_log_id: 0,
                history_entry_count: 0,
                initial_messages: None,
                rollout_path: PathBuf::new(),
            };
            Arc::new(new_session_info(
                app.chat_widget.config_ref(),
                event,
                is_first,
            )) as Arc<dyn HistoryCell>
        };

        // Simulate the transcript after trimming for a fork, replaying history, and
        // appending the edited turn. The session header separates the retained history
        // from the forked conversation's replayed turns.
        app.transcript_cells = vec![
            make_header(true),
            user_cell("first question"),
            agent_cell("answer first"),
            user_cell("follow-up"),
            agent_cell("answer follow-up"),
            make_header(false),
            user_cell("first question"),
            agent_cell("answer first"),
            user_cell("follow-up (edited)"),
            agent_cell("answer edited"),
        ];

        assert_eq!(user_count(&app.transcript_cells), 2);

        app.backtrack.base_id = Some(ConversationId::new());
        app.backtrack.primed = true;
        app.backtrack.nth_user_message = user_count(&app.transcript_cells).saturating_sub(1);

        app.confirm_backtrack_from_main();

        let (_, nth, prefill) = app.backtrack.pending.clone().expect("pending backtrack");
        assert_eq!(nth, 1);
        assert_eq!(prefill, "follow-up (edited)");
    }

    #[test]
    fn session_summary_skip_zero_usage() {
        assert!(session_summary(TokenUsage::default(), None).is_none());
    }

    #[test]
    fn session_summary_includes_resume_hint() {
        let usage = TokenUsage {
            input_tokens: 10,
            output_tokens: 2,
            total_tokens: 12,
            ..Default::default()
        };
        let conversation =
            ConversationId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();

        let summary = session_summary(usage, Some(conversation)).expect("summary");
        assert_eq!(
            summary.usage_line,
            "Token usage: total=12 input=10 output=2"
        );
        assert_eq!(
            summary.resume_command,
            Some("codex resume 123e4567-e89b-12d3-a456-426614174000".to_string())
        );
    }
}
