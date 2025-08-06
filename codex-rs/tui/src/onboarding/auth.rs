use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;

use codex_login::AuthMode;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::colors::LIGHT_BLUE;
use crate::colors::SUCCESS_GREEN;
use crate::onboarding::onboarding_screen::KeyEventResult;
use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::shimmer::FrameTicker;
use crate::shimmer::shimmer_spans;
use std::path::PathBuf;
// no additional imports

pub(crate) enum SignInState {
    PickMode,
    ChatGptContinueInBrowser(#[allow(dead_code)] ContinueInBrowserState),
    ChatGptSuccess,
}

pub(crate) struct ContinueInBrowserState {
    login_child: Option<codex_login::SpawnedLogin>,
    // Kept so it gets dropped and cleaned up when the state is dropped.
    _frame_ticker: Option<FrameTicker>,
}

impl Drop for ContinueInBrowserState {
    fn drop(&mut self) {
        if let Some(child) = &self.login_child {
            if let Ok(mut locked) = child.child.lock() {
                // Best-effort terminate and reap the child to avoid zombies.
                let _ = locked.kill();
                let _ = locked.wait();
            }
        }
    }
}

pub(crate) struct AuthModeState {
    pub mode: AuthMode,
    pub error: Option<String>,
    pub sign_in_state: SignInState,
    pub event_tx: AppEventSender,
    pub codex_home: PathBuf,
}

impl AuthModeState {
    fn start_chatgpt_login(&mut self) -> KeyEventResult {
        self.error = None;
        if matches!(self.sign_in_state, SignInState::ChatGptContinueInBrowser(_)) {
            return KeyEventResult::None;
        }

        match codex_login::spawn_login_with_chatgpt(&self.codex_home) {
            Ok(child) => {
                self.sign_in_state =
                    SignInState::ChatGptContinueInBrowser(ContinueInBrowserState {
                        login_child: Some(child.clone()),
                        _frame_ticker: Some(FrameTicker::new(self.event_tx.clone())),
                    });
                self.event_tx.send(AppEvent::RequestRedraw);

                self.spawn_completion_poller(child);
                KeyEventResult::None
            }
            Err(e) => {
                self.sign_in_state = SignInState::PickMode;
                self.error = Some(e.to_string());
                self.event_tx.send(AppEvent::RequestRedraw);
                KeyEventResult::None
            }
        }
    }

    /// TODO: Read/write from the correct hierarchy config overrides + auth json + OPENAI_API_KEY.
    fn verify_api_key(&mut self) -> KeyEventResult {
        if std::env::var("OPENAI_API_KEY").is_err() {
            self.error = Some("OPENAI_API_KEY is not set".to_string());
            self.event_tx.send(AppEvent::RequestRedraw);
            KeyEventResult::None
        } else {
            KeyEventResult::Continue
        }
    }

