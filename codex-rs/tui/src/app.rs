use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::chatwidget::ChatWidget;
use crate::file_search::FileSearchManager;
use crate::transcript_app::TranscriptApp;
use crate::tui;
use crate::tui::TuiEvent;
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::protocol::TokenUsage;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::supports_keyboard_enhancement;
use ratatui::layout::Rect;
use ratatui::text::Line;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;
use tokio::select;
use tokio::sync::mpsc::unbounded_channel;

pub(crate) struct App {
    server: Arc<ConversationManager>,
    app_event_tx: AppEventSender,
    chat_widget: ChatWidget,

    /// Config is stored here so we can recreate ChatWidgets as needed.
    config: Config,

    file_search: FileSearchManager,

    transcript_lines: Vec<Line<'static>>,

    // Transcript overlay state
    transcript_overlay: Option<TranscriptApp>,
    deferred_history_lines: Vec<Line<'static>>,
    transcript_saved_viewport: Option<Rect>,

    enhanced_keys_supported: bool,

    /// Controls the animation thread that sends CommitTick events.
    commit_anim_running: Arc<AtomicBool>,
}

impl App {
    pub async fn run(
        tui: &mut tui::Tui,
        config: Config,
        initial_prompt: Option<String>,
        initial_images: Vec<PathBuf>,
    ) -> Result<TokenUsage> {
        use tokio_stream::StreamExt;
        let (app_event_tx, mut app_event_rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(app_event_tx);

        let conversation_manager = Arc::new(ConversationManager::default());

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
            deferred_history_lines: Vec::new(),
            transcript_saved_viewport: None,
            commit_anim_running: Arc::new(AtomicBool::new(false)),
        };

        let tui_events = tui.event_stream();
        tokio::pin!(tui_events);

        tui.frame_requester().schedule_frame();

        while select! {
            Some(event) = app_event_rx.recv() => {
                app.handle_event(tui, event)?
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
        if let Some(overlay) = &mut self.transcript_overlay {
            overlay.handle_event(tui, event)?;
            if overlay.is_done {
                // Exit alternate screen and restore viewport.
                let _ = execute!(tui.terminal.backend_mut(), LeaveAlternateScreen);
                if let Some(saved) = self.transcript_saved_viewport.take() {
                    tui.terminal.set_viewport_area(saved);
                }
                if !self.deferred_history_lines.is_empty() {
                    let lines = std::mem::take(&mut self.deferred_history_lines);
                    tui.insert_history_lines(lines);
                }
                self.transcript_overlay = None;
            }
            tui.frame_requester().schedule_frame();
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
                #[cfg(unix)]
                TuiEvent::ResumeFromSuspend => {
                    let cursor_pos = tui.terminal.get_cursor_position()?;
                    tui.terminal.set_viewport_area(ratatui::layout::Rect::new(
                        0,
                        cursor_pos.y,
                        0,
                        0,
                    ));
                }
            }
        }
        Ok(true)
    }

    fn handle_event(&mut self, tui: &mut tui::Tui, event: AppEvent) -> Result<bool> {
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
            AppEvent::ExitRequest => {
                return Ok(false);
            }
            AppEvent::CodexOp(op) => self.chat_widget.submit_op(op),
            AppEvent::DiffResult(text) => {
                self.chat_widget.add_diff_output(text);
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
                let _ = execute!(tui.terminal.backend_mut(), EnterAlternateScreen);
                if let Ok(size) = tui.terminal.size() {
                    self.transcript_saved_viewport = Some(tui.terminal.viewport_area);
                    tui.terminal
                        .set_viewport_area(Rect::new(0, 0, size.width, size.height));
                    let _ = tui.terminal.clear();
                }

                self.transcript_overlay = Some(TranscriptApp::new(self.transcript_lines.clone()));
                tui.frame_requester().schedule_frame();
            }
            KeyEvent {
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            } => {
                self.chat_widget.handle_key_event(key_event);
            }
            _ => {
                // Ignore Release key events.
            }
        };
    }
}
