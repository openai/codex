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
use ratatui::widgets::Wrap;

use codex_login::AuthMode;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::colors::LIGHT_BLUE;
use crate::colors::SUCCESS_GREEN;
use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;
use crate::shimmer::shimmer_spans;
use std::path::PathBuf;

use super::onboarding_screen::StepState;
// no additional imports

#[derive(Debug)]
pub(crate) enum SignInState {
    PickMode,
    ChatGptContinueInBrowser(ContinueInBrowserState),
    ChatGptSuccessMessage,
    ChatGptSuccess,
    EnvVarMissing,
    EnvVarFound,
}

#[derive(Debug)]
/// Used to manage the lifecycle of SpawnedLogin and ensure it gets cleaned up.
pub(crate) struct ContinueInBrowserState {
    login_child: Option<codex_login::SpawnedLogin>,
}
impl Drop for ContinueInBrowserState {
    fn drop(&mut self) {
        if let Some(child) = &self.login_child {
            child.cancel();
        }
    }
}

impl KeyboardHandler for AuthModeWidget {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.highlighted_mode = AuthMode::ChatGPT;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.highlighted_mode = AuthMode::ApiKey;
            }
            KeyCode::Char('1') => {
                self.start_chatgpt_login();
            }
            KeyCode::Char('2') => self.verify_api_key(),
            KeyCode::Enter => match self.sign_in_state {
                SignInState::PickMode => match self.highlighted_mode {
                    AuthMode::ChatGPT => self.start_chatgpt_login(),
                    AuthMode::ApiKey => self.verify_api_key(),
                },
                SignInState::EnvVarMissing => self.sign_in_state = SignInState::PickMode,
                SignInState::ChatGptSuccessMessage => {
                    self.sign_in_state = SignInState::ChatGptSuccess
                }
                _ => {}
            },
            KeyCode::Esc => {
                if matches!(self.sign_in_state, SignInState::ChatGptContinueInBrowser(_)) {
                    self.sign_in_state = SignInState::PickMode;
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug)]
pub(crate) struct AuthModeWidget {
    pub event_tx: AppEventSender,
    pub highlighted_mode: AuthMode,
    pub error: Option<String>,
    pub sign_in_state: SignInState,
    pub codex_home: PathBuf,
}

impl AuthModeWidget {
    fn render_pick_mode(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::raw("> "),
                Span::styled(
                    "Sign in with ChatGPT to use Codex as part of your paid plan",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "or connect an API key for usage-based billing",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
        ];

        let create_mode_item = |idx: usize,
                                selected_mode: AuthMode,
                                text: &str,
                                description: &str|
         -> Vec<Line<'static>> {
            let is_selected = self.highlighted_mode == selected_mode;
            let caret = if is_selected { ">" } else { " " };

            let line1 = if is_selected {
                Line::from(vec![
                    Span::styled(
                        format!("{} {}. ", caret, idx + 1),
                        Style::default().fg(LIGHT_BLUE).add_modifier(Modifier::DIM),
                    ),
                    Span::styled(text.to_owned(), Style::default().fg(LIGHT_BLUE)),
                ])
            } else {
                Line::from(format!("  {}. {text}", idx + 1))
            };

            let line2 = if is_selected {
                Line::from(format!("     {description}"))
                    .style(Style::default().fg(LIGHT_BLUE).add_modifier(Modifier::DIM))
            } else {
                Line::from(format!("     {description}"))
                    .style(Style::default().add_modifier(Modifier::DIM))
            };

            vec![line1, line2]
        };

