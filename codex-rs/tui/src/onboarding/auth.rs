#![allow(clippy::unwrap_used)]

use codex_login::AuthManager;
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
use std::sync::RwLock;

use crate::LoginStatus;
use crate::onboarding::onboarding_screen::KeyboardHandler;
use crate::onboarding::onboarding_screen::StepStateProvider;
use crate::shimmer::shimmer_spans;
use crate::tui::FrameRequester;
use std::path::PathBuf;
use std::sync::Arc;

use super::onboarding_screen::StepState;

#[derive(Clone)]
pub(crate) enum SignInState {
    PickMode,
    ChatGptContinueInBrowser(ContinueInBrowserState),
    ChatGptSuccessMessage,
    ChatGptSuccess,
    EnvVarMissing,
    EnvVarFound,
}

#[derive(Clone)]
/// Used to manage the lifecycle of SpawnedLogin and ensure it gets cleaned up.
pub(crate) struct ContinueInBrowserState {
    auth_url: String,
    shutdown_flag: Option<ShutdownHandle>,
}

impl Drop for ContinueInBrowserState {
    fn drop(&mut self) {
        if let Some(handle) = &self.shutdown_flag {
            handle.shutdown();
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
            KeyCode::Enter => {
                let sign_in_state = { (*self.sign_in_state.read().unwrap()).clone() };
                match sign_in_state {
                    SignInState::PickMode => match self.highlighted_mode {
                        AuthMode::ChatGPT => {
                            self.start_chatgpt_login();
                        }
                        AuthMode::ApiKey => {
                            self.verify_api_key();
                        }
                    },
                    SignInState::EnvVarMissing => {
                        *self.sign_in_state.write().unwrap() = SignInState::PickMode;
                    }
                    SignInState::ChatGptSuccessMessage => {
                        *self.sign_in_state.write().unwrap() = SignInState::ChatGptSuccess;
                    }
                    _ => {}
                }
            }
            KeyCode::Esc => {
                tracing::info!("Esc pressed");
                let sign_in_state = { (*self.sign_in_state.read().unwrap()).clone() };
                if matches!(sign_in_state, SignInState::ChatGptContinueInBrowser(_)) {
                    *self.sign_in_state.write().unwrap() = SignInState::PickMode;
                    self.request_frame.schedule_frame();
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
pub(crate) struct AuthModeWidget {
    pub request_frame: FrameRequester,
    pub highlighted_mode: AuthMode,
    pub error: Option<String>,
    pub sign_in_state: Arc<RwLock<SignInState>>,
    pub codex_home: PathBuf,
    pub login_status: LoginStatus,
    pub preferred_auth_method: AuthMode,
    pub auth_manager: Arc<AuthManager>,
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

        // If the user is already authenticated but the method differs from their
        // preferred auth method, show a brief explanation.
        if let LoginStatus::AuthMode(current) = self.login_status
            && current != self.preferred_auth_method
        {
            let to_label = |mode: AuthMode| match mode {
                AuthMode::ApiKey => "API key",
                AuthMode::ChatGPT => "ChatGPT",
            };
            let msg = format!(
                "  You’re currently using {} while your preferred method is {}.",
                to_label(current),
                to_label(self.preferred_auth_method)
            );
            lines.push(Line::from(msg).style(Style::default()));
            lines.push(Line::from(""));
        }

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
        let chatgpt_label = if matches!(self.login_status, LoginStatus::AuthMode(AuthMode::ChatGPT))
        {
            "Continue using ChatGPT"
        } else {
            "Sign in with ChatGPT"
        };

        lines.extend(create_mode_item(
            0,
            AuthMode::ChatGPT,
            chatgpt_label,
            "Usage included with Plus, Pro, and Team plans",
        ));
        let api_key_label = if matches!(self.login_status, LoginStatus::AuthMode(AuthMode::ApiKey))
        {
            "Continue using API key"
        } else {
            "Provide your own API key"
        };
        lines.extend(create_mode_item(
            1,
            AuthMode::ApiKey,
            api_key_label,
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
        self.request_frame
            .schedule_frame_in(std::time::Duration::from_millis(100));
        spans.extend(shimmer_spans("Finish signing in via your browser"));
        let mut lines = vec![Line::from(spans), Line::from("")];

        let sign_in_state = self.sign_in_state.read().unwrap();
        if let SignInState::ChatGptContinueInBrowser(state) = &*sign_in_state
            && !state.auth_url.is_empty()
        {
            lines.push(Line::from("  If the link doesn't open automatically, open the following link to authenticate:"));
            lines.push(Line::from(vec![
                Span::raw("  "),
                state.auth_url.as_str().cyan().underlined(),
            ]));
            lines.push(Line::from(""));
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
        // If we're already authenticated with ChatGPT, don't start a new login –
        // just proceed to the success message flow.
        if matches!(self.login_status, LoginStatus::AuthMode(AuthMode::ChatGPT)) {
            *self.sign_in_state.write().unwrap() = SignInState::ChatGptSuccess;
            self.request_frame.schedule_frame();
            return;
        }

        self.error = None;
        let opts = ServerOptions::new(self.codex_home.clone(), CLIENT_ID.to_string());
        match run_login_server(opts) {
            Ok(child) => {
                let sign_in_state = self.sign_in_state.clone();
                let request_frame = self.request_frame.clone();
                let auth_manager = self.auth_manager.clone();
                tokio::spawn(async move {
                    let auth_url = child.auth_url.clone();
                    {
                        *sign_in_state.write().unwrap() =
                            SignInState::ChatGptContinueInBrowser(ContinueInBrowserState {
                                auth_url,
                                shutdown_flag: Some(child.cancel_handle()),
                            });
                    }
                    request_frame.schedule_frame();
                    let r = child.block_until_done().await;
                    match r {
                        Ok(()) => {
                            // Force the auth manager to reload the new auth information.
                            auth_manager.reload();

                            *sign_in_state.write().unwrap() = SignInState::ChatGptSuccessMessage;
                            request_frame.schedule_frame();
                        }
                        _ => {
                            *sign_in_state.write().unwrap() = SignInState::PickMode;
                            // self.error = Some(e.to_string());
                            request_frame.schedule_frame();
                        }
                    }
                });
            }
            Err(e) => {
                *self.sign_in_state.write().unwrap() = SignInState::PickMode;
                self.error = Some(e.to_string());
                self.request_frame.schedule_frame();
            }
        }
    }

    /// TODO: Read/write from the correct hierarchy config overrides + auth json + OPENAI_API_KEY.
    fn verify_api_key(&mut self) {
        if matches!(self.login_status, LoginStatus::AuthMode(AuthMode::ApiKey)) {
            // We already have an API key configured (e.g., from auth.json or env),
            // so mark this step complete immediately.
            *self.sign_in_state.write().unwrap() = SignInState::EnvVarFound;
        } else {
            *self.sign_in_state.write().unwrap() = SignInState::EnvVarMissing;
        }
        self.request_frame.schedule_frame();
    }
}

impl StepStateProvider for AuthModeWidget {
    fn get_step_state(&self) -> StepState {
        let sign_in_state = self.sign_in_state.read().unwrap();
        match &*sign_in_state {
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
        let sign_in_state = self.sign_in_state.read().unwrap();
        match &*sign_in_state {
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
