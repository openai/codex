use crate::app_backtrack::BacktrackState;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::chatwidget::ChatWidget;
use crate::chatwidget::ThreadPickerEntry;
use crate::file_search::FileSearchManager;
use crate::history_cell::HistoryCell;
use crate::pager_overlay::Overlay;
use crate::resume_picker::ResumeSelection;
use crate::session_id::SessionId;
use crate::session_manager::AutoNameKind;
use crate::session_manager::SessionManager;
use crate::session_manager::ThreadOrigin;
use crate::text_formatting::concise_request_summary;
use crate::tui;
use crate::tui::TuiEvent;
use codex_ansi_escape::ansi_escape_line;
use codex_core::AuthManager;
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::config::persist_model_selection;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::BackgroundEventEvent;
use codex_core::protocol::ConversationPathResponseEvent;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::TaskCompleteEvent;
use codex_core::protocol::TokenUsage;
use codex_core::protocol_config_types::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::mcp_protocol::ConversationId;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
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

#[derive(Debug, Clone)]
pub struct AppExitInfo {
    pub token_usage: TokenUsage,
    pub conversation_id: Option<ConversationId>,
}

pub(crate) struct App {
    pub(crate) server: Arc<ConversationManager>,
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) chat_widget: SessionManager,
    pub(crate) auth_manager: Arc<AuthManager>,

    /// Config is stored here so we can recreate ChatWidgets as needed.
    pub(crate) config: Config,
    pub(crate) active_profile: Option<String>,

    pub(crate) file_search: FileSearchManager,

    // Pager overlay state (Transcript or Static like Diff)
    pub(crate) overlay: Option<Overlay>,
    pub(crate) deferred_history_lines: Vec<Line<'static>>,
    has_emitted_history_lines: bool,

    pub(crate) enhanced_keys_supported: bool,

    /// Controls the animation thread that sends CommitTick events.
    pub(crate) commit_anim_running: Arc<AtomicBool>,

    // Esc-backtracking state grouped
    pub(crate) backtrack: crate::app_backtrack::BacktrackState,

    thread_fork_pending: Option<ThreadForkPending>,
    next_session_id: u64,
}

struct ThreadForkPending {
    base_id: ConversationId,
    title: String,
}

impl App {
    pub(super) fn alloc_session_id(&mut self) -> SessionId {
        let id = SessionId::new(self.next_session_id);
        self.next_session_id = self.next_session_id.saturating_add(1);
        id
    }

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

        let conversation_manager = Arc::new(ConversationManager::new(auth_manager.clone()));

        let enhanced_keys_supported = tui.enhanced_keys_supported();

        let next_session_id_raw: u64 = 1;
        let main_session_id = SessionId::new(0);