        lines.extend(create_mode_item(
            0,
            AuthMode::ChatGPT,
            "Sign in with ChatGPT",
            "Usage included with Plus, Pro, and Team plans",
        ));
        lines.extend(create_mode_item(
            1,
            AuthMode::ApiKey,
            "Provide your own API key",
            "Pay for what you use",
        ));
        lines.push(Line::from(""));
        lines.push(
            Line::from("  Press Enter to continue")
                .style(Style::default().add_modifier(Modifier::DIM)),
        );
        if let Some(err) = &self.error {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                err.as_str(),
                Style::default().fg(Color::Red),
            )));
        }

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_continue_in_browser(&self, area: Rect, buf: &mut Buffer) {
        let mut spans = vec![Span::from("> ")];
        // Schedule a follow-up frame to keep the shimmer animation going.
        self.event_tx
            .send(AppEvent::ScheduleFrameIn(std::time::Duration::from_millis(
                100,
            )));
        spans.extend(shimmer_spans("Finish signing in via your browser"));
        let mut lines = vec![Line::from(spans), Line::from("")];

        if let SignInState::ChatGptContinueInBrowser(state) = &self.sign_in_state {
            if let Some(url) = state
                .login_child
                .as_ref()
                .and_then(|child| child.get_login_url())
            {
                lines.push(Line::from("  If the link doesn't open automatically, open the following link to authenticate:"));
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        url,
                        Style::default()
                            .fg(LIGHT_BLUE)
                            .add_modifier(Modifier::UNDERLINED),
                    ),
                ]));
                lines.push(Line::from(""));
            }
        }

        lines.push(
            Line::from("  Press Esc to cancel").style(Style::default().add_modifier(Modifier::DIM)),
        );
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_chatgpt_success_message(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            Line::from("✓ Signed in with your ChatGPT account")
                .style(Style::default().fg(SUCCESS_GREEN)),
            Line::from(""),
            Line::from("> Before you start:"),
            Line::from(""),
            Line::from("  Decide how much autonomy you want to grant Codex"),
            Line::from(vec![
                Span::raw("  For more details see the "),
                Span::styled(
                    "\u{1b}]8;;https://github.com/openai/codex\u{7}Codex docs\u{1b}]8;;\u{7}",
                    Style::default().add_modifier(Modifier::UNDERLINED),
                ),
            ])
            .style(Style::default().add_modifier(Modifier::DIM)),
            Line::from(""),
            Line::from("  Codex can make mistakes")
                .style(Style::default().fg(Color::White)),
            Line::from("  Review the code it writes and commands it runs")
                .style(Style::default().add_modifier(Modifier::DIM)),
            Line::from(""),
            Line::from("  Powered by your ChatGPT account"),
            Line::from(vec![
                Span::raw("  Uses your plan's rate limits and "),
                Span::styled(
                    "\u{1b}]8;;https://chatgpt.com/#settings\u{7}training data preferences\u{1b}]8;;\u{7}",
                    Style::default().add_modifier(Modifier::UNDERLINED),
                ),
            ])
            .style(Style::default().add_modifier(Modifier::DIM)),
            Line::from(""),
            Line::from("  Press Enter to continue").style(Style::default().fg(LIGHT_BLUE)),
        ];

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_chatgpt_success(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            Line::from("✓ Signed in with your ChatGPT account")
                .style(Style::default().fg(SUCCESS_GREEN)),
        ];

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_env_var_found(&self, area: Rect, buf: &mut Buffer) {
        let lines =
            vec![Line::from("✓ Using OPENAI_API_KEY").style(Style::default().fg(SUCCESS_GREEN))];

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_env_var_missing(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            Line::from(
                "  To use Codex with the OpenAI API, set OPENAI_API_KEY in your environment",
            )
            .style(Style::default().fg(Color::Blue)),
            Line::from(""),
            Line::from("  Press Enter to return")
                .style(Style::default().add_modifier(Modifier::DIM)),
        ];

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn start_chatgpt_login(&mut self) {
        self.error = None;
        match codex_login::spawn_login_with_chatgpt(&self.codex_home) {
            Ok(child) => {
                self.spawn_completion_poller(child.clone());
                self.sign_in_state =
                    SignInState::ChatGptContinueInBrowser(ContinueInBrowserState {
                        login_child: Some(child),
                    });
                self.event_tx.send(AppEvent::RequestRedraw);
            }
            Err(e) => {
                self.sign_in_state = SignInState::PickMode;
                self.error = Some(e.to_string());
                self.event_tx.send(AppEvent::RequestRedraw);
            }
        }
    }

    /// TODO: Read/write from the correct hierarchy config overrides + auth json + OPENAI_API_KEY.
    fn verify_api_key(&mut self) {
        if std::env::var("OPENAI_API_KEY").is_err() {
            self.sign_in_state = SignInState::EnvVarMissing;
        } else {
            self.sign_in_state = SignInState::EnvVarFound;
        }
        self.event_tx.send(AppEvent::RequestRedraw);
    }

    fn spawn_completion_poller(&self, child: codex_login::SpawnedLogin) {
        let event_tx = self.event_tx.clone();
        std::thread::spawn(move || {
            loop {
                if let Some(success) = child.get_auth_result() {
                    if success {
                        event_tx.send(AppEvent::OnboardingAuthComplete(Ok(())));
                    } else {
                        event_tx.send(AppEvent::OnboardingAuthComplete(Err(
                            "login failed".to_string()
                        )));
                    }
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        });
    }
}

impl StepStateProvider for AuthModeWidget {
    fn get_step_state(&self) -> StepState {
        match &self.sign_in_state {
            SignInState::PickMode
            | SignInState::EnvVarMissing
            | SignInState::ChatGptContinueInBrowser(_)
            | SignInState::ChatGptSuccessMessage => StepState::InProgress,
            SignInState::ChatGptSuccess | SignInState::EnvVarFound => StepState::Complete,
        }
    }
}

impl WidgetRef for AuthModeWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        match self.sign_in_state {
            SignInState::PickMode => {
                self.render_pick_mode(area, buf);
            }
            SignInState::ChatGptContinueInBrowser(_) => {
                self.render_continue_in_browser(area, buf);
            }
            SignInState::ChatGptSuccessMessage => {
                self.render_chatgpt_success_message(area, buf);
            }
            SignInState::ChatGptSuccess => {
                self.render_chatgpt_success(area, buf);
            }
            SignInState::EnvVarMissing => {
                self.render_env_var_missing(area, buf);
            }
            SignInState::EnvVarFound => {
                self.render_env_var_found(area, buf);
            }
        }
    }
}
