use crate::app_event::AppEvent;
use crate::chatwidget::ChatWidget;
use crate::git_warning_screen::GitWarningOutcome;
use crate::git_warning_screen::GitWarningScreen;
use crate::scroll_event_helper::ScrollEventHelper;
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
use std::sync::mpsc::Sender;
use std::sync::mpsc::channel;

/// Top‑level application state – which full‑screen view is currently active.
enum AppState {
    /// The main chat UI is visible.
    Chat,
    /// The start‑up warning that recommends running codex inside a Git repo.
    GitWarning { screen: GitWarningScreen },
}

pub(crate) struct App<'a> {
    app_event_tx: Sender<AppEvent>,
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
        let scroll_event_helper = ScrollEventHelper::new(app_event_tx.clone());

        // Spawn a dedicated thread for reading the crossterm event loop and
        // re-publishing the events as AppEvents, as appropriate.
        {
            let app_event_tx = app_event_tx.clone();
            std::thread::spawn(move || {
                while let Ok(event) = crossterm::event::read() {
                    let app_event = match event {
                        crossterm::event::Event::Key(key_event) => AppEvent::KeyEvent(key_event),
                        crossterm::event::Event::Resize(_, _) => AppEvent::Redraw,
                        crossterm::event::Event::Mouse(MouseEvent {
                            kind: MouseEventKind::ScrollUp,
                            ..
                        }) => {
                            scroll_event_helper.scroll_up();
                            continue;
                        }
                        crossterm::event::Event::Mouse(MouseEvent {
                            kind: MouseEventKind::ScrollDown,
                            ..
                        }) => {
                            scroll_event_helper.scroll_down();
                            continue;
                        }
                        _ => {
                            continue;
                        }
                    };
                    if let Err(e) = app_event_tx.send(app_event) {
                        tracing::error!("failed to send event: {e}");
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
    pub fn event_sender(&self) -> Sender<AppEvent> {
        self.app_event_tx.clone()
    }

    pub(crate) fn run(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        // Insert an event to trigger the first render.
        let app_event_tx = self.app_event_tx.clone();
        app_event_tx.send(AppEvent::Redraw).unwrap();

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
                            self.app_event_tx.send(AppEvent::ExitRequest).unwrap();
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
                        let _ = self.chat_widget.update_latest_log(line);
                    }
                }
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
                if let Err(e) = self.chat_widget.handle_key_event(key_event) {
                    tracing::error!("SendError: {e}");
                }
            }
            AppState::GitWarning { screen } => match screen.handle_key_event(key_event) {
                GitWarningOutcome::Continue => {
                    self.app_state = AppState::Chat;
                    let _ = self.app_event_tx.send(AppEvent::Redraw);
                }
                GitWarningOutcome::Quit => {
                    let _ = self.app_event_tx.send(AppEvent::ExitRequest);
                }
                GitWarningOutcome::None => {
                    // do nothing
                }
            },
        }
    }

    fn dispatch_scroll_event(&mut self, scroll_delta: i32) {
        if matches!(self.app_state, AppState::Chat) {
            if let Err(e) = self.chat_widget.handle_scroll_delta(scroll_delta) {
                tracing::error!("SendError: {e}");
            }
        }
    }

    fn dispatch_codex_event(&mut self, event: Event) {
        if matches!(self.app_state, AppState::Chat) {
            if let Err(e) = self.chat_widget.handle_codex_event(event) {
                tracing::error!("SendError: {e}");
            }
        }
    }
}
