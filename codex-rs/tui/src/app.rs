use crate::app_backtrack::BacktrackState;
use crate::app_event::AppEvent;
use crate::app_event::CheckpointAction;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ApprovalRequest;
use crate::chatwidget::ChatWidget;
use crate::chatwidget::prompts_equivalent;
use crate::diff_render::DiffSummary;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::file_search::FileSearchManager;
use crate::history_cell::AgentMessageCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::PlanUpdateCell;
use crate::history_cell::UserHistoryCell;
use crate::pager_overlay::Overlay;
use crate::render::highlight::highlight_bash_to_lines;
use crate::resume_picker::ResumeSelection;
use crate::tui;
use crate::tui::TuiEvent;
use chrono::Utc;
use codex_ansi_escape::ansi_escape_line;
use codex_core::AuthManager;
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::config::persist_alarm_script;
use codex_core::config::persist_global_prompt;
use codex_core::config::persist_model_selection;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::SessionSource;
use codex_core::protocol::TokenUsage;
use codex_core::protocol_config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::ConversationId;
use codex_protocol::plan_tool::StepStatus;
use codex_protocol::plan_tool::UpdatePlanArgs;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use rand::Rng;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::fmt::Write as _;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use tokio::select;
use tokio::sync::mpsc::unbounded_channel;
// use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AppExitInfo {
    pub token_usage: TokenUsage,
    pub conversation_id: Option<ConversationId>,
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
    pub(crate) auto_checkpoint_enabled: bool,

    // Esc-backtracking state grouped
    pub(crate) backtrack: crate::app_backtrack::BacktrackState,
}

