use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::chatwidget::ChatWidget;
use crate::file_search::FileSearchManager;
use crate::get_git_diff::get_git_diff;
use crate::insert_history::write_lines;
use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::OnboardingScreen;
use crate::onboarding::onboarding_screen::OnboardingScreenArgs;
use crate::should_show_login_screen;
use crate::slash_command::SlashCommand;
use crate::tui;
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::protocol::Event;
use codex_core::protocol::Op;
use color_eyre::eyre::Result;
use crossterm::SynchronizedUpdate;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::terminal::supports_keyboard_enhancement;
use ratatui::layout::Offset;
use ratatui::prelude::Backend;
use ratatui::text::Line;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use tempfile::NamedTempFile;

/// Time window for debouncing redraw requests.
const REDRAW_DEBOUNCE: Duration = Duration::from_millis(1);

/// Top-level application state: which full-screen view is currently active.
#[allow(clippy::large_enum_variant)]
enum AppState<'a> {
    Onboarding {
        screen: OnboardingScreen,
    },
    /// The main chat UI is visible.
    Chat {
        /// Boxed to avoid a large enum variant and reduce the overall size of
        /// `AppState`.
        widget: Box<ChatWidget<'a>>,
    },
}

pub(crate) struct App<'a> {
    server: Arc<ConversationManager>,
    app_event_tx: AppEventSender,
    app_event_rx: Receiver<AppEvent>,
    app_state: AppState<'a>,

    /// Config is stored here so we can recreate ChatWidgets as needed.
    config: Config,

    file_search: FileSearchManager,

    pending_history_lines: Vec<Line<'static>>,
    transcript: Vec<Line<'static>>,

    enhanced_keys_supported: bool,

    /// Controls the animation thread that sends CommitTick events.
    commit_anim_running: Arc<AtomicBool>,

    /// Channel to schedule one-shot animation frames; coalesced by a single
    /// scheduler thread.
    frame_schedule_tx: std::sync::mpsc::Sender<Instant>,
    event_reader_enabled: Arc<AtomicBool>,
}

/// Aggregate parameters needed to create a `ChatWidget`, as creation may be
/// deferred until after the Git warning screen is dismissed.
#[derive(Clone, Debug)]
pub(crate) struct ChatWidgetArgs {
    pub(crate) config: Config,
    initial_prompt: Option<String>,
    initial_images: Vec<PathBuf>,
    enhanced_keys_supported: bool,
}

