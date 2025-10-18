#![allow(clippy::unwrap_used)]

use codex_core::AuthManager;
use codex_core::auth::CLIENT_ID;
use codex_core::auth::login_with_api_key;
use codex_core::auth::read_openai_api_key_from_env;
use codex_login::ServerOptions;
use codex_login::ShutdownHandle;
use codex_login::run_login_server;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;

use codex_app_server_protocol::AuthMode;
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
    ApiKeyEntry(ApiKeyInputState),
    ApiKeyConfigured,
}

#[derive(Clone, Default)]
pub(crate) struct ApiKeyInputState {
    value: String,
    prepopulated_from_env: bool,
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
        if self.handle_api_key_entry_key_event(&key_event) {
            return;
        }

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
            KeyCode::Char('2') => self.start_api_key_entry(),
            KeyCode::Enter => {
                let sign_in_state = { (*self.sign_in_state.read().unwrap()).clone() };
                match sign_in_state {
                    SignInState::PickMode => match self.highlighted_mode {
                        AuthMode::ChatGPT => {
                            self.start_chatgpt_login();
                        }
                        AuthMode::ApiKey => {
                            self.start_api_key_entry();
                        }
                    },
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

    fn handle_paste(&mut self, pasted: String) {
        let _ = self.handle_api_key_entry_paste(pasted);
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
    pub auth_manager: Arc<AuthManager>,
}

impl AuthModeWidget {
    fn render_pick_mode(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                "  ".into(),
                "Sign in with ChatGPT to use Codex as part of your paid plan".into(),
            ]),
            Line::from(vec![
                "  ".into(),
                "or connect an API key for usage-based billing".into(),
            ]),
            "".into(),
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
                format!("  {}. {text}", idx + 1).into()
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
        lines.push("".into());
        lines.extend(create_mode_item(
            1,
            AuthMode::ApiKey,
            "Provide your own API key",
            "Pay for what you use",
        ));
        lines.push("".into());
        lines.push(
            // AE: Following styles.md, this should probably be Cyan because it's a user input tip.
            //     But leaving this for a future cleanup.
            "  Press Enter to continue".dim().into(),
        );
        if let Some(err) = &self.error {
            lines.push("".into());
            lines.push(err.as_str().red().into());
        }

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_continue_in_browser(&self, area: Rect, buf: &mut Buffer) {
        let mut spans = vec!["  ".into()];
        // Schedule a follow-up frame to keep the shimmer animation going.
        self.request_frame
            .schedule_frame_in(std::time::Duration::from_millis(100));
        spans.extend(shimmer_spans("Finish signing in via your browser"));
        let mut lines = vec![spans.into(), "".into()];

        let sign_in_state = self.sign_in_state.read().unwrap();
        if let SignInState::ChatGptContinueInBrowser(state) = &*sign_in_state
            && !state.auth_url.is_empty()
        {
            lines.push("  If the link doesn't open automatically, open the following link to authenticate:".into());
            lines.push("".into());
            lines.push(Line::from(state.auth_url.as_str().cyan().underlined()));
            lines.push("".into());
        }

        lines.push("  Press Esc to cancel".dim().into());
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_chatgpt_success_message(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            "✓ Signed in with your ChatGPT account".fg(Color::Green).into(),
            "".into(),
            "  Before you start:".into(),
            "".into(),
            "  Decide how much autonomy you want to grant Codex".into(),
            Line::from(vec![
                "  For more details see the ".into(),
                "\u{1b}]8;;https://github.com/openai/codex\u{7}Codex docs\u{1b}]8;;\u{7}".underlined(),
            ])
            .dim(),
            "".into(),
            "  Codex can make mistakes".into(),
            "  Review the code it writes and commands it runs".dim().into(),
            "".into(),
            "  Powered by your ChatGPT account".into(),
            Line::from(vec![
                "  Uses your plan's rate limits and ".into(),
                "\u{1b}]8;;https://chatgpt.com/#settings\u{7}training data preferences\u{1b}]8;;\u{7}".underlined(),
            ])
            .dim(),
            "".into(),
            "  Press Enter to continue".fg(Color::Cyan).into(),
        ];

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_chatgpt_success(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            "✓ Signed in with your ChatGPT account"
                .fg(Color::Green)
                .into(),
        ];

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_api_key_configured(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            "✓ API key configured".fg(Color::Green).into(),
            "".into(),
            "  Codex will use usage-based billing with your API key.".into(),
        ];

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn render_api_key_entry(&self, area: Rect, buf: &mut Buffer, state: &ApiKeyInputState) {
        let [intro_area, input_area, footer_area] = Layout::vertical([
            Constraint::Min(4),
            Constraint::Length(3),
            Constraint::Min(2),
        ])
        .areas(area);

        let mut intro_lines: Vec<Line> = vec![
            Line::from(vec![
                "> ".into(),
                "Use your own OpenAI API key for usage-based billing".bold(),
            ]),
            "".into(),
            "  Paste or type your API key below. It will be stored locally in auth.json.".into(),
            "".into(),
        ];
        if state.prepopulated_from_env {
            intro_lines.push("  Detected OPENAI_API_KEY environment variable.".into());
            intro_lines.push(
                "  Paste a different key if you prefer to use another account."
                    .dim()
                    .into(),
            );
            intro_lines.push("".into());
        }
        Paragraph::new(intro_lines)
            .wrap(Wrap { trim: false })
            .render(intro_area, buf);

        let content_line: Line = if state.value.is_empty() {
            vec!["Paste or type your API key".dim()].into()
        } else {
            Line::from(state.value.clone())
        };
        Paragraph::new(content_line)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .title("API key")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .render(input_area, buf);

        let mut footer_lines: Vec<Line> = vec![
            "  Press Enter to save".dim().into(),
            "  Press Esc to go back".dim().into(),
        ];
        if let Some(error) = &self.error {
            footer_lines.push("".into());
            footer_lines.push(error.as_str().red().into());
        }
        Paragraph::new(footer_lines)
            .wrap(Wrap { trim: false })
            .render(footer_area, buf);
    }

    fn handle_api_key_entry_key_event(&mut self, key_event: &KeyEvent) -> bool {
        let mut should_save: Option<String> = None;
        let mut should_request_frame = false;

        {
            let mut guard = self.sign_in_state.write().unwrap();
            if let SignInState::ApiKeyEntry(state) = &mut *guard {
                match key_event.code {
                    KeyCode::Esc => {
                        *guard = SignInState::PickMode;
                        self.error = None;
                        should_request_frame = true;
                    }
                    KeyCode::Enter => {
                        let trimmed = state.value.trim().to_string();
                        if trimmed.is_empty() {
                            self.error = Some("API key cannot be empty".to_string());
                            should_request_frame = true;
                        } else {
                            should_save = Some(trimmed);
                        }
                    }
                    KeyCode::Backspace => {
                        if state.prepopulated_from_env {
                            state.value.clear();
                            state.prepopulated_from_env = false;
                        } else {
                            state.value.pop();
                        }
                        self.error = None;
                        should_request_frame = true;
                    }
                    KeyCode::Char(c)
                        if !key_event.modifiers.contains(KeyModifiers::CONTROL)
                            && !key_event.modifiers.contains(KeyModifiers::ALT) =>
                    {
                        if state.prepopulated_from_env {
                            state.value.clear();
                            state.prepopulated_from_env = false;
                        }
                        state.value.push(c);
                        self.error = None;
                        should_request_frame = true;
                    }
                    _ => {}
                }
                // handled; let guard drop before potential save
            } else {
                return false;
            }
        }

        if let Some(api_key) = should_save {
            self.save_api_key(api_key);
        } else if should_request_frame {
            self.request_frame.schedule_frame();
        }
        true
    }

    fn handle_api_key_entry_paste(&mut self, pasted: String) -> bool {
        let trimmed = pasted.trim();
        if trimmed.is_empty() {
            return false;
        }

        let mut guard = self.sign_in_state.write().unwrap();
        if let SignInState::ApiKeyEntry(state) = &mut *guard {
            if state.prepopulated_from_env {
                state.value = trimmed.to_string();
                state.prepopulated_from_env = false;
            } else {
                state.value.push_str(trimmed);
            }
            self.error = None;
        } else {
            return false;
        }

        drop(guard);
        self.request_frame.schedule_frame();
        true
    }

    fn start_api_key_entry(&mut self) {
        self.error = None;
        let prefill_from_env = read_openai_api_key_from_env();
        let mut guard = self.sign_in_state.write().unwrap();
        match &mut *guard {
            SignInState::ApiKeyEntry(state) => {
                if state.value.is_empty() {
                    if let Some(prefill) = prefill_from_env {
                        state.value = prefill;
                        state.prepopulated_from_env = true;
                    } else {
                        state.prepopulated_from_env = false;
                    }
                }
            }
            _ => {
                *guard = SignInState::ApiKeyEntry(ApiKeyInputState {
                    value: prefill_from_env.clone().unwrap_or_default(),
                    prepopulated_from_env: prefill_from_env.is_some(),
                });
            }
        }
        drop(guard);
        self.request_frame.schedule_frame();
    }

    fn save_api_key(&mut self, api_key: String) {
        match login_with_api_key(&self.codex_home, &api_key) {
            Ok(()) => {
                self.error = None;
                self.login_status = LoginStatus::AuthMode(AuthMode::ApiKey);
                self.auth_manager.reload();
                *self.sign_in_state.write().unwrap() = SignInState::ApiKeyConfigured;
            }
            Err(err) => {
                self.error = Some(format!("Failed to save API key: {err}"));
                let mut guard = self.sign_in_state.write().unwrap();
                if let SignInState::ApiKeyEntry(existing) = &mut *guard {
                    if existing.value.is_empty() {
                        existing.value.push_str(&api_key);
                    }
                    existing.prepopulated_from_env = false;
                } else {
                    *guard = SignInState::ApiKeyEntry(ApiKeyInputState {
                        value: api_key,
                        prepopulated_from_env: false,
                    });
                }
            }
        }

        self.request_frame.schedule_frame();
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
}

impl StepStateProvider for AuthModeWidget {
    fn get_step_state(&self) -> StepState {
        let sign_in_state = self.sign_in_state.read().unwrap();
        match &*sign_in_state {
            SignInState::PickMode
            | SignInState::ApiKeyEntry(_)
            | SignInState::ChatGptContinueInBrowser(_)
            | SignInState::ChatGptSuccessMessage => StepState::InProgress,
            SignInState::ChatGptSuccess | SignInState::ApiKeyConfigured => StepState::Complete,
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
            SignInState::ApiKeyEntry(state) => {
                self.render_api_key_entry(area, buf, state);
            }
            SignInState::ApiKeyConfigured => {
                self.render_api_key_configured(area, buf);
            }
        }
    }
}
