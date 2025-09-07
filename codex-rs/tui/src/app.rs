use crate::app_backtrack::BacktrackState;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::chatwidget::ChatWidget;
use crate::file_search::FileSearchManager;
use crate::pager_overlay::Overlay;
use crate::resume_picker::ResumeSelection;
use crate::tui;
use crate::tui::TuiEvent;
use codex_ansi_escape::ansi_escape_line;
use codex_core::AuthManager;
use codex_core::ConversationManager;
use codex_core::ConversationsPage;
use codex_core::Cursor;
use codex_core::RolloutRecorder;
use codex_core::config::Config;
use codex_core::protocol::TokenUsage;
use codex_protocol::config_types::ReasoningSummary;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::terminal::supports_keyboard_enhancement;
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
// use uuid::Uuid;

pub(crate) struct App {
    pub(crate) server: Arc<ConversationManager>,
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) chat_widget: ChatWidget,

    /// Config is stored here so we can recreate ChatWidgets as needed.
    pub(crate) config: Config,

    pub(crate) file_search: FileSearchManager,

    pub(crate) transcript_lines: Vec<Line<'static>>,

    // Pager overlay state (Transcript or Static like Diff)
    pub(crate) overlay: Option<Overlay>,
    pub(crate) deferred_history_lines: Vec<Line<'static>>,
    has_emitted_history_lines: bool,

    pub(crate) enhanced_keys_supported: bool,

    /// Controls the animation thread that sends CommitTick events.
    pub(crate) commit_anim_running: Arc<AtomicBool>,

    // Esc-backtracking state grouped
    pub(crate) backtrack: crate::app_backtrack::BacktrackState,

    // Post-turn judge options
    pub(crate) turn_judge_enabled: bool,
    pub(crate) turn_judge_prompt: Option<String>,
    pub(crate) autopilot_enabled: bool,
    pub(crate) yes_man_enabled: bool,
    pub(crate) reviewer_enabled: bool,
    pub(crate) reviewer_model: String,
    pub(crate) reviewer_effort: codex_core::protocol_config_types::ReasoningEffort,
    pub(crate) last_prd_len: Option<u64>,
    pub(crate) last_prd_mtime: Option<std::time::SystemTime>,
}