impl App<'_> {
    pub(crate) fn new(
        config: Config,
        initial_prompt: Option<String>,
        initial_images: Vec<std::path::PathBuf>,
        show_trust_screen: bool,
    ) -> Self {
        let conversation_manager = Arc::new(ConversationManager::default());

        let (app_event_tx, app_event_rx) = channel();
        let app_event_tx = AppEventSender::new(app_event_tx);
        let event_reader_enabled = Arc::new(AtomicBool::new(true));

        let enhanced_keys_supported = supports_keyboard_enhancement().unwrap_or(false);

        // Spawn a dedicated thread for reading the crossterm event loop and
        // re-publishing the events as AppEvents, as appropriate.
        {
            let app_event_tx = app_event_tx.clone();
            let events_enabled = event_reader_enabled.clone();
            std::thread::spawn(move || {
                loop {
                    if !events_enabled.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(50));
                        continue;
                    }
                    // This timeout is necessary to avoid holding the event lock
                    // that crossterm::event::read() acquires. In particular,
                    // reading the cursor position (crossterm::cursor::position())
                    // needs to acquire the event lock, and so will fail if it
                    // can't acquire it within 2 sec. Resizing the terminal
                    // crashes the app if the cursor position can't be read.
                    if let Ok(true) = crossterm::event::poll(Duration::from_millis(100)) {
                        if let Ok(event) = crossterm::event::read() {
                            match event {
                                crossterm::event::Event::Key(key_event) => {
                                    app_event_tx.send(AppEvent::KeyEvent(key_event));
                                }
                                crossterm::event::Event::Resize(_, _) => {
                                    app_event_tx.send(AppEvent::RequestRedraw);
                                }
                                crossterm::event::Event::Paste(pasted) => {
                                    // Many terminals convert newlines to \r when pasting (e.g., iTerm2),
                                    // but tui-textarea expects \n. Normalize CR to LF.
                                    // [tui-textarea]: https://github.com/rhysd/tui-textarea/blob/4d18622eeac13b309e0ff6a55a46ac6706da68cf/src/textarea.rs#L782-L783
                                    // [iTerm2]: https://github.com/gnachman/iTerm2/blob/5d0c0d9f68523cbd0494dad5422998964a2ecd8d/sources/iTermPasteHelper.m#L206-L216
                                    let pasted = pasted.replace("\r", "\n");
                                    app_event_tx.send(AppEvent::Paste(pasted));
                                }
                                _ => {
                                    // Ignore any other events.
                                }
                            }
                        }
                    } else {
                        // Timeout expired, no `Event` is available
                    }
                }
            });
        }

        let show_login_screen = should_show_login_screen(&config);
        let app_state = if show_login_screen || show_trust_screen {
            let chat_widget_args = ChatWidgetArgs {
                config: config.clone(),
                initial_prompt,
                initial_images,
                enhanced_keys_supported,
            };
            AppState::Onboarding {
                screen: OnboardingScreen::new(OnboardingScreenArgs {
                    event_tx: app_event_tx.clone(),
                    codex_home: config.codex_home.clone(),
                    cwd: config.cwd.clone(),
                    show_login_screen,
                    show_trust_screen,
                    chat_widget_args,
                }),
            }
        } else {
            let chat_widget = ChatWidget::new(
                config.clone(),
                conversation_manager.clone(),
                app_event_tx.clone(),
                initial_prompt,
                initial_images,
                enhanced_keys_supported,
            );
            AppState::Chat {
                widget: Box::new(chat_widget),
            }
        };

        let file_search = FileSearchManager::new(config.cwd.clone(), app_event_tx.clone());

        // Spawn a single scheduler thread that coalesces both debounced redraw
        // requests and animation frame requests, and emits a single Redraw event
        // at the earliest requested time.
        let (frame_tx, frame_rx) = channel::<Instant>();
        {
            let app_event_tx = app_event_tx.clone();
            std::thread::spawn(move || {
                use std::sync::mpsc::RecvTimeoutError;
                let mut next_deadline: Option<Instant> = None;
                loop {
                    if next_deadline.is_none() {
                        match frame_rx.recv() {
                            Ok(deadline) => next_deadline = Some(deadline),
                            Err(_) => break,
                        }
                    }

                    #[expect(clippy::expect_used)]
                    let deadline = next_deadline.expect("deadline set");
                    let now = Instant::now();
                    let timeout = if deadline > now {
                        deadline - now
                    } else {
                        Duration::from_millis(0)
                    };

                    match frame_rx.recv_timeout(timeout) {
                        Ok(new_deadline) => {
                            next_deadline =
                                Some(next_deadline.map_or(new_deadline, |d| d.min(new_deadline)));
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            app_event_tx.send(AppEvent::Redraw);
                            next_deadline = None;
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            });
        }
        Self {
            server: conversation_manager,
            app_event_tx,
            pending_history_lines: Vec::new(),
            transcript: Vec::new(),
            app_event_rx,
            app_state,
            config,
            file_search,
            enhanced_keys_supported,
            commit_anim_running: Arc::new(AtomicBool::new(false)),
            frame_schedule_tx: frame_tx,
            event_reader_enabled,
        }
    }

    fn schedule_frame_in(&self, dur: Duration) {
        let _ = self.frame_schedule_tx.send(Instant::now() + dur);
    }

    pub(crate) fn run(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        // Schedule the first render immediately.
        let _ = self.frame_schedule_tx.send(Instant::now());

        while let Ok(event) = self.app_event_rx.recv() {
            match event {
                AppEvent::InsertHistoryLines(lines) => {
                    self.transcript.extend(lines.clone());
                    self.pending_history_lines.extend(lines);
                    self.app_event_tx.send(AppEvent::RequestRedraw);
                }
                AppEvent::InsertHistoryCell(cell) => {
                    self.transcript.extend(cell.transcript_lines());
                    self.pending_history_lines.extend(cell.display_lines());
                    self.app_event_tx.send(AppEvent::RequestRedraw);
                }
                AppEvent::RequestRedraw => {
                    self.schedule_frame_in(REDRAW_DEBOUNCE);
                }
                AppEvent::ScheduleFrameIn(dur) => {
                    self.schedule_frame_in(dur);
                }
                AppEvent::Redraw => {
                    std::io::stdout().sync_update(|_| self.draw_next_frame(terminal))??;
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
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.on_commit_tick();
                    }
                }
                AppEvent::KeyEvent(key_event) => {
                    match key_event {
                        KeyEvent {
                            code: KeyCode::Char('c'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
                            kind: KeyEventKind::Press,
                            ..
                        } => match &mut self.app_state {
                            AppState::Chat { widget } => {
                                widget.on_ctrl_c();
                            }
                            AppState::Onboarding { .. } => {
                                self.app_event_tx.send(AppEvent::ExitRequest);
                            }
                        },
                        KeyEvent {
                            code: KeyCode::Char('z'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
                            kind: KeyEventKind::Press,
                            ..
                        } => {
                            #[cfg(unix)]
                            {
                                self.suspend(terminal)?;
                            }
                            // No-op on non-Unix platforms.
                        }
                        KeyEvent {
                            code: KeyCode::Char('t'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
                            kind: KeyEventKind::Press,
                            ..
                        } => {
                            self.event_reader_enabled.store(false, Ordering::Relaxed);
                            let mut tmp = NamedTempFile::new()?;
                            write_lines(tmp.as_file_mut(), self.transcript.clone());
                            tmp.flush().ok();
                            let path = tmp.into_temp_path();
                            tui::restore_modes()?;
                            let _ = Command::new("less")
                                .arg("-R")
                                .arg("+G")
                                .arg(path.as_os_str())
                                .status();
                            let _ = path.close();
                            tui::set_modes()?;
                            self.event_reader_enabled.store(true, Ordering::Relaxed);
                            self.app_event_tx.send(AppEvent::RequestRedraw);
                        }
                        KeyEvent {
                            code: KeyCode::Char('d'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
                            kind: KeyEventKind::Press,
                            ..
                        } => {
                            match &mut self.app_state {
                                AppState::Chat { widget } => {
                                    if widget.composer_is_empty() {
                                        self.app_event_tx.send(AppEvent::ExitRequest);
                                    } else {
                                        // Treat Ctrl+D as a normal key event when the composer
                                        // is not empty so that it doesn't quit the application
                                        // prematurely.
                                        self.dispatch_key_event(key_event);
                                    }
                                }
                                AppState::Onboarding { .. } => {
                                    self.app_event_tx.send(AppEvent::ExitRequest);
                                }
                            }
                        }
                        KeyEvent {
                            kind: KeyEventKind::Press | KeyEventKind::Repeat,
                            ..
                        } => {
                            self.dispatch_key_event(key_event);
                        }
                        _ => {
                            // Ignore Release key events.
                        }
                    };
                }
                AppEvent::Paste(text) => {
                    self.dispatch_paste_event(text);
                }
                AppEvent::CodexEvent(event) => {
                    self.dispatch_codex_event(event);
                }
                AppEvent::ExitRequest => {
                    break;
                }
                AppEvent::CodexOp(op) => match &mut self.app_state {
                    AppState::Chat { widget } => widget.submit_op(op),
                    AppState::Onboarding { .. } => {}
                },
                AppEvent::DispatchCommand(command) => match command {
                    SlashCommand::New => {
                        // User accepted – switch to chat view.
                        let new_widget = Box::new(ChatWidget::new(
                            self.config.clone(),
                            self.server.clone(),
                            self.app_event_tx.clone(),
                            None,
                            Vec::new(),
                            self.enhanced_keys_supported,
                        ));
                        self.app_state = AppState::Chat { widget: new_widget };
                        self.app_event_tx.send(AppEvent::RequestRedraw);
                    }
                    SlashCommand::Init => {
                        // Guard: do not run if a task is active.
                        if let AppState::Chat { widget } = &mut self.app_state {
                            const INIT_PROMPT: &str = include_str!("../prompt_for_init_command.md");
                            widget.submit_text_message(INIT_PROMPT.to_string());
                        }
                    }
                    SlashCommand::Compact => {
                        if let AppState::Chat { widget } = &mut self.app_state {
                            widget.clear_token_usage();
                            self.app_event_tx.send(AppEvent::CodexOp(Op::Compact));
                        }
                    }
                    SlashCommand::Quit => {
                        break;
                    }
                    SlashCommand::Logout => {
                        if let Err(e) = codex_login::logout(&self.config.codex_home) {
                            tracing::error!("failed to logout: {e}");
                        }
                        break;
                    }
                    SlashCommand::Diff => {
                        let (is_git_repo, diff_text) = match get_git_diff() {
                            Ok(v) => v,
                            Err(e) => {
                                let msg = format!("Failed to compute diff: {e}");
                                if let AppState::Chat { widget } = &mut self.app_state {
                                    widget.add_diff_output(msg);
                                }
                                continue;
                            }
                        };

                        if let AppState::Chat { widget } = &mut self.app_state {
                            let text = if is_git_repo {
                                diff_text
                            } else {
                                "`/diff` — _not inside a git repository_".to_string()
                            };
                            widget.add_diff_output(text);
                        }
                    }
                    SlashCommand::Mention => {
                        if let AppState::Chat { widget } = &mut self.app_state {
                            widget.insert_str("@");
                        }
                    }
                    SlashCommand::Status => {
                        if let AppState::Chat { widget } = &mut self.app_state {
                            widget.add_status_output();
                        }
                    }
                    SlashCommand::Prompts => {
                        if let AppState::Chat { widget } = &mut self.app_state {
                            widget.add_prompts_output();
                        }
                    }
                    #[cfg(debug_assertions)]
                    SlashCommand::TestApproval => {
                        use codex_core::protocol::EventMsg;
                        use std::collections::HashMap;

                        use codex_core::protocol::ApplyPatchApprovalRequestEvent;
                        use codex_core::protocol::FileChange;

                        self.app_event_tx.send(AppEvent::CodexEvent(Event {
                            id: "1".to_string(),
                            // msg: EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                            //     call_id: "1".to_string(),
                            //     command: vec!["git".into(), "apply".into()],
                            //     cwd: self.config.cwd.clone(),
                            //     reason: Some("test".to_string()),
                            // }),
                            msg: EventMsg::ApplyPatchApprovalRequest(
                                ApplyPatchApprovalRequestEvent {
                                    call_id: "1".to_string(),
                                    changes: HashMap::from([
                                        (
                                            PathBuf::from("/tmp/test.txt"),
                                            FileChange::Add {
                                                content: "test".to_string(),
                                            },
                                        ),
                                        (
                                            PathBuf::from("/tmp/test2.txt"),
                                            FileChange::Update {
                                                unified_diff: "+test\n-test2".to_string(),
                                                move_path: None,
                                            },
                                        ),
                                    ]),
                                    reason: None,
                                    grant_root: Some(PathBuf::from("/tmp")),
                                },
                            ),
                        }));
                    }
                },
                AppEvent::OnboardingAuthComplete(result) => {
                    if let AppState::Onboarding { screen } = &mut self.app_state {
                        screen.on_auth_complete(result);
                    }
                }
                AppEvent::OnboardingComplete(ChatWidgetArgs {
                    config,
                    enhanced_keys_supported,
                    initial_images,
                    initial_prompt,
                }) => {
                    self.app_state = AppState::Chat {
                        widget: Box::new(ChatWidget::new(
                            config,
                            self.server.clone(),
                            self.app_event_tx.clone(),
                            initial_prompt,
                            initial_images,
                            enhanced_keys_supported,
                        )),
                    }
                }
                AppEvent::StartFileSearch(query) => {
                    if !query.is_empty() {
                        self.file_search.on_user_query(query);
                    }
                }
                AppEvent::FileSearchResult { query, matches } => {
                    if let AppState::Chat { widget } = &mut self.app_state {
                        widget.apply_file_search_result(query, matches);
                    }
                }
            }
        }
        terminal.clear()?;

        Ok(())
    }

    #[cfg(unix)]
    fn suspend(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        tui::restore_modes()?;
        // SAFETY: Unix-only code path. We intentionally send SIGTSTP to the
        // current process group (pid 0) to trigger standard job-control
        // suspension semantics. This FFI does not involve any raw pointers,
        // is not called from a signal handler, and uses a constant signal.
        // Errors from kill are acceptable (e.g., if already stopped) — the
        // subsequent re-init path will still leave the terminal in a good state.
        // We considered `nix`, but didn't think it was worth pulling in for this one call.
        unsafe { libc::kill(0, libc::SIGTSTP) };
        *terminal = tui::init(&self.config)?;
        terminal.clear()?;
        self.app_event_tx.send(AppEvent::RequestRedraw);
        Ok(())
    }

    pub(crate) fn token_usage(&self) -> codex_core::protocol::TokenUsage {
        match &self.app_state {
            AppState::Chat { widget } => widget.token_usage().clone(),
            AppState::Onboarding { .. } => codex_core::protocol::TokenUsage::default(),
        }
    }

    fn draw_next_frame(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        if matches!(self.app_state, AppState::Onboarding { .. }) {
            terminal.clear()?;
        }

        let screen_size = terminal.size()?;
        let last_known_screen_size = terminal.last_known_screen_size;
        if screen_size != last_known_screen_size {
            let cursor_pos = terminal.get_cursor_position()?;
            let last_known_cursor_pos = terminal.last_known_cursor_pos;
            if cursor_pos.y != last_known_cursor_pos.y {
                // The terminal was resized. The only point of reference we have for where our viewport
                // was moved is the cursor position.
                // NB this assumes that the cursor was not wrapped as part of the resize.
                let cursor_delta = cursor_pos.y as i32 - last_known_cursor_pos.y as i32;

                let new_viewport_area = terminal.viewport_area.offset(Offset {
                    x: 0,
                    y: cursor_delta,
                });
                terminal.set_viewport_area(new_viewport_area);
                terminal.clear()?;
            }
        }

        let size = terminal.size()?;
        let desired_height = match &self.app_state {
            AppState::Chat { widget } => widget.desired_height(size.width),
            AppState::Onboarding { .. } => size.height,
        };

        let mut area = terminal.viewport_area;
        area.height = desired_height.min(size.height);
        area.width = size.width;
        if area.bottom() > size.height {
            terminal
                .backend_mut()
                .scroll_region_up(0..area.top(), area.bottom() - size.height)?;
            area.y = size.height - area.height;
        }
        if area != terminal.viewport_area {
            terminal.clear()?;
            terminal.set_viewport_area(area);
        }
        if !self.pending_history_lines.is_empty() {
            crate::insert_history::insert_history_lines(
                terminal,
                self.pending_history_lines.clone(),
            );
            self.pending_history_lines.clear();
        }
        terminal.draw(|frame| match &mut self.app_state {
            AppState::Chat { widget } => {
                if let Some((x, y)) = widget.cursor_pos(frame.area()) {
                    frame.set_cursor_position((x, y));
                }
                frame.render_widget_ref(&**widget, frame.area())
            }
            AppState::Onboarding { screen } => frame.render_widget_ref(&*screen, frame.area()),
        })?;
        Ok(())
    }

    /// Dispatch a KeyEvent to the current view and let it decide what to do
    /// with it.
    fn dispatch_key_event(&mut self, key_event: KeyEvent) {
        match &mut self.app_state {
            AppState::Chat { widget } => {
                widget.handle_key_event(key_event);
            }
            AppState::Onboarding { screen } => match key_event.code {
                KeyCode::Char('q') => {
                    self.app_event_tx.send(AppEvent::ExitRequest);
                }
                _ => screen.handle_key_event(key_event),
            },
        }
    }

    fn dispatch_paste_event(&mut self, pasted: String) {
        match &mut self.app_state {
            AppState::Chat { widget } => widget.handle_paste(pasted),
            AppState::Onboarding { .. } => {}
        }
    }

    fn dispatch_codex_event(&mut self, event: Event) {
        match &mut self.app_state {
            AppState::Chat { widget } => widget.handle_codex_event(event),
            AppState::Onboarding { .. } => {}
        }
    }
}
