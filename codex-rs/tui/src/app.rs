use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::chatwidget::ChatWidget;
use crate::file_search::FileSearchManager;
use crate::get_git_diff::get_git_diff;
use crate::git_warning_screen::GitWarningOutcome;
use crate::git_warning_screen::GitWarningScreen;
use crate::login_screen::LoginScreen;
use crate::scroll_event_helper::ScrollEventHelper;
use crate::slash_command::SlashCommand;
use crate::tui;
use codex_core::config::Config;
use codex_core::protocol::Event;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

/// Time window for debouncing redraw requests.
const REDRAW_DEBOUNCE: Duration = Duration::from_millis(10);

/// Top-level application state: which full-screen view is currently active.
#[allow(clippy::large_enum_variant)]
enum AppState<'a> {
    /// The main chat UI is visible.
    Chat {
        /// Boxed to avoid a large enum variant and reduce the overall size of
        /// `AppState`.
        widget: Box<ChatWidget<'a>>,
    },
    /// The login screen for the OpenAI provider.
    Login { screen: LoginScreen },
    /// The start-up warning that recommends running codex inside a Git repo.
    GitWarning { screen: GitWarningScreen },
}

pub(crate) struct App<'a> {
    app_event_tx: AppEventSender,
    app_event_rx: Receiver<AppEvent>,
    app_state: AppState<'a>,

    /// Config is stored here so we can recreate ChatWidgets as needed.
    config: Config,

    file_search: FileSearchManager,

    /// True when a redraw has been scheduled but not yet executed.
    pending_redraw: Arc<AtomicBool>,

    /// Stored parameters needed to instantiate the ChatWidget later, e.g.,
    /// after dismissing the Git-repo warning.
    chat_args: Option<ChatWidgetArgs>,
}

/// Aggregate parameters needed to create a `ChatWidget`, as creation may be
/// deferred until after the Git warning screen is dismissed.
#[derive(Clone)]
struct ChatWidgetArgs {
    config: Config,
    initial_prompt: Option<String>,
    initial_images: Vec<PathBuf>,
}