    fn spawn_completion_poller(&self, child: codex_login::SpawnedLogin) {
        let child_arc = child.child.clone();
        let stderr_buf = child.stderr.clone();
        let event_tx = self.event_tx.clone();
        std::thread::spawn(move || {
            loop {
                let done = {
                    if let Ok(mut locked) = child_arc.lock() {
                        match locked.try_wait() {
                            Ok(Some(status)) => Some(status.success()),
                            Ok(None) => None,
                            Err(_) => Some(false),
                        }
                    } else {
                        Some(false)
                    }
                };
                if let Some(success) = done {
                    if success {
                        event_tx.send(AppEvent::OnboardingAuthComplete(Ok(())));
                    } else {
                        let err = stderr_buf
                            .lock()
                            .ok()
                            .and_then(|b| String::from_utf8(b.clone()).ok())
                            .unwrap_or_else(|| "login_with_chatgpt subprocess failed".to_string());
                        event_tx.send(AppEvent::OnboardingAuthComplete(Err(err)));
                    }
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        });
    }

    fn current_frame(&self) -> usize {
        // Derive frame index from wall-clock time to avoid storing animation state.
        // 100ms per frame to match the previous ticker cadence.
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        (now_ms / 100) as usize
    }
}

impl KeyboardHandler for AuthModeState {
    fn handle_key_event(&mut self, key_event: KeyEvent) -> KeyEventResult {
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.mode = AuthMode::ChatGPT;
                KeyEventResult::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.mode = AuthMode::ApiKey;
                KeyEventResult::None
            }
            KeyCode::Char('1') => {
                self.mode = AuthMode::ChatGPT;
                self.start_chatgpt_login();
                KeyEventResult::None
            }
            KeyCode::Char('2') => {
                self.mode = AuthMode::ApiKey;
                self.verify_api_key()
            }
            KeyCode::Enter => match self.mode {
                AuthMode::ChatGPT => match self.sign_in_state {
                    SignInState::PickMode => self.start_chatgpt_login(),
                    SignInState::ChatGptContinueInBrowser(_) => KeyEventResult::None,
                    SignInState::ChatGptSuccess => KeyEventResult::Continue,
                },
                AuthMode::ApiKey => self.verify_api_key(),
            },
            KeyCode::Esc => {
                if matches!(self.sign_in_state, SignInState::ChatGptContinueInBrowser(_)) {
                    self.sign_in_state = SignInState::PickMode;
                    self.event_tx.send(AppEvent::RequestRedraw);
                    KeyEventResult::None
                } else {
                    KeyEventResult::Quit
                }
            }
            KeyCode::Char('q') => KeyEventResult::Quit,
            _ => KeyEventResult::None,
        }
    }
}
pub(crate) struct AuthModeWidget<'a> {
    pub state: &'a AuthModeState,
}

impl<'a> AuthModeWidget<'a> {
    fn render_pick_mode(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::raw("> "),
                Span::styled(
                    "Sign in with your ChatGPT account?",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
        ];

        let create_mode_item = |idx: usize, selected_mode: AuthMode, text: &str| {
            let is_selected = self.state.mode == selected_mode;
            let caret = if is_selected { ">" } else { " " };
            let content = format!("{} {}. {}", caret, idx + 1, text);
            if is_selected {
                Line::styled(content, Style::default().fg(LIGHT_BLUE))
            } else {
                Line::from(content)
            }
        };

        lines.push(create_mode_item(
            0,
            AuthMode::ChatGPT,
            "Yes, sign in with ChatGPT or create a new account",
        ));
        lines.push(create_mode_item(
            1,
            AuthMode::ApiKey,
            "No, connect my API key (usage-based pricing applies)",
        ));
        lines.push(Line::from(""));
        lines.push(
            Line::from("Press Enter to continue")
                .style(Style::default().add_modifier(Modifier::DIM)),
        );
        if let Some(err) = &self.state.error {
            lines.push(Line::from(Span::styled(
                err.as_str(),
                Style::default().fg(Color::Red),
            )));
        }

        Paragraph::new(lines).render(area, buf);
    }

    fn render_chatgpt_success(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![Line::from(Span::styled(
            "✓ Login with ChatGPT successful.",
            Style::default().fg(SUCCESS_GREEN),
        ))];

        Paragraph::new(lines).render(area, buf);
    }

    fn render_continue_in_browser(&self, area: Rect, buf: &mut Buffer) {
        let idx = self.state.current_frame();
        let lines = vec![
            Line::from(shimmer_spans("Opening browser to sign in", idx)),
            Line::from(""),
            Line::from("Press Escape to cancel")
                .style(Style::default().add_modifier(Modifier::DIM)),
        ];
        Paragraph::new(lines).render(area, buf);
    }
}

impl WidgetRef for AuthModeWidget<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        match self.state.sign_in_state {
            SignInState::PickMode => {
                self.render_pick_mode(area, buf);
            }
            SignInState::ChatGptContinueInBrowser(_) => {
                self.render_continue_in_browser(area, buf);
            }
            SignInState::ChatGptSuccess => {
                self.render_chatgpt_success(area, buf);
            }
        }
    }
}
