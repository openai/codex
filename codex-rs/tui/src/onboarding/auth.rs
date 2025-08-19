use codex_login::CLIENT_ID;
use codex_login::ServerOptions;
use codex_login::ShutdownHandle;
use codex_login::run_login_server;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use codex_login::AuthMode;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
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
    auth_url: String,
    shutdown_handle: Option<ShutdownHandle>,
    _login_wait_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for ContinueInBrowserState {
    fn drop(&mut self) {
        if let Some(flag) = &self.shutdown_handle {
            flag.shutdown();
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
                    format!("{} {}. ", caret, idx + 1).cyan().dim(),
                    text.to_string().cyan(),
                ])
            } else {
                Line::from(format!("  {}. {text}", idx + 1))
            };

            let line2 = if is_selected {
                Line::from(format!("     {description}"))
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::DIM)
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
            // AE: Following styles.md, this should probably be Cyan because it's a user input tip.
            //     But leaving this for a future cleanup.
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
            if !state.auth_url.is_empty() {
                lines.push(Line::from("  If the link doesn't open automatically, open the following link to authenticate:"));
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    state.auth_url.as_str().cyan().underlined(),
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
            Line::from("✓ Signed in with your ChatGPT account").fg(Color::Green),
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
            Line::from("  Codex can make mistakes"),
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
            Line::from("  Press Enter to continue").fg(Color::Cyan),
        ];

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_chatgpt_success(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![Line::from("✓ Signed in with your ChatGPT account").fg(Color::Green)];

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_env_var_found(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![Line::from("✓ Using OPENAI_API_KEY").fg(Color::Green)];

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_env_var_missing(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            Line::from(
                "  To use Codex with the OpenAI API, set OPENAI_API_KEY in your environment",
            )
            .style(Style::default().fg(Color::Cyan)),
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
        let opts = ServerOptions::new(self.codex_home.clone(), CLIENT_ID.to_string());
        let server = run_login_server(opts, None);
        match server {
            Ok(child) => {
                let auth_url = child.auth_url.clone();
                let shutdown_handle = child.cancel_handle();

                let event_tx = self.event_tx.clone();
                let join_handle = tokio::spawn(async move {
                    spawn_completion_poller(child, event_tx).await;
                });
                self.sign_in_state =
                    SignInState::ChatGptContinueInBrowser(ContinueInBrowserState {
                        auth_url,
                        shutdown_handle: Some(shutdown_handle),
                        _login_wait_handle: Some(join_handle),
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
}

async fn spawn_completion_poller(
    child: codex_login::LoginServer,
    event_tx: AppEventSender,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if let Ok(()) = child.block_until_done().await {
            event_tx.send(AppEvent::OnboardingAuthComplete(Ok(())));
        } else {
            event_tx.send(AppEvent::OnboardingAuthComplete(Err(
                "login failed".to_string()
            )));
        }
    })
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