impl App<'_> {
    pub(crate) fn new(
        config: Config,
        initial_prompt: Option<String>,
        show_login_screen: bool,
        show_git_warning: bool,
        initial_images: Vec<std::path::PathBuf>,
    ) -> Self {
        let (app_event_tx, app_event_rx) = channel();
        let app_event_tx = AppEventSender::new(app_event_tx);
        let pending_redraw = Arc::new(AtomicBool::new(false));
        let scroll_event_helper = ScrollEventHelper::new(app_event_tx.clone());

        // Spawn a dedicated thread for reading the crossterm event loop and
        // re-publishing the events as AppEvents, as appropriate.
        {
            let app_event_tx = app_event_tx.clone();
            std::thread::spawn(move || {
                loop {
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
                                crossterm::event::Event::Mouse(MouseEvent {
                                    kind: MouseEventKind::ScrollUp,
                                    ..
                                }) => {
                                    scroll_event_helper.scroll_up();
                                }
                                crossterm::event::Event::Mouse(MouseEvent {
                                    kind: MouseEventKind::ScrollDown,
                                    ..
                                }) => {
                                    scroll_event_helper.scroll_down();
                                }
                                crossterm::event::Event::Paste(pasted) => {
                                    // Many terminals convert newlines to \r when
                                    // pasting, e.g. [iTerm2][]. But [tui-textarea
                                    // expects \n][tui-textarea]. This seems like a bug
                                    // in tui-textarea IMO, but work around it for now.
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

        let (app_state, chat_args) = if show_login_screen {
            (
                AppState::Login {
                    screen: LoginScreen::new(app_event_tx.clone(), config.codex_home.clone()),
                },
                Some(ChatWidgetArgs {
                    config: config.clone(),
                    initial_prompt,
                    initial_images,
                }),
            )
        } else if show_git_warning {
            (
                AppState::GitWarning {
                    screen: GitWarningScreen::new(),
                },
                Some(ChatWidgetArgs {
                    config: config.clone(),
                    initial_prompt,
                    initial_images,
                }),
            )
        } else {
            let chat_widget = ChatWidget::new(
                config.clone(),
                app_event_tx.clone(),
                initial_prompt,
                initial_images,
            );
            (
                AppState::Chat {
                    widget: Box::new(chat_widget),
                },
                None,
            )
        };

        let file_search = FileSearchManager::new(config.cwd.clone(), app_event_tx.clone());
        Self {
            app_event_tx,
            app_event_rx,
            app_state,
            config,
            file_search,
            pending_redraw,
            chat_args,
        }
    }

    /// Clone of the internal event sender so external tasks (e.g. log bridge)
    /// can inject `AppEvent`s.
    pub fn event_sender(&self) -> AppEventSender {
        self.app_event_tx.clone()
    }

    /// Schedule a redraw if one is not already pending.
    #[allow(clippy::unwrap_used)]
    fn schedule_redraw(&self) {
        // Attempt to set the flag to `true`. If it was already `true`, another
        // redraw is already pending so we can return early.
        if self
            .pending_redraw
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let tx = self.app_event_tx.clone();
        let pending_redraw = self.pending_redraw.clone();
        thread::spawn(move || {
            thread::sleep(REDRAW_DEBOUNCE);
            tx.send(AppEvent::Redraw);
            pending_redraw.store(false, Ordering::SeqCst);
        });
    }

    pub(crate) fn run(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        // Insert an event to trigger the first render.
        let app_event_tx = self.app_event_tx.clone();
        app_event_tx.send(AppEvent::RequestRedraw);

        while let Ok(event) = self.app_event_rx.recv() {
            match event {
                AppEvent::InsertHistory(lines) => {
                    crate::insert_history::insert_history_lines(terminal, lines);
                    self.app_event_tx.send(AppEvent::RequestRedraw);
                }
                AppEvent::RequestRedraw => {
                    self.schedule_redraw();
                }
                AppEvent::Redraw => {
                    self.draw_next_frame(terminal)?;
                }
                AppEvent::KeyEvent(key_event) => {
                    match key_event {
                        KeyEvent {
                            code: KeyCode::Char('c'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
                            ..
                        } => {
                            match &mut self.app_state {
                                AppState::Chat { widget } => {
                                    widget.on_ctrl_c();
                                }
                                AppState::Login { .. } | AppState::GitWarning { .. } => {
                                    // No-op.
                                }
                            }
                        }
                        KeyEvent {
                            code: KeyCode::Char('d'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
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
                                AppState::Login { .. } | AppState::GitWarning { .. } => {
                                    self.app_event_tx.send(AppEvent::ExitRequest);
                                }
                            }
                        }
                        _ => {
                            self.dispatch_key_event(key_event);
                        }
                    };
                }
                AppEvent::Scroll(scroll_delta) => {
                    self.dispatch_scroll_event(scroll_delta);
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
                    AppState::Login { .. } | AppState::GitWarning { .. } => {}
                },
                AppEvent::LatestLog(line) => match &mut self.app_state {
                    AppState::Chat { widget } => widget.update_latest_log(line),
                    AppState::Login { .. } | AppState::GitWarning { .. } => {}
                },
                AppEvent::DispatchCommand(command) => match command {
                    SlashCommand::New => {
                        let new_widget = Box::new(ChatWidget::new(
                            self.config.clone(),
                            self.app_event_tx.clone(),
                            None,
                            Vec::new(),
                        ));
                        self.app_state = AppState::Chat { widget: new_widget };
                        self.app_event_tx.send(AppEvent::RequestRedraw);
                    }
                    SlashCommand::Quit => {
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
                },
                AppEvent::StartFileSearch(query) => {
                    self.file_search.on_user_query(query);
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

    pub(crate) fn token_usage(&self) -> codex_core::protocol::TokenUsage {
        match &self.app_state {
            AppState::Chat { widget } => widget.token_usage().clone(),
            AppState::Login { .. } | AppState::GitWarning { .. } => {
                codex_core::protocol::TokenUsage::default()
            }
        }
    }

    fn draw_next_frame(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        // TODO: add a throttle to avoid redrawing too often

        match &mut self.app_state {
            AppState::Chat { widget } => {
                terminal.draw(|frame| frame.render_widget_ref(&**widget, frame.area()))?;
            }
            AppState::Login { screen } => {
                terminal.draw(|frame| frame.render_widget_ref(&*screen, frame.area()))?;
            }
            AppState::GitWarning { screen } => {
                terminal.draw(|frame| frame.render_widget_ref(&*screen, frame.area()))?;
            }
        }
        Ok(())
    }

    /// Dispatch a KeyEvent to the current view and let it decide what to do
    /// with it.
    fn dispatch_key_event(&mut self, key_event: KeyEvent) {
        match &mut self.app_state {
            AppState::Chat { widget } => {
                widget.handle_key_event(key_event);
            }
            AppState::Login { screen } => screen.handle_key_event(key_event),
            AppState::GitWarning { screen } => match screen.handle_key_event(key_event) {
                GitWarningOutcome::Continue => {
                    // User accepted – switch to chat view.
                    let args = match self.chat_args.take() {
                        Some(args) => args,
                        None => panic!("ChatWidgetArgs already consumed"),
                    };

                    let widget = Box::new(ChatWidget::new(
                        args.config,
                        self.app_event_tx.clone(),
                        args.initial_prompt,
                        args.initial_images,
                    ));
                    self.app_state = AppState::Chat { widget };
                    self.app_event_tx.send(AppEvent::RequestRedraw);
                }
                GitWarningOutcome::Quit => {
                    self.app_event_tx.send(AppEvent::ExitRequest);
                }
                GitWarningOutcome::None => {
                    // do nothing
                }
            },
        }
    }

    fn dispatch_paste_event(&mut self, pasted: String) {
        match &mut self.app_state {
            AppState::Chat { widget } => widget.handle_paste(pasted),
            AppState::Login { .. } | AppState::GitWarning { .. } => {}
        }
    }

    fn dispatch_scroll_event(&mut self, scroll_delta: i32) {
        match &mut self.app_state {
            AppState::Chat { widget } => widget.handle_scroll_delta(scroll_delta),
            AppState::Login { .. } | AppState::GitWarning { .. } => {}
        }
    }

    fn dispatch_codex_event(&mut self, event: Event) {
        match &mut self.app_state {
            AppState::Chat { widget } => widget.handle_codex_event(event),
            AppState::Login { .. } | AppState::GitWarning { .. } => {}
        }
    }
}
