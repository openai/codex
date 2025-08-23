use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::chatwidget::ChatWidget;
use crate::file_search::FileSearchManager;
use crate::transcript_app::TranscriptApp;
use crate::tui;
use crate::tui::TuiEvent;
use codex_ansi_escape::ansi_escape_line;
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::protocol::TokenUsage;
use codex_login::AuthManager;
use color_eyre::eyre::Result;
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
    pub(crate) app_event_tx: AppEventSender,
    pub(crate) chat_widget: ChatWidget,

    /// Config is stored here so we can recreate ChatWidgets as needed.
    pub(crate) config: Config,

    pub(crate) file_search: FileSearchManager,

    pub(crate) transcript_lines: Vec<Line<'static>>,

    // Transcript overlay state
    pub(crate) transcript_overlay: Option<TranscriptApp>,
    // If true, overlay is opened as an Esc-backtrack preview.
    pub(crate) transcript_overlay_is_backtrack: bool,
    pub(crate) deferred_history_lines: Vec<Line<'static>>,

    pub(crate) enhanced_keys_supported: bool,

    /// Controls the animation thread that sends CommitTick events.
    pub(crate) commit_anim_running: Arc<AtomicBool>,

    // Esc-backtracking state
    pub(crate) esc_backtrack_primed: bool,
    pub(crate) esc_backtrack_base: Option<uuid::Uuid>,
    pub(crate) esc_backtrack_count: usize,
    // Pending: base_id, drop_count, prefill text
    pub(crate) pending_backtrack: Option<(uuid::Uuid, usize, String)>,
}