impl App {
    pub async fn run(
        tui: &mut tui::Tui,
        auth_manager: Arc<AuthManager>,
        config: Config,
        active_profile: Option<String>,
        initial_prompt: Option<String>,
        initial_images: Vec<PathBuf>,
        resume_selection: ResumeSelection,
    ) -> Result<AppExitInfo> {
        use tokio_stream::StreamExt;
        let (app_event_tx, mut app_event_rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(app_event_tx);

        let conversation_manager = Arc::new(ConversationManager::new(
            auth_manager.clone(),
            SessionSource::Cli,
        ));

        let enhanced_keys_supported = tui.enhanced_keys_supported();

        let sanitized_cli_prompt = initial_prompt
            .as_ref()
            .and_then(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .filter(|prompt| {
                config
                    .global_prompt
                    .as_ref()
                    .map(|gp| !prompts_equivalent(prompt, gp))
                    .unwrap_or(true)
            });

        let chat_widget = match resume_selection {
            ResumeSelection::StartFresh | ResumeSelection::Exit => {
                let init = crate::chatwidget::ChatWidgetInit {
                    config: config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: app_event_tx.clone(),
                    initial_prompt: sanitized_cli_prompt.clone(),
                    initial_images: initial_images.clone(),
                    enhanced_keys_supported,
                    auth_manager: auth_manager.clone(),
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
                    initial_prompt: sanitized_cli_prompt.clone(),
                    initial_images: initial_images.clone(),
                    enhanced_keys_supported,
                    auth_manager: auth_manager.clone(),
                };
                ChatWidget::new_from_existing(
                    init,
                    resumed.conversation,
                    resumed.session_configured,
                )
            }
        };

        let file_search = FileSearchManager::new(config.cwd.clone(), app_event_tx.clone());

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
            auto_checkpoint_enabled: false,
            backtrack: BacktrackState::default(),
        };

        let onboarding_notice = Line::from(
            "[Codex Super] Auto-interaction mode active: trust, login, and approvals run automatically.",
        );
        tui.insert_history_lines(vec![onboarding_notice]);
        app.has_emitted_history_lines = true;

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
                            frame.render_widget_ref(&self.chat_widget, frame.area());
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
                let init = crate::chatwidget::ChatWidgetInit {
                    config: self.config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: self.app_event_tx.clone(),
                    initial_prompt: None,
                    initial_images: Vec::new(),
                    enhanced_keys_supported: self.enhanced_keys_supported,
                    auth_manager: self.auth_manager.clone(),
                };
                self.chat_widget = ChatWidget::new(init, self.server.clone());
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
            AppEvent::SetCheckpointAutomation { enabled } => {
                let previous = self.auto_checkpoint_enabled;
                self.auto_checkpoint_enabled = enabled;
                self.chat_widget.set_auto_checkpoint_enabled(enabled);
                if enabled {
                    let message = if previous {
                        "Automatic checkpoints already enabled."
                    } else {
                        "Automatic checkpoints enabled."
                    };
                    let hint = if previous {
                        Some("Checkpoints continue to save after each Codex response using names like YYYY-MM-DD-abc123.".to_string())
                    } else {
                        Some("A checkpoint will be saved after each Codex response using names like YYYY-MM-DD-abc123.".to_string())
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
            AppEvent::StartFileSearch(query) => {
                if !query.is_empty() {
                    self.file_search.on_user_query(query);
                }
            }
            AppEvent::FileSearchResult { query, matches } => {
                self.chat_widget.apply_file_search_result(query, matches);
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
            AppEvent::OpenReasoningPopup { model, presets } => {
                self.chat_widget.open_reasoning_popup(model, presets);
            }
            AppEvent::PersistModelSelection { model, effort } => {
                let profile = self.active_profile.as_deref();
                match persist_model_selection(&self.config.codex_home, profile, &model, effort)
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
            AppEvent::PersistGlobalPrompt { prompt } => {
                match persist_global_prompt(&self.config.codex_home, prompt.as_deref()).await {
                    Ok(()) => {
                        let new_value = prompt.clone();
                        self.config.global_prompt = new_value.clone();
                        self.chat_widget.set_global_prompt(new_value.clone());

                        let (message, hint) = if let Some(text) = new_value.as_ref() {
                            let trimmed = text.trim();
                            let trimmed_len = trimmed.chars().count();
                            let mut preview: String = trimmed.chars().take(80).collect();
                            if trimmed_len > preview.chars().count() {
                                preview.push('…');
                            }
                            (
                                "Global prompt updated".to_string(),
                                Some(format!(
                                    "Will be prepended to your first message: {preview}"
                                )),
                            )
                        } else {
                            (
                                "Global prompt cleared".to_string(),
                                Some(
                                    "New sessions will start without prepending extra instructions."
                                        .to_string(),
                                ),
                            )
                        };

                        self.chat_widget.add_info_message(message, hint);
                    }
                    Err(err) => {
                        tracing::error!(error = %err, "failed to persist global prompt");
                        self.chat_widget
                            .add_error_message(format!("Failed to update global prompt: {err}"));
                    }
                }
            }
            AppEvent::PersistAlarmScript { script } => {
                match persist_alarm_script(&self.config.codex_home, script.as_deref()).await {
                    Ok(()) => {
                        self.config.alarm_script = script.clone();
                        self.config.notify = script
                            .as_ref()
                            .map(|value| Config::alarm_script_to_notify_command(value));
                        self.chat_widget.set_alarm_script(script.clone());

                        let (message, hint) = if let Some(value) = script.as_ref() {
                            let mut preview: String = value.chars().take(80).collect();
                            if value.chars().count() > preview.chars().count() {
                                preview.push('…');
                            }
                            (
                                "Alarm script updated".to_string(),
                                Some(format!("Will run via `sh -c`: {preview}")),
                            )
                        } else {
                            (
                                "Alarm script disabled".to_string(),
                                Some(
                                    "Codex will no longer run a script after each turn."
                                        .to_string(),
                                ),
                            )
                        };

                        self.chat_widget.add_info_message(message, hint);
                    }
                    Err(err) => {
                        tracing::error!(error = %err, "failed to persist alarm script");
                        self.chat_widget
                            .add_error_message(format!("Failed to update alarm script: {err}"));
                    }
                }
            }
            AppEvent::UpdateAskForApprovalPolicy(policy) => {
                self.chat_widget.set_approval_policy(policy);
            }
            AppEvent::UpdateSandboxPolicy(policy) => {
                self.chat_widget.set_sandbox_policy(policy);
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
        match self.create_checkpoint(None) {
            Ok((checkpoint_name, path)) => {
                self.chat_widget.add_info_message(
                    format!("Auto checkpoint '{checkpoint_name}' saved."),
                    Some(path.display().to_string()),
                );
            }
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to save auto checkpoint: {err:#}"));
            }
        }
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
                if let Ok(names) = self.list_checkpoint_names() {
                    if !names.is_empty() {
                        self.chat_widget.add_info_message(
                            "Available checkpoints:".to_string(),
                            Some(names.join(", ")),
                        );
                    }
                }
                return;
            }
            match fs::read_to_string(&path) {
                Ok(contents) => {
                    let composer_text =
                        format!("Continue from checkpoint `{sanitized}`.\n\n{}", contents);
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

    fn create_checkpoint(&self, name: Option<String>) -> Result<(String, PathBuf)> {
        let chosen_name = self.choose_checkpoint_name(name);
        let dir = self.checkpoint_dir();
        fs::create_dir_all(&dir)
            .wrap_err_with(|| format!("failed to create checkpoint directory {}", dir.display()))?;
        let file_path = dir.join(format!("{chosen_name}.md"));
        let contents = self.build_checkpoint_markdown(&chosen_name);
        fs::write(&file_path, contents)
            .wrap_err_with(|| format!("failed to write checkpoint {}", file_path.display()))?;
        Ok((chosen_name, file_path))
    }

    fn checkpoint_dir(&self) -> PathBuf {
        self.config.cwd.join(".codex").join("checkpoints")
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
        let _ = writeln!(out, "# Checkpoint {}", name);
        out.push('\n');
        let _ = writeln!(out, "- Saved at: {}", timestamp);
        let _ = writeln!(out, "- Working directory: `{}`", self.config.cwd.display());
        if let Some(conversation_id) = self.chat_widget.conversation_id() {
            let _ = writeln!(out, "- Conversation ID: `{}`", conversation_id);
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
                    let _ = writeln!(out, "> {}", expl);
                    out.push('\n');
                }
                if update.plan.is_empty() {
                    out.push_str("_No steps provided._\n\n");
                } else {
                    for step in &update.plan {
                        let marker = Self::step_status_marker(&step.status);
                        let trimmed_step = step.step.trim();
                        let _ = writeln!(out, "- [{}] {}", marker, trimmed_step);
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
            {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
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
                    let last = responses.last_mut().unwrap();
                    if !last.ends_with('\n') {
                        last.push('\n');
                    }
                    last.push_str(&cleaned);
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
            // with an empty composer. In any other state, forward Esc so the
            // active UI (e.g. status indicator, modals, popups) handles it.
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
}

struct CheckpointContext {
    user_prompts: Vec<String>,
    agent_responses: Vec<String>,
    plan_updates: Vec<UpdatePlanArgs>,
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
            auto_checkpoint_enabled: false,
            backtrack: BacktrackState::default(),
        }
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
    fn random_checkpoint_name_uses_date_prefix() {
        let name = App::generate_random_checkpoint_name();
        let today_prefix = Utc::now().format("%Y-%m-%d").to_string();
        assert!(
            name.starts_with(&format!("{today_prefix}-")),
            "expected checkpoint name '{name}' to begin with '{today_prefix}-'"
        );
        let suffix = &name[today_prefix.len() + 1..];
        assert_eq!(
            suffix.len(),
            6,
            "expected hash suffix of length 6 but found '{}'",
            suffix.len()
        );
        assert!(
            suffix
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit()),
            "hash suffix should be base36 but was '{suffix}'"
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
}