        let chat_widget = match resume_selection {
            ResumeSelection::StartFresh | ResumeSelection::Exit => {
                let init = crate::chatwidget::ChatWidgetInit {
                    config: config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: app_event_tx.clone(),
                    initial_prompt: initial_prompt.clone(),
                    initial_images: initial_images.clone(),
                    enhanced_keys_supported,
                    auth_manager: auth_manager.clone(),
                    session_id: main_session_id,
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
                    session_id: main_session_id,
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
            chat_widget: SessionManager::single(chat_widget, main_session_id),
            auth_manager: auth_manager.clone(),
            config,
            active_profile,
            file_search,
            enhanced_keys_supported,
            overlay: None,
            deferred_history_lines: Vec::new(),
            has_emitted_history_lines: false,
            commit_anim_running: Arc::new(AtomicBool::new(false)),
            backtrack: BacktrackState::default(),
            thread_fork_pending: None,
            next_session_id: next_session_id_raw,
        };

        app.update_active_thread_header();

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
                            frame.render_widget_ref(&*self.chat_widget, frame.area());
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
                let session_id = self.alloc_session_id();
                let init = crate::chatwidget::ChatWidgetInit {
                    config: self.config.clone(),
                    frame_requester: tui.frame_requester(),
                    app_event_tx: self.app_event_tx.clone(),
                    initial_prompt: None,
                    initial_images: Vec::new(),
                    enhanced_keys_supported: self.enhanced_keys_supported,
                    auth_manager: self.auth_manager.clone(),
                    session_id,
                };
                self.chat_widget =
                    SessionManager::single(ChatWidget::new(init, self.server.clone()), session_id);
                tui.frame_requester().schedule_frame();
            }
            AppEvent::InsertHistoryCell {
                session_id,
                conversation_id,
                cell,
            } => {
                let cell: Arc<dyn HistoryCell> = cell.into();
                let target_idx = self
                    .chat_widget
                    .index_for_session_id(session_id)
                    .or_else(|| {
                        conversation_id
                            .as_ref()
                            .and_then(|id| self.chat_widget.index_for_conversation_id(id))
                    })
                    .unwrap_or_else(|| self.chat_widget.active_index());
                let is_active = target_idx == self.chat_widget.active_index();

                if let Some(handle) = self.chat_widget.session_mut(target_idx) {
                    handle.transcript_cells.push(cell.clone());
                }

                if !is_active {
                    return Ok(true);
                }

                if let Some(Overlay::Transcript(t)) = &mut self.overlay {
                    t.insert_cell(cell.clone());
                    tui.frame_requester().schedule_frame();
                }
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
            AppEvent::StartThread => {
                self.on_start_thread_request();
            }
            AppEvent::NewBlankThread => {
                self.on_new_blank_thread(tui);
            }
            AppEvent::ClearActiveThread => {
                self.clear_active_thread(tui);
            }
            AppEvent::PromptCloseActiveThread => {
                self.prompt_close_active_thread();
            }
            AppEvent::SuggestThreadName { session_id, text } => {
                self.on_suggest_thread_name(tui, session_id, text);
            }
            AppEvent::ToggleThreadPicker => {
                self.toggle_thread_picker();
            }
            AppEvent::SwitchThread(idx) => {
                self.switch_thread(tui, idx, true);
            }
            AppEvent::QuitRequested => {
                self.on_quit_requested();
            }
            AppEvent::CloseThread { index, summarize } => {
                self.close_thread(tui, index, summarize);
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
            AppEvent::CodexEvent {
                session_id,
                conversation_id,
                event,
            } => {
                let msg = event.msg.clone();
                let target_idx = self
                    .chat_widget
                    .index_for_session_id(session_id)
                    .or_else(|| self.chat_widget.index_for_conversation_id(&conversation_id))
                    .unwrap_or_else(|| self.chat_widget.active_index());
                let is_active = target_idx == self.chat_widget.active_index();
                let increment_unread = !is_active && Self::event_marks_unread(&msg);

                if is_active {
                    self.chat_widget.handle_codex_event(event);
                } else if let Some(handle) = self.chat_widget.session_mut(target_idx) {
                    handle.widget.handle_codex_event(event);
                } else {
                    self.chat_widget.handle_codex_event(event);
                }

                if let Some(handle) = self.chat_widget.session_mut(target_idx) {
                    if increment_unread {
                        handle.unread_count = handle.unread_count.saturating_add(1);
                        tui.frame_requester().schedule_frame();
                    }

                    match msg {
                        EventMsg::SessionConfigured(configured) => {
                            handle.conversation_id = Some(configured.session_id);
                        }
                        _ => {
                            handle.conversation_id = handle.widget.conversation_id();
                        }
                    }
                }
            }
            AppEvent::ConversationHistory(ev) => {
                if !self.on_conversation_history_for_thread(tui, &ev).await? {
                    self.on_conversation_history_for_backtrack(tui, ev).await?;
                }
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
                self.on_update_reasoning_effort(effort);
            }
            AppEvent::UpdateModel(model) => {
                self.chat_widget.set_model(&model);
                self.config.model = model.clone();
                if let Some(family) = find_family_for_model(&model) {
                    self.config.model_family = family;
                }
            }
            AppEvent::PersistModelSelection { model, effort } => {
                let profile = self.active_profile.as_deref();
                match persist_model_selection(&self.config.codex_home, profile, &model, effort)
                    .await
                {
                    Ok(()) => {
                        if let Some(profile) = profile {
                            self.chat_widget.add_info_message(
                                format!("Model changed to {model} for {profile} profile"),
                                None,
                            );
                        } else {
                            self.chat_widget
                                .add_info_message(format!("Model changed to {model}"), None);
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

    fn on_start_thread_request(&mut self) {
        let Some(base_id) = self.chat_widget.conversation_id() else {
            self.chat_widget.add_error_message(
                "Cannot create a thread before the session is configured.".to_string(),
            );
            return;
        };

        if self.thread_fork_pending.is_some() {
            self.chat_widget
                .add_info_message("Thread creation already in progress.".to_string(), None);
            return;
        }

        let title = self.next_child_thread_title();
        self.thread_fork_pending = Some(ThreadForkPending {
            base_id,
            title: title.clone(),
        });
        self.chat_widget
            .add_info_message(format!("Creating {title}..."), None);
        self.app_event_tx.send(AppEvent::CodexOp(Op::GetPath));
    }

    fn on_new_blank_thread(&mut self, tui: &mut tui::Tui) {
        let title = self.next_top_level_thread_title();

        let session_id = self.alloc_session_id();
        let init = crate::chatwidget::ChatWidgetInit {
            config: self.config.clone(),
            frame_requester: tui.frame_requester(),
            app_event_tx: self.app_event_tx.clone(),
            initial_prompt: None,
            initial_images: Vec::new(),
            enhanced_keys_supported: self.enhanced_keys_supported,
            auth_manager: self.auth_manager.clone(),
            session_id,
        };

        let widget = ChatWidget::new(init, self.server.clone());
        let idx = self.chat_widget.add_session(widget, title, session_id);
        if let Some(handle) = self.chat_widget.session_mut(idx) {
            handle.auto_name_kind = Some(AutoNameKind::TopLevel);
            handle.display_title = None;
        }
        self.chat_widget.refresh_thread_paths();

        self.chat_widget.switch_active(idx);
        if let Some(handle) = self.chat_widget.session_mut(idx) {
            handle.widget.clear_request_summary();
        }
        self.apply_display_labels_for(idx);

        self.has_emitted_history_lines = false;
        self.deferred_history_lines.clear();
        self.overlay = None;
        self.render_transcript_once(tui);
        self.chat_widget.set_composer_text(String::new());
        let path_display = self.chat_widget.display_thread_path_string_active();
        self.chat_widget
            .add_info_message(format!("Started {path_display}"), None);
        tui.frame_requester().schedule_frame();
    }

    fn clear_active_thread(&mut self, tui: &mut tui::Tui) {
        let idx = self.chat_widget.active_index();
        let (_title, origin, session_id) = match self.chat_widget.sessions().get(idx) {
            Some(session) => (
                session.title.clone(),
                session.origin.clone(),
                session.session_id,
            ),
            None => {
                self.chat_widget
                    .add_error_message("Active thread not found.".to_string());
                return;
            }
        };
        let init = crate::chatwidget::ChatWidgetInit {
            config: self.config.clone(),
            frame_requester: tui.frame_requester(),
            app_event_tx: self.app_event_tx.clone(),
            initial_prompt: None,
            initial_images: Vec::new(),
            enhanced_keys_supported: self.enhanced_keys_supported,
            auth_manager: self.auth_manager.clone(),
            session_id,
        };

        let new_widget = ChatWidget::new(init, self.server.clone());

        if let Some(handle) = self.chat_widget.session_mut(idx) {
            handle.widget = new_widget;
            handle.transcript_cells.clear();
            handle.conversation_id = None;
            handle.unread_count = 0;
            handle.origin = origin;
            handle.auto_name_kind = if handle.origin.is_some() {
                Some(AutoNameKind::Child)
            } else {
                Some(AutoNameKind::TopLevel)
            };
            handle.display_title = None;
        } else {
            self.chat_widget
                .add_error_message("Active thread not found.".to_string());
            return;
        }

        self.chat_widget.refresh_thread_paths();
        self.chat_widget.switch_active(idx);
        if let Some(handle) = self.chat_widget.session_mut(idx) {
            handle.widget.clear_request_summary();
        }
        let display_label = self.chat_widget.display_label_for_index(idx);
        let display_path = self.chat_widget.display_thread_path_of(idx);
        self.chat_widget.set_active_title(display_label);
        self.chat_widget.set_active_thread_path(display_path);

        self.has_emitted_history_lines = false;
        self.deferred_history_lines.clear();
        self.overlay = None;
        self.render_transcript_once(tui);
        self.chat_widget.set_composer_text(String::new());
        let label = self.chat_widget.display_label_for_index(idx);
        let summary = self
            .chat_widget
            .display_summary_for_index(idx)
            .map(str::to_string);
        if let Some(summary) = summary {
            self.chat_widget
                .add_info_message(format!("Cleared {label} — {summary}"), None);
        } else {
            self.chat_widget
                .add_info_message(format!("Cleared {label}"), None);
        }
        tui.frame_requester().schedule_frame();
    }

    fn toggle_thread_picker(&mut self) {
        if self.chat_widget.len() <= 1 {
            self.chat_widget
                .add_info_message("No other threads yet.".to_string(), None);
            return;
        }

        let active_idx = self.chat_widget.active_index();
        let entries: Vec<ThreadPickerEntry> = self
            .chat_widget
            .sessions()
            .iter()
            .enumerate()
            .map(|(idx, session)| ThreadPickerEntry {
                index: idx,
                title: self.chat_widget.display_label_for_index(idx),
                conversation_id: session.conversation_id,
                is_active: idx == active_idx,
                unread_count: session.unread_count,
                path: self.chat_widget.display_thread_path_of(idx),
            })
            .collect();

        self.chat_widget.open_thread_picker(entries);
    }

    fn prompt_close_active_thread(&mut self) {
        if self.chat_widget.len() <= 1 {
            self.chat_widget.add_error_message(
                "Cannot close the only active conversation. Use /clear or /quit instead."
                    .to_string(),
            );
            return;
        }

        let idx = self.chat_widget.active_index();
        let (title, origin) = match self.chat_widget.sessions().get(idx) {
            Some(session) => (session.title.clone(), session.origin.clone()),
            None => {
                self.chat_widget
                    .add_error_message("Active thread not found.".to_string());
                return;
            }
        };

        let Some(origin) = origin else {
            self.chat_widget.add_error_message(
                "Active thread does not have a parent conversation to return to.".to_string(),
            );
            return;
        };

        let parent_label = self.parent_label_for_origin(&origin);
        self.chat_widget
            .open_quit_thread_prompt(idx, title, parent_label);
    }

    fn switch_thread(&mut self, tui: &mut tui::Tui, idx: usize, announce: bool) {
        if idx == self.chat_widget.active_index() {
            return;
        }
        if !self.chat_widget.switch_active(idx) {
            self.chat_widget
                .add_error_message(format!("Thread {idx} not found."));
            return;
        }

        self.has_emitted_history_lines = false;
        self.deferred_history_lines.clear();
        self.overlay = None;
        self.render_transcript_once(tui);
        self.apply_display_labels_for(idx);
        if announce {
            let label = self.chat_widget.display_label_for_index(idx);
            let summary = self
                .chat_widget
                .display_summary_for_index(idx)
                .map(str::to_string);
            if let Some(summary) = summary {
                self.chat_widget
                    .add_info_message(format!("Switched to {label} — {summary}"), None);
            } else {
                self.chat_widget
                    .add_info_message(format!("Switched to {label}"), None);
            }
        }
        tui.frame_requester().schedule_frame();
    }

    fn switch_thread_relative(&mut self, tui: &mut tui::Tui, delta: isize) {
        let len = self.chat_widget.len();
        if len <= 1 {
            self.chat_widget
                .add_info_message("No other threads yet.".to_string(), None);
            return;
        }

        let current = self.chat_widget.active_index();
        let len_isize = len as isize;
        let next = ((current as isize + delta).rem_euclid(len_isize)) as usize;
        if next == current {
            return;
        }

        self.switch_thread(tui, next, false);
    }

    fn close_thread(&mut self, tui: &mut tui::Tui, idx: usize, summarize: bool) {
        if self.chat_widget.len() <= 1 {
            self.chat_widget
                .add_error_message("Cannot close the only active conversation.".to_string());
            return;
        }

        let child_display = self.chat_widget.display_label_for_index(idx);
        let (_child_id, origin, new_entries) = {
            let Some(child_session) = self.chat_widget.sessions().get(idx) else {
                self.chat_widget
                    .add_error_message(format!("Thread {idx} not found."));
                return;
            };
            let Some(origin) = child_session.origin.clone() else {
                self.chat_widget.add_error_message(
                    "This thread does not have a parent conversation to return to.".to_string(),
                );
                return;
            };
            let new_entries = child_session
                .transcript_cells
                .len()
                .saturating_sub(origin.parent_snapshot_len);
            (child_session.title.clone(), origin, new_entries)
        };

        if !self.chat_widget.remove_session(idx) {
            self.chat_widget
                .add_error_message("Failed to close thread.".to_string());
            return;
        }

        let parent_idx = self.resolve_parent_index(&origin);
        if self.chat_widget.len() == 0 {
            self.app_event_tx.send(AppEvent::ExitRequest);
            return;
        }
        let parent_idx = parent_idx.min(self.chat_widget.len().saturating_sub(1));
        self.switch_thread(tui, parent_idx, false);

        if summarize {
            let summary_text = self.build_summary_text(&child_display, new_entries);
            self.chat_widget.set_composer_text(summary_text);
            self.chat_widget.add_info_message(
                format!(
                    "Review the summary for '{child_display}', edit if needed, then send it to share updates."
                ),
                None,
            );
        } else {
            self.chat_widget
                .add_info_message(format!("Closed thread '{child_display}'."), None);
        }
    }

    fn resolve_parent_index(&self, origin: &ThreadOrigin) -> usize {
        self.chat_widget.parent_index_for(origin).unwrap_or(0)
    }

    fn parent_label_for_origin(&self, origin: &ThreadOrigin) -> Option<String> {
        self.chat_widget
            .parent_index_for(origin)
            .map(|idx| self.chat_widget.display_thread_path_of(idx).join("/"))
    }

    fn event_marks_unread(msg: &EventMsg) -> bool {
        matches!(
            msg,
            EventMsg::AgentMessage(_)
                | EventMsg::BackgroundEvent(BackgroundEventEvent { .. })
                | EventMsg::TaskComplete(TaskCompleteEvent {
                    last_agent_message: Some(_),
                    ..
                })
        )
    }

    fn build_summary_text(&self, title: &str, new_entries: usize) -> String {
        let plural = if new_entries == 1 { "" } else { "s" };
        if new_entries == 0 {
            format!("Thread '{title}' summary (no new messages since fork):\n- Notes:\n  - ")
        } else {
            format!(
                "Thread '{title}' summary ({new_entries} new message{plural} since fork):\n- Key updates:\n  - "
            )
        }
    }

    fn next_child_thread_title(&self) -> String {
        let existing = self
            .chat_widget
            .sessions()
            .iter()
            .filter(|s| s.origin.is_some())
            .count();
        let next = existing + 1;
        format!("thread-{next}")
    }

    fn next_top_level_thread_title(&self) -> String {
        let top_level = self
            .chat_widget
            .sessions()
            .iter()
            .filter(|s| s.origin.is_none())
            .count();
        format!("#main-{top_level}")
    }

    fn on_suggest_thread_name(&mut self, tui: &mut tui::Tui, session_id: SessionId, text: String) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        let Some(idx) = self.chat_widget.index_for_session_id(session_id) else {
            return;
        };
        if self
            .chat_widget
            .sessions()
            .get(idx)
            .and_then(|handle| handle.auto_name_kind)
            .is_none()
        {
            return;
        }

        let Some(summary) = Self::topic_summary(trimmed) else {
            if let Some(handle) = self.chat_widget.session_mut(idx) {
                handle.auto_name_kind = None;
            }
            return;
        };

        if self
            .chat_widget
            .update_display_title(idx, Some(summary))
        {
            if self.chat_widget.active_index() == idx {
                self.apply_display_labels_for(idx);
                let label = self.chat_widget.display_label_for_index(idx);
                let display_summary = self
                    .chat_widget
                    .display_summary_for_index(idx)
                    .map(str::to_string);
                if let Some(display_summary) = display_summary {
                    self.chat_widget
                        .add_info_message(format!("Named thread '{label}' — {display_summary}"), None);
                } else {
                    self.chat_widget
                        .add_info_message(format!("Named thread '{label}'"), None);
                }
            }
            tui.frame_requester().schedule_frame();
        }
    }

    fn topic_summary(text: &str) -> Option<String> {
        concise_request_summary(text, 10, 60)
    }

    fn apply_display_labels_for(&mut self, idx: usize) {
        let base_label = self.chat_widget.display_label_for_index(idx);
        let title = if let Some(summary) = self.chat_widget.display_summary_for_index(idx) {
            format!("{base_label} • {summary}")
        } else {
            base_label
        };
        let path = self.chat_widget.display_thread_path_of(idx);
        self.chat_widget.set_active_title(title);
        self.chat_widget.set_active_thread_path(path);
        self.update_active_thread_header();
    }

    fn update_active_thread_header(&mut self) {
        let path = self
            .chat_widget
            .display_thread_path_of(self.chat_widget.active_index());
        self.chat_widget.set_active_thread_path(path);
    }

    fn on_quit_requested(&mut self) {
        let additional_threads = self.chat_widget.len().saturating_sub(1);

        if additional_threads > 0 {
            let plural = if additional_threads == 1 { "" } else { "s" };
            self.chat_widget.add_info_message(
                format!("Warning: exiting will close {additional_threads} open thread{plural}."),
                Some("Use /close to summarize a thread before quitting.".to_string()),
            );
        }

        self.app_event_tx.send(AppEvent::ExitRequest);
    }

    async fn on_conversation_history_for_thread(
        &mut self,
        tui: &mut tui::Tui,
        ev: &ConversationPathResponseEvent,
    ) -> Result<bool> {
        let pending = match self.thread_fork_pending.take() {
            Some(pending) if pending.base_id == ev.conversation_id => pending,
            Some(pending) => {
                self.thread_fork_pending = Some(pending);
                return Ok(false);
            }
            None => return Ok(false),
        };
        let cfg = self.config.clone();
        let path = ev.path.clone();

        match self
            .server
            .resume_conversation_from_rollout(cfg.clone(), path, self.auth_manager.clone())
            .await
        {
            Ok(new_conv) => {
                let session_id = self.alloc_session_id();
                self.install_thread_session(tui, cfg, new_conv, pending.title, session_id);
            }
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to create thread: {err}"));
            }
        }

        Ok(true)
    }

    fn install_thread_session(
        &mut self,
        tui: &mut tui::Tui,
        cfg: Config,
        new_conv: codex_core::NewConversation,
        title: String,
        session_id: SessionId,
    ) {
        let parent_index = self.chat_widget.active_index();
        let parent_conversation_id = self
            .chat_widget
            .sessions()
            .get(parent_index)
            .and_then(|s| s.conversation_id);
        let parent_snapshot_len = self
            .chat_widget
            .sessions()
            .get(parent_index)
            .map(|s| s.transcript_cells.len())
            .unwrap_or(0);

        let base_transcript = self.chat_widget.active_transcript().clone();
        let session_configured = new_conv.session_configured.clone();

        let init = crate::chatwidget::ChatWidgetInit {
            config: cfg,
            frame_requester: tui.frame_requester(),
            app_event_tx: self.app_event_tx.clone(),
            initial_prompt: None,
            initial_images: Vec::new(),
            enhanced_keys_supported: self.enhanced_keys_supported,
            auth_manager: self.auth_manager.clone(),
            session_id,
        };

        let new_widget = crate::chatwidget::ChatWidget::new_from_existing(
            init,
            new_conv.conversation,
            new_conv.session_configured,
        );

        let new_index = self.chat_widget.add_session(new_widget, title, session_id);
        if let Some(handle) = self.chat_widget.session_mut(new_index) {
            handle.transcript_cells = base_transcript;
            handle.conversation_id = Some(session_configured.session_id);
            handle.origin = Some(SessionManager::thread_origin(
                parent_index,
                parent_conversation_id,
                parent_snapshot_len,
            ));
            handle.auto_name_kind = Some(AutoNameKind::Child);
        }

        self.chat_widget.refresh_thread_paths();
        self.chat_widget.switch_active(new_index);
        if let Some(handle) = self.chat_widget.session_mut(new_index) {
            handle.widget.clear_request_summary();
        }
        self.apply_display_labels_for(new_index);
        self.chat_widget
            .set_active_conversation_id(session_configured.session_id);

        self.has_emitted_history_lines = false;
        self.deferred_history_lines.clear();
        self.overlay = None;
        self.render_transcript_once(tui);
        let label = self.chat_widget.display_label_for_index(new_index);
        let summary = self
            .chat_widget
            .display_summary_for_index(new_index)
            .map(str::to_string);
        if let Some(summary) = summary {
            self.chat_widget
                .add_info_message(format!("Switched to {label} — {summary}"), None);
        } else {
            self.chat_widget
                .add_info_message(format!("Switched to {label}"), None);
        }
        tui.frame_requester().schedule_frame();
    }

    async fn handle_key_event(&mut self, tui: &mut tui::Tui, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::F(7),
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                self.switch_thread_relative(tui, -1);
            }
            KeyEvent {
                code: KeyCode::F(8),
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                self.switch_thread_relative(tui, 1);
            }
            KeyEvent {
                code: KeyCode::Char('t'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } => {
                // Enter alternate screen and set viewport to full size.
                let _ = tui.enter_alt_screen();
                let transcript = self.chat_widget.active_transcript().clone();
                self.overlay = Some(Overlay::new_transcript(transcript));
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
    use codex_protocol::mcp_protocol::ConversationId;
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

        let session_id = chat_widget.session_id();

        App {
            server,
            app_event_tx,
            chat_widget: SessionManager::single(chat_widget, session_id),
            auth_manager,
            config,
            active_profile: None,
            file_search,
            overlay: None,
            deferred_history_lines: Vec::new(),
            has_emitted_history_lines: false,
            enhanced_keys_supported: false,
            commit_anim_running: Arc::new(AtomicBool::new(false)),
            backtrack: BacktrackState::default(),
            thread_fork_pending: None,
            next_session_id: session_id.raw().saturating_add(1),
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
            let path = if is_first {
                vec!["main".to_string()]
            } else {
                vec!["main".to_string(), "fork".to_string()]
            };
            Arc::new(new_session_info(
                app.chat_widget.config_ref(),
                event,
                is_first,
                &path,
            )) as Arc<dyn HistoryCell>
        };

        // Simulate the transcript after trimming for a fork, replaying history, and
        // appending the edited turn. The session header separates the retained history
        // from the forked conversation's replayed turns.
        *app.chat_widget.active_transcript_mut() = vec![
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

        assert_eq!(user_count(app.chat_widget.active_transcript()), 2);

        app.backtrack.base_id = Some(ConversationId::new());
        app.backtrack.primed = true;
        app.backtrack.nth_user_message =
            user_count(app.chat_widget.active_transcript()).saturating_sub(1);

        app.confirm_backtrack_from_main();

        let (_, nth, prefill) = app.backtrack.pending.clone().expect("pending backtrack");
        assert_eq!(nth, 1);
        assert_eq!(prefill, "follow-up (edited)");
    }

}