impl App {
    async fn find_last_session_by_meta<F>(&self, predicate: F) -> Result<Option<std::path::PathBuf>>
    where
        F: Fn(&serde_json::Value) -> bool,
    {
        let mut anchor: Option<Cursor> = None;
        for _ in 0..5 {
            let page: ConversationsPage =
                RolloutRecorder::list_conversations(&self.config.codex_home, 50, anchor.as_ref())
                    .await?;
            for item in page.items.iter() {
                if let Some(first) = item.head.first() {
                    let same_cwd = first
                        .get("cwd")
                        .and_then(|v| v.as_str())
                        .map(|s| std::path::Path::new(s) == self.config.cwd)
                        .unwrap_or(false);
                    if !same_cwd {
                        continue;
                    }
                    if predicate(first) {
                        return Ok(Some(item.path.clone()));
                    }
                }
            }
            if page.next_cursor.is_none() {
                break;
            }
            anchor = page.next_cursor;
        }
        Ok(None)
    }
    // TODO(codex): Reduce parameter count by grouping App::run params into a single
    // AppRunParams config struct and pass that instead of many args. This `allow`
    // is temporary to unblock clippy in CI.
    // Tracking: https://github.com/codex-team/codex/issues/0000
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        tui: &mut tui::Tui,
        auth_manager: Arc<AuthManager>,
        config: Config,
        initial_prompt: Option<String>,
        initial_images: Vec<PathBuf>,
        resume_selection: ResumeSelection,
        turn_judge_enabled: bool,
        turn_judge_prompt: Option<String>,
    ) -> Result<TokenUsage> {
        use tokio_stream::StreamExt;
        let (app_event_tx, mut app_event_rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(app_event_tx);

        let conversation_manager = Arc::new(ConversationManager::new(auth_manager.clone()));

        let enhanced_keys_supported = supports_keyboard_enhancement().unwrap_or(false);

        let chat_widget = match resume_selection {
            ResumeSelection::StartFresh | ResumeSelection::Exit => {
                let init = crate::chatwidget::ChatWidgetInit {
                    config: config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: app_event_tx.clone(),
                    initial_prompt: initial_prompt.clone(),
                    initial_images: initial_images.clone(),
                    enhanced_keys_supported,
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
            auth_manager,
            app_event_tx,
            chat_widget,
            config,
            file_search,
            enhanced_keys_supported,
            transcript_lines: Vec::new(),
            overlay: None,
            deferred_history_lines: Vec::new(),
            has_emitted_history_lines: false,
            commit_anim_running: Arc::new(AtomicBool::new(false)),
            backtrack: BacktrackState::default(),
            turn_judge_enabled,
            turn_judge_prompt,
            autopilot_enabled: turn_judge_enabled,
            yes_man_enabled: false,
            reviewer_enabled: false,
            reviewer_model: "gpt-5".to_string(),
            reviewer_effort: codex_core::protocol_config_types::ReasoningEffort::Minimal,
            last_prd_len: None,
            last_prd_mtime: None,
        };

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
        Ok(app.token_usage())
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
                };
                self.chat_widget = ChatWidget::new(init, self.server.clone());
                tui.frame_requester().schedule_frame();
            }
            AppEvent::InsertHistoryCell(cell) => {
                let mut cell_transcript = cell.transcript_lines();
                if !cell.is_stream_continuation() && !self.transcript_lines.is_empty() {
                    cell_transcript.insert(0, Line::from(""));
                }
                if let Some(Overlay::Transcript(t)) = &mut self.overlay {
                    t.insert_lines(cell_transcript.clone());
                    tui.frame_requester().schedule_frame();
                }
                self.transcript_lines.extend(cell_transcript.clone());
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
                // Intercept TaskComplete to trigger a post-turn evaluation or Yes‑Man autopilot.
                let msg = &event.msg;
                if let codex_core::protocol::EventMsg::TaskComplete(tc) = msg {
                    if self.yes_man_enabled {
                        let server = self.server.clone();
                        let main_session_id = self.chat_widget.session_id();
                        tokio::spawn(async move {
                            if let Some(sess_id) = main_session_id
                                && let Ok(conv) = server.get_conversation(sess_id).await
                            {
                                let _ = conv
                                    .submit(codex_core::protocol::Op::UserInput {
                                        items: vec![codex_core::protocol::InputItem::Text {
                                            text: "Siga o plano e prossiga para a próxima etapa."
                                                .to_string(),
                                        }],
                                    })
                                    .await;
                            }
                        });
                    } else if self.reviewer_enabled {
                        // Build reviewer context and spawn reviewer session
                        let prd_path = self.config.cwd.join("PRD.md");
                        let prd_budget = 64 * 1024usize;
                        // Decide how to include PRD: full on first change, tasks-only otherwise.
                        let (prd_mode, prd_meta_str) = match std::fs::metadata(&prd_path) {
                            Ok(md) => {
                                let len = md.len();
                                let mtime = md.modified().ok();
                                let unchanged = self.last_prd_len == Some(len)
                                    && self.last_prd_mtime == mtime;
                                // Update cached meta for next turn
                                self.last_prd_len = Some(len);
                                self.last_prd_mtime = mtime;
                                let mode = if unchanged {
                                    crate::reviewer::PrdMode::TasksOnly
                                } else {
                                    crate::reviewer::PrdMode::Full
                                };
                                let meta = match mtime
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                {
                                    Some(d) => format!("len={len}, mtime={}s", d.as_secs()),
                                    None => format!("len={len}"),
                                };
                                (mode, Some(meta))
                            }
                            Err(_) => (crate::reviewer::PrdMode::Omit, None),
                        };
                        let user_prompt =
                            self.chat_widget.last_user_prompt().map(|s| s.to_string());
                        let assistant_output = tc.last_agent_message.clone();
                        let plan_text = self.chat_widget.last_plan_text();
                        let diff_text = self.chat_widget.last_unified_diff().map(|s| s.to_string());
                        let bundle = crate::reviewer::build_reviewer_bundle(
                            &prd_path,
                            prd_budget,
                            prd_mode,
                            prd_meta_str.as_deref(),
                            user_prompt.as_deref(),
                            assistant_output.as_deref(),
                            plan_text.as_deref(),
                            diff_text.as_deref(),
                            &self.reviewer_model,
                            &format!("{}", self.reviewer_effort),
                            &self.config.cwd,
                            env!("CARGO_PKG_VERSION"),
                        );
                        let tx = self.app_event_tx.clone();
                        let server = self.server.clone();
                        let mut cfg = self.config.clone();
                        cfg.model = self.reviewer_model.clone();
                        cfg.model_reasoning_effort = self.reviewer_effort;
                        cfg.model_reasoning_summary = ReasoningSummary::Detailed;
                        let main_session_id = self.chat_widget.session_id();
                        let autopilot = self.autopilot_enabled;
                        tokio::spawn(async move {
                            crate::reviewer::run_reviewer_session(
                                server,
                                &cfg,
                                bundle,
                                tx,
                                main_session_id,
                                autopilot,
                            )
                            .await;
                        });
                    } else if self.turn_judge_enabled {
                        let last = tc.last_agent_message.clone();
                        let prompt = self
                            .turn_judge_prompt
                            .clone()
                            .unwrap_or_else(default_judge_prompt);
                        let tx = self.app_event_tx.clone();
                        let server = self.server.clone();
                        let cfg = self.config.clone();
                        let main_session_id = self.chat_widget.session_id();
                        let autopilot = self.autopilot_enabled;
                        tokio::spawn(async move {
                            crate::post_turn_eval::run_post_turn_evaluation(
                                server,
                                &cfg,
                                &prompt,
                                last,
                                tx,
                                main_session_id,
                                autopilot,
                            )
                            .await;
                        });
                    }
                }
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
                self.overlay = Some(Overlay::new_static_with_title(
                    pager_lines,
                    "D I F F".to_string(),
                ));
                tui.frame_requester().schedule_frame();
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
                self.chat_widget.set_reasoning_effort(effort);
            }
            AppEvent::UpdateModel(model) => {
                self.chat_widget.set_model(model);
            }
            AppEvent::UpdateAskForApprovalPolicy(policy) => {
                self.chat_widget.set_approval_policy(policy);
            }
            AppEvent::UpdateSandboxPolicy(policy) => {
                self.chat_widget.set_sandbox_policy(policy);
            }
            AppEvent::OpenResumePicker => {
                // Ensure any active bottom-pane modal is dismissed before entering alt-screen
                self.chat_widget.dismiss_active_view();
                match crate::resume_picker::run_resume_picker(
                    tui,
                    &self.config.codex_home,
                    &self.config.cwd,
                )
                .await
                {
                    Ok(crate::resume_picker::ResumeSelection::Resume(path)) => {
                        let resumed = self
                            .server
                            .resume_conversation_from_rollout(
                                self.config.clone(),
                                path.clone(),
                                self.auth_manager.clone(),
                            )
                            .await
                            .wrap_err_with(|| {
                                format!("Failed to resume session from {}", path.display())
                            })?;
                        let init = crate::chatwidget::ChatWidgetInit {
                            config: self.config.clone(),
                            frame_requester: tui.frame_requester(),
                            app_event_tx: self.app_event_tx.clone(),
                            initial_prompt: None,
                            initial_images: Vec::new(),
                            enhanced_keys_supported: self.enhanced_keys_supported,
                        };
                        self.chat_widget = ChatWidget::new_from_existing(
                            init,
                            resumed.conversation,
                            resumed.session_configured,
                        );
                        // Make sure the bottom pane is back to normal state after resume.
                        self.chat_widget.dismiss_active_view();
                        tui.frame_requester().schedule_frame();
                    }
                    Ok(crate::resume_picker::ResumeSelection::StartFresh) => {
                        // Start a fresh session and insert an info note in history.
                        self.app_event_tx.send(AppEvent::NewSession);
                        use ratatui::style::Stylize as _;
                        use ratatui::text::Line as RtLine;
                        // Build a small info cell and insert after the new session is created.
                        let note: Vec<RtLine<'static>> = vec![RtLine::from(
                            "No previous sessions found — starting a new one"
                                .italic()
                                .dim(),
                        )];
                        let cell = crate::history_cell::new_info_note(note);
                        self.app_event_tx
                            .send(AppEvent::InsertHistoryCell(Box::new(cell)));
                    }
                    Ok(crate::resume_picker::ResumeSelection::Exit) => {
                        return Ok(false);
                    }
                    Err(e) => return Err(e),
                }
            }
            AppEvent::OpenLastJudgeSession => {
                let found = self
                    .find_last_session_by_meta(|meta| {
                        let model = meta.get("model").and_then(|v| v.as_str()).unwrap_or("");
                        let effort = meta
                            .get("reasoning_effort")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        model == "gpt-5" && effort.eq_ignore_ascii_case("minimal")
                    })
                    .await?;
                if let Some(path) = found {
                    self.app_event_tx.send(AppEvent::ResumeFromPath(path));
                } else {
                    self.app_event_tx.send(AppEvent::OpenResumePicker);
                }
            }
            AppEvent::OpenLastReviewerSession => {
                let found = self
                    .find_last_session_by_meta(|meta| {
                        if let Some(role) = meta.get("role").and_then(|v| v.as_str())
                            && role == "reviewer"
                        {
                            return true;
                        }
                        // Fallback V1 heuristic
                        let model = meta.get("model").and_then(|v| v.as_str()).unwrap_or("");
                        let effort = meta
                            .get("reasoning_effort")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        model == self.reviewer_model && effort.eq_ignore_ascii_case("minimal")
                    })
                    .await?;
                if let Some(path) = found {
                    self.app_event_tx.send(AppEvent::ResumeFromPath(path));
                } else {
                    self.app_event_tx.send(AppEvent::OpenResumePicker);
                }
            }
            AppEvent::ResumeFromPath(path) => {
                // Dismiss any lingering modal before switching session
                self.chat_widget.dismiss_active_view();
                let resumed = self
                    .server
                    .resume_conversation_from_rollout(
                        self.config.clone(),
                        path.clone(),
                        self.auth_manager.clone(),
                    )
                    .await
                    .wrap_err_with(|| {
                        format!("Failed to resume session from {}", path.display())
                    })?;
                let init = crate::chatwidget::ChatWidgetInit {
                    config: self.config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: self.app_event_tx.clone(),
                    initial_prompt: None,
                    initial_images: Vec::new(),
                    enhanced_keys_supported: self.enhanced_keys_supported,
                };
                self.chat_widget = ChatWidget::new_from_existing(
                    init,
                    resumed.conversation,
                    resumed.session_configured,
                );
                self.chat_widget.dismiss_active_view();
                tui.frame_requester().schedule_frame();
            }
            AppEvent::UpdateTurnJudgeEnabled(on) => {
                self.turn_judge_enabled = on;
                self.chat_widget.set_judge_enabled(on);
            }
            AppEvent::UpdateAutopilotEnabled(on) => {
                self.autopilot_enabled = on;
                self.chat_widget.set_autopilot_enabled(on);
            }
            AppEvent::UpdateTurnJudgePrompt(p) => {
                self.turn_judge_prompt = p;
            }
            AppEvent::UpdateYesManEnabled(on) => {
                self.yes_man_enabled = on;
                if on {
                    self.turn_judge_enabled = false;
                    self.autopilot_enabled = true;
                }
                self.chat_widget.set_yes_man_enabled(on);
                // Ensure chat footer reflects combined state flags
                self.chat_widget.set_judge_enabled(self.turn_judge_enabled);
                self.chat_widget
                    .set_autopilot_enabled(self.autopilot_enabled);
            }
            AppEvent::UpdateReviewerEnabled(on) => {
                self.reviewer_enabled = on;
                if on {
                    self.turn_judge_enabled = false;
                }
                self.chat_widget.set_reviewer_enabled(on);
                self.chat_widget.set_judge_enabled(self.turn_judge_enabled);
            }
            AppEvent::UpdateReviewerModel(m) => {
                self.reviewer_model = m;
                // Inform footer: reviewer stays enabled status unchanged.
                self.chat_widget.set_reviewer_enabled(self.reviewer_enabled);
            }
            AppEvent::UpdateReviewerEffort(eff) => {
                self.reviewer_effort = eff;
            }
            AppEvent::OpenAutopilotPopup => {
                self.chat_widget.open_autopilot_popup();
            }
            AppEvent::UpdatePatchGateEnabled(on) => {
                // Record in shared prefs and update UI flags
                crate::autopilot_prefs::set_patchgate_enabled(on);
                self.chat_widget.set_patchgate_enabled(on);
                // Brief log entry for visibility
                use ratatui::style::Stylize as _;
                let lines: Vec<ratatui::text::Line<'static>> = vec![
                    "⚙\u{200A}".into(),
                    if on {
                        "PatchGate enabled (autopilot)".bold().into()
                    } else {
                        "PatchGate disabled (autopilot)".dim().into()
                    },
                ];
                self.app_event_tx
                    .send(AppEvent::InsertHistoryCell(Box::new(crate::history_cell::new_info_note(lines))));
            }
            AppEvent::UpdatePatchGatePermissive(on) => {
                crate::autopilot_prefs::set_patchgate_permissive(on);
                self.chat_widget.set_patchgate_permissive(on);
                // Record-only informational note
                use ratatui::style::Stylize as _;
                let text = if on {
                    "PatchGate mode set: permissive"
                } else {
                    "PatchGate mode set: strict"
                };
                let lines: Vec<ratatui::text::Line<'static>> =
                    vec!["⚙\u{200A}".into(), text.dim().into()];
                self.app_event_tx
                    .send(AppEvent::InsertHistoryCell(Box::new(crate::history_cell::new_info_note(lines))));
            }
            AppEvent::OpenReviewerModelPopup => {
                self.chat_widget.open_reviewer_model_popup();
            }
            AppEvent::ShowTextOverlay { title, text } => {
                let _ = tui.enter_alt_screen();
                let lines: Vec<ratatui::text::Line<'static>> = if text.trim().is_empty() {
                    vec!["(empty)".italic().into()]
                } else {
                    text.lines().map(|l| l.to_string().into()).collect()
                };
                self.overlay = Some(Overlay::new_static_with_title(lines, title));
                tui.frame_requester().schedule_frame();
            }
        }
        Ok(true)
    }

    pub(crate) fn token_usage(&self) -> codex_core::protocol::TokenUsage {
        self.chat_widget.token_usage().clone()
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
                self.overlay = Some(Overlay::new_transcript(self.transcript_lines.clone()));
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
                && self.backtrack.count > 0
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

fn default_judge_prompt() -> String {
    // Keep this short and deterministic. Ask for JSON.
    "Você é um avaliador. Analise a resposta do assistente e responda apenas em JSON da forma {\"follow_plan\": true|false, \"reason\": \"breve justificativa\"}. Diga true somente se a resposta indicar com clareza que é seguro e apropriado seguir o plano atual sem ajustes.".to_string()
}
