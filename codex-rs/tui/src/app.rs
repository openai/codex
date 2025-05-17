use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::chatwidget::ChatWidget;
use crate::git_warning_screen::GitWarningOutcome;
use crate::git_warning_screen::GitWarningScreen;
use crate::mouse_capture::MouseCapture;
use crate::scroll_event_helper::ScrollEventHelper;
use crate::slash_command::SlashCommand;
use crate::tui;
use codex_core::config::Config;
use codex_core::protocol::Event;
use codex_core::protocol::Op;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::channel;

/// Top‑level application state – which full‑screen view is currently active.
enum AppState {
    /// The main chat UI is visible.
    Chat,
    /// The start‑up warning that recommends running codex inside a Git repo.
    GitWarning { screen: GitWarningScreen },
}

pub(crate) struct App<'a> {
    app_event_tx: AppEventSender,
    app_event_rx: Receiver<AppEvent>,
    chat_widget: ChatWidget<'a>,
    app_state: AppState,
}

impl App<'_> {
    pub(crate) fn new(
        config: Config,
        initial_prompt: Option<String>,
        show_git_warning: bool,
        initial_images: Vec<std::path::PathBuf>,
    ) -> Self {
        let (app_event_tx, app_event_rx) = channel();
        let app_event_tx = AppEventSender::new(app_event_tx);
        let scroll_event_helper = ScrollEventHelper::new(app_event_tx.clone());

        // Spawn a dedicated thread for reading the crossterm event loop and
        // re-publishing the events as AppEvents, as appropriate.
        {
            let app_event_tx = app_event_tx.clone();
            std::thread::spawn(move || {
                while let Ok(event) = crossterm::event::read() {
                    match event {
                        crossterm::event::Event::Key(key_event) => {
                            app_event_tx.send(AppEvent::KeyEvent(key_event));
                        }
                        crossterm::event::Event::Resize(_, _) => {
                            app_event_tx.send(AppEvent::Redraw);
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
                            use crossterm::event::KeyModifiers;

                            for ch in pasted.chars() {
                                let key_event = match ch {
                                    '\n' | '\r' => {
                                        // Represent newline as <Shift+Enter> so that the bottom
                                        // pane treats it as a literal newline instead of a submit
                                        // action (submission is only triggered on Enter *without*
                                        // any modifiers).
                                        KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT)
                                    }
                                    _ => KeyEvent::new(KeyCode::Char(ch), KeyModifiers::empty()),
                                };
                                app_event_tx.send(AppEvent::KeyEvent(key_event));
                            }
                        }
                        _ => {
                            // Ignore any other events.
                        }
                    }
                }
            });
        }

        let chat_widget = ChatWidget::new(
            config,
            app_event_tx.clone(),
            initial_prompt.clone(),
            initial_images,
        );

        let app_state = if show_git_warning {
            AppState::GitWarning {
                screen: GitWarningScreen::new(),
            }
        } else {
            AppState::Chat
        };

        Self {
            app_event_tx,
            app_event_rx,
            chat_widget,
            app_state,
        }
    }

    /// Clone of the internal event sender so external tasks (e.g. log bridge)
    /// can inject `AppEvent`s.
    pub fn event_sender(&self) -> AppEventSender {
        self.app_event_tx.clone()
    }

    pub(crate) fn run(
        &mut self,
        terminal: &mut tui::Tui,
        mouse_capture: &mut MouseCapture,
    ) -> Result<()> {
        // Insert an event to trigger the first render.
        let app_event_tx = self.app_event_tx.clone();
        app_event_tx.send(AppEvent::Redraw);

        while let Ok(event) = self.app_event_rx.recv() {
            match event {
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
                            self.chat_widget.submit_op(Op::Interrupt);
                        }
                        KeyEvent {
                            code: KeyCode::Char('d'),
                            modifiers: crossterm::event::KeyModifiers::CONTROL,
                            ..
                        } => {
                            self.app_event_tx.send(AppEvent::ExitRequest);
                        }
                        _ => {
                            self.dispatch_key_event(key_event);
                        }
                    };
                }
                AppEvent::Scroll(scroll_delta) => {
                    self.dispatch_scroll_event(scroll_delta);
                }
                AppEvent::CodexEvent(event) => {
                    self.dispatch_codex_event(event);
                }
                AppEvent::ExitRequest => {
                    break;
                }
                AppEvent::CodexOp(op) => {
                    if matches!(self.app_state, AppState::Chat) {
                        self.chat_widget.submit_op(op);
                    }
                }
                AppEvent::LatestLog(line) => {
                    if matches!(self.app_state, AppState::Chat) {
                        self.chat_widget.update_latest_log(line);
                    }
                }
                AppEvent::DispatchCommand(command) => match command {
                    SlashCommand::Clear => {
                        self.chat_widget.clear_conversation_history();
                    }
                    SlashCommand::ToggleMouseMode => {
                        if let Err(e) = mouse_capture.toggle() {
                            tracing::error!("Failed to toggle mouse mode: {e}");
                        }
                    }
                    SlashCommand::Quit => {
                        break;
                    }
                },
            }
        }
        terminal.clear()?;

        Ok(())
    }

    fn draw_next_frame(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        match &mut self.app_state {
            AppState::Chat => {
                terminal.draw(|frame| frame.render_widget_ref(&self.chat_widget, frame.area()))?;
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
            AppState::Chat => {
                self.chat_widget.handle_key_event(key_event);
            }
            AppState::GitWarning { screen } => match screen.handle_key_event(key_event) {
                GitWarningOutcome::Continue => {
                    self.app_state = AppState::Chat;
                    self.app_event_tx.send(AppEvent::Redraw);
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

    fn dispatch_scroll_event(&mut self, scroll_delta: i32) {
        if matches!(self.app_state, AppState::Chat) {
            self.chat_widget.handle_scroll_delta(scroll_delta);
        }
    }

    fn dispatch_codex_event(&mut self, event: Event) {
        if matches!(self.app_state, AppState::Chat) {
            self.chat_widget.handle_codex_event(event);
        }
    }
}