impl App {
    pub async fn run(
        tui: &mut tui::Tui,
        auth_manager: Arc<AuthManager>,
        config: Config,
        initial_prompt: Option<String>,
        initial_images: Vec<PathBuf>,
    ) -> Result<TokenUsage> {
        use tokio_stream::StreamExt;
        let (app_event_tx, mut app_event_rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(app_event_tx);

        let conversation_manager = Arc::new(ConversationManager::new(auth_manager.clone()));

        let enhanced_keys_supported = supports_keyboard_enhancement().unwrap_or(false);

        let chat_widget = ChatWidget::new(
            config.clone(),
            conversation_manager.clone(),
            tui.frame_requester(),
            app_event_tx.clone(),
            initial_prompt,
            initial_images,
            enhanced_keys_supported,
        );

        let file_search = FileSearchManager::new(config.cwd.clone(), app_event_tx.clone());

        let mut app = Self {
            server: conversation_manager,
            app_event_tx,
            chat_widget,
            config,
            file_search,
            enhanced_keys_supported,
            transcript_lines: Vec::new(),
            transcript_overlay: None,
            transcript_overlay_is_backtrack: false,
            deferred_history_lines: Vec::new(),
            commit_anim_running: Arc::new(AtomicBool::new(false)),
            esc_backtrack_primed: false,
            esc_backtrack_base: None,
            esc_backtrack_count: 0,
            pending_backtrack: None,
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
        if self.transcript_overlay.is_some() {
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
                TuiEvent::AttachImage {
                    path,
                    width,
                    height,
                    format_label,
                } => {
                    self.chat_widget
                        .attach_image(path, width, height, format_label);
                }
            }
        }
        Ok(true)
    }

    async fn handle_event(&mut self, tui: &mut tui::Tui, event: AppEvent) -> Result<bool> {
        match event {
            AppEvent::NewSession => {
                self.chat_widget = ChatWidget::new(
                    self.config.clone(),
                    self.server.clone(),
                    tui.frame_requester(),
                    self.app_event_tx.clone(),
                    None,
                    Vec::new(),
                    self.enhanced_keys_supported,
                );
                tui.frame_requester().schedule_frame();
            }
            AppEvent::InsertHistoryLines(lines) => {
                if let Some(overlay) = &mut self.transcript_overlay {
                    overlay.insert_lines(lines.clone());
                    tui.frame_requester().schedule_frame();
                }
                self.transcript_lines.extend(lines.clone());
                if self.transcript_overlay.is_some() {
                    self.deferred_history_lines.extend(lines);
                } else {
                    tui.insert_history_lines(lines);
                }
            }
            AppEvent::InsertHistoryCell(cell) => {
                if let Some(overlay) = &mut self.transcript_overlay {
                    overlay.insert_lines(cell.transcript_lines());
                    tui.frame_requester().schedule_frame();
                }
                self.transcript_lines.extend(cell.transcript_lines());
                let display = cell.display_lines();
                if !display.is_empty() {
                    if self.transcript_overlay.is_some() {
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
                // If a backtrack is pending and this history corresponds to the base session, fork.
                if let Some((base_id, _, _)) = self.pending_backtrack.as_ref() {
                    if ev.conversation_id == *base_id {
                        // Safe to take now that we know it's the matching response.
                        if let Some((_, drop_count, prefill)) = self.pending_backtrack.take() {
                            // Fork using provided history entries.
                            let cfg = self.chat_widget.config_ref().clone();
                            match self
                                .server
                                .fork_conversation(ev.entries.clone(), drop_count, cfg.clone())
                                .await
                            {
                                Ok(new_conv) => {
                                    let conv = new_conv.conversation;
                                    let session_configured = new_conv.session_configured;
                                    self.chat_widget = ChatWidget::new_from_existing(
                                        cfg,
                                        conv,
                                        session_configured,
                                        tui.frame_requester(),
                                        self.app_event_tx.clone(),
                                        self.enhanced_keys_supported,
                                    );

                                    // Trim transcript to preserve only content up to the selected user message.
                                    if let Some(cut_idx) =
                                        crate::backtrack_helpers::find_nth_last_user_header_index(
                                            &self.transcript_lines,
                                            drop_count,
                                        )
                                    {
                                        self.transcript_lines.truncate(cut_idx);
                                    } else {
                                        self.transcript_lines.clear();
                                    }
                                    self.render_transcript_once(tui);

                                    if !prefill.is_empty() {
                                        self.chat_widget.insert_str(&prefill);
                                    }
                                    tui.frame_requester().schedule_frame();
                                }
                                Err(e) => {
                                    tracing::error!("error forking conversation: {e:#}");
                                }
                            }
                        }
                    } else {
                        // Not matching base; ignore and keep pending.
                    }
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
                self.transcript_overlay = Some(TranscriptApp::with_title(
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
        }
        Ok(true)
    }

    pub(crate) fn token_usage(&self) -> codex_core::protocol::TokenUsage {
        self.chat_widget.token_usage().clone()
    }

    async fn handle_key_event(&mut self, tui: &mut tui::Tui, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: crossterm::event::KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } => {
                self.chat_widget.on_ctrl_c();
            }
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: crossterm::event::KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } if self.chat_widget.composer_is_empty() => {
                self.app_event_tx.send(AppEvent::ExitRequest);
            }
            KeyEvent {
                code: KeyCode::Char('t'),
                modifiers: crossterm::event::KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            } => {
                // Enter alternate screen and set viewport to full size.
                let _ = tui.enter_alt_screen();
                self.transcript_overlay = Some(TranscriptApp::new(self.transcript_lines.clone()));
                tui.frame_requester().schedule_frame();
            }
            // Esc primes/advances backtracking when composer is empty.
            KeyEvent {
                code: KeyCode::Esc,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                self.handle_backtrack_esc_key(tui);
            }
            // Enter confirms backtrack when primed + count > 0. Otherwise pass to widget.
            KeyEvent {
                code: KeyCode::Enter,
                kind: KeyEventKind::Press,
                ..
            } if self.esc_backtrack_primed
                && self.esc_backtrack_count > 0
                && self.chat_widget.composer_is_empty() =>
            {
                if let Some(base_id) = self.esc_backtrack_base {
                    let drop_last_messages = self.esc_backtrack_count;
                    let prefill = crate::backtrack_helpers::nth_last_user_text(
                        &self.transcript_lines,
                        drop_last_messages,
                    )
                    .unwrap_or_default();
                    self.request_backtrack(prefill, base_id, drop_last_messages);
                }
                // Reset backtrack state after confirming.
                self.esc_backtrack_primed = false;
                self.esc_backtrack_base = None;
                self.esc_backtrack_count = 0;
            }
            KeyEvent {
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                // Any non-Esc key press should cancel a primed backtrack.
                // This avoids stale "Esc-primed" state after the user starts typing
                // (even if they later backspace to empty).
                if key_event.code != KeyCode::Esc && self.esc_backtrack_primed {
                    self.esc_backtrack_primed = false;
                    self.esc_backtrack_base = None;
                    self.esc_backtrack_count = 0;
                }
                self.chat_widget.handle_key_event(key_event);
            }
            _ => {
                // Ignore Release key events.
            }
        };
    }
}
